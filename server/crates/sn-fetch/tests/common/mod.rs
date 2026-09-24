//! A minimal HTTP server for integration tests. Supports HEAD, GET with
//! optional range handling (configurable), `Connection: close` semantics.

#![forbid(unsafe_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct TestServer {
    pub addr: SocketAddr,
    _handle: JoinHandle<()>,
}

impl TestServer {
    /// Start serving `data` on a random port. When `ranges` is false the
    /// server behaves like a range-hostile origin: no `Accept-Ranges`
    /// header, and `Range` requests are answered with the full body.
    pub fn start(data: Vec<u8>, ranges: bool) -> Self {
        let data = Arc::new(data);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind in test");
        let addr = listener.local_addr().expect("addr in test");
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let data = Arc::clone(&data);
                std::thread::spawn(move || {
                    let _ = handle_conn(stream, &data, ranges);
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

fn handle_conn(stream: TcpStream, data: &[u8], ranges: bool) -> std::io::Result<()> {
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
