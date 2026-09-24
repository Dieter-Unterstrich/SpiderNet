//! A minimal HTTP server for integration tests. Supports HEAD, GET with
//! optional range handling (configurable), simulated failures for range
//! GETs (flaky/exhausted/non-retryable modes), and a broken exit whose
//! client fails at transport level, for retry tests.

#![forbid(unsafe_code)]
// Tests are allowed to unwrap/expect; production code denies them via
// workspace lints (server/Cargo.toml).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use sn_fetch::exit::HttpExit;
use sn_fetch::types::ExitId;

/// When the test origin should fail `Range` GETs (retry simulation).
/// Only used by the retry test binary; allow dead_code for the others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FailMode {
    #[default]
    Never,
    /// Fail the first `n` range GETs with HTTP 500, then serve normally.
    FirstN(u32),
    /// Fail every range GET with HTTP 500.
    Always,
    /// Answer every range GET with the given status (e.g. 416 to
    /// simulate a non-retryable range rejection).
    AlwaysStatus(u16),
}

/// Knobs for the test origin.
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub struct ServerConfig {
    pub fail_mode: FailMode,
}

pub struct TestServer {
    pub addr: SocketAddr,
    _handle: JoinHandle<()>,
}

impl TestServer {
    /// Start serving `data` on a random port. When `ranges` is false the
    /// server behaves like a range-hostile origin: no `Accept-Ranges`
    /// header, and `Range` requests are answered with the full body.
    pub fn start(data: Vec<u8>, ranges: bool) -> Self {
        Self::start_with(data, ranges, ServerConfig::default())
    }

    /// Start with a custom [`ServerConfig`] (failure simulation).
    pub fn start_with(data: Vec<u8>, ranges: bool, config: ServerConfig) -> Self {
        let data = Arc::new(data);
        let config = Arc::new(config);
        let range_get_count = Arc::new(AtomicU32::new(0));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind in test");
        let addr = listener.local_addr().expect("local addr in test");
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (data, config, range_get_count) = (
                    Arc::clone(&data),
                    Arc::clone(&config),
                    Arc::clone(&range_get_count),
                );
                std::thread::spawn(move || {
                    let _ = handle_conn(stream, &data, ranges, &config, &range_get_count);
                });
            }
        });
        Self {
            addr,
            _handle: handle,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }
}

fn handle_conn(
    stream: TcpStream,
    data: &[u8],
    ranges: bool,
    config: &ServerConfig,
    range_get_count: &AtomicU32,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let mut range_header: Option<String> = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("range:") {
            range_header = Some(value.trim().to_string());
        }
    }

    let mut stream = stream;
    let len = data.len() as u64;
    let method = request_line
        .split_whitespace()
        .next()
        .unwrap_or("GET")
        .to_string();

    match method.as_str() {
        "HEAD" => {
            let accept = if ranges { "bytes" } else { "none" };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nAccept-Ranges: {accept}\r\nConnection: close\r\n\r\n"
            )?;
        }
        "GET" => {
            let range = range_header.as_deref().and_then(parse_range).flatten();
            match (ranges, range) {
                (true, Some((start, end))) => {
                    if should_fail_range_get(config, range_get_count) {
                        write_fail_response(&mut stream, config.fail_mode)?;
                        return Ok(());
                    }
                    let slice_len = end - start + 1;
                    write!(
                        stream,
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {slice_len}\r\nContent-Range: bytes {start}-{end}/{len}\r\nConnection: close\r\n\r\n"
                    )?;
                    stream.write_all(&data[start as usize..=(end as usize).min(data.len() - 1)])?;
                }
                _ => {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                    )?;
                    stream.write_all(data)?;
                }
            }
        }
        _ => {
            write!(
                stream,
                "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )?;
        }
    }

    stream.flush()?;
    Ok(())
}

/// Decide (and count) whether this range GET should fail. HEAD and
/// non-range GETs are never counted nor failed.
fn should_fail_range_get(config: &ServerConfig, range_get_count: &AtomicU32) -> bool {
    match config.fail_mode {
        FailMode::Never => false,
        FailMode::Always | FailMode::AlwaysStatus(_) => true,
        FailMode::FirstN(limit) => range_get_count.fetch_add(1, Ordering::Relaxed) < limit,
    }
}

fn write_fail_response(stream: &mut TcpStream, mode: FailMode) -> std::io::Result<()> {
    let status = match mode {
        FailMode::AlwaysStatus(status) => status,
        _ => 500,
    };
    write!(
        stream,
        "HTTP/1.1 {status} Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()
}

/// Parse `bytes=a-b` into inclusive `(start, end)`. Anything else
/// (open-ended, malformed) is treated as absent.
fn parse_range(value: &str) -> Option<Option<(u64, u64)>> {
    let spec = value.strip_prefix("bytes=")?;
    let (start_str, end_str) = spec.split_once('-')?;
    let start: u64 = start_str.trim().parse().ok()?;
    let end: u64 = end_str.trim().parse().ok()?;
    if end >= start {
        Some(Some((start, end)))
    } else {
        Some(None)
    }
}

/// An exit whose client sends every request through a proxy that
/// refuses connections — every request through this exit fails at
/// transport level (retryable), as needed for exit-reassignment tests.
#[allow(dead_code)]
pub struct BrokenProxyExit {
    id: ExitId,
    client: reqwest::Client,
}

impl BrokenProxyExit {
    #[allow(dead_code)]
    pub fn new(id: ExitId) -> Self {
        let proxy = reqwest::Proxy::http("http://127.0.0.1:1").expect("static proxy url parses");
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("client builds in test");
        Self { id, client }
    }
}

impl HttpExit for BrokenProxyExit {
    fn id(&self) -> ExitId {
        self.id
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
