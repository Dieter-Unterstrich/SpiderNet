//! The exit server: accept neighbor connections, admit them (allowlist,
//! quota), and relay bytes between neighbor and origin.
//!
//! Relay is verbatim: the exit never rewrites or inspects payload bytes.
//! TLS stays end-to-end between the neighbor's client and the origin —
//! the exit only sees which host is contacted and how many bytes flow.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::{is_public_origin, ExitConfig, OriginPolicy, QuotaPolicy};
use crate::error::ExitError;
use crate::parse::{self, RequestTarget, MAX_HEAD_BYTES};

/// Time budget for reading the request head and connecting to the origin.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

const RESP_FORBIDDEN: &[u8] =
    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const RESP_BAD_REQUEST: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const RESP_TUNNEL_OK: &[u8] = b"HTTP/1.1 200 Connection established\r\n\r\n";

/// Per-neighbor quota ledger: fixed windows anchored at the neighbor's
/// first use, accounting applied after each finished connection.
///
/// v0 limitation: enforcement happens when a connection starts; the
/// bytes a connection carries are accounted when it ends. One very large
/// connection can therefore overshoot the quota.
#[derive(Debug, Default)]
pub struct QuotaLedger {
    clients: Mutex<HashMap<IpAddr, ClientWindow>>,
}

#[derive(Debug)]
struct ClientWindow {
    window_started: Instant,
    used_bytes: u64,
}

/// Admission decision for one connection attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitDecision {
    Allowed,
    /// The neighbor's quota is exhausted; retry after this many seconds.
    QuotaExhausted {
        retry_after_secs: u64,
    },
}

impl QuotaLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether a connection from `source` may start now.
    pub fn admit(&self, source: IpAddr, quota: Option<QuotaPolicy>) -> AdmitDecision {
        let Some(policy) = quota else {
            return AdmitDecision::Allowed;
        };
        let mut clients = Self::lock_recovered(&self.clients);
        let window = roll_window(clients.entry(source), policy, Instant::now());
        if window.used_bytes >= policy.max_bytes() {
            let elapsed = window.window_started.elapsed();
            let remaining = policy
                .window()
                .checked_sub(elapsed)
                .unwrap_or(Duration::ZERO);
            AdmitDecision::QuotaExhausted {
                retry_after_secs: remaining.as_secs().max(1),
            }
        } else {
            AdmitDecision::Allowed
        }
    }

    /// Account the traffic of one finished connection.
    pub fn record(&self, source: IpAddr, bytes: u64, quota: Option<QuotaPolicy>) {
        let Some(policy) = quota else {
            return;
        };
        let mut clients = Self::lock_recovered(&self.clients);
        let window = roll_window(clients.entry(source), policy, Instant::now());
        window.used_bytes = window.used_bytes.saturating_add(bytes);
    }

    /// Total accounted bytes for one neighbor (visible in metrics later).
    pub fn used_bytes(&self, source: IpAddr) -> u64 {
        Self::lock_recovered(&self.clients)
            .get(&source)
            .map_or(0, |w| w.used_bytes)
    }

    /// Lock without panicking on a poisoned mutex: the ledger is
    /// self-healing (windows roll over), so recovering the guard is
    /// always safe here.
    fn lock_recovered(
        clients: &Mutex<HashMap<IpAddr, ClientWindow>>,
    ) -> std::sync::MutexGuard<'_, HashMap<IpAddr, ClientWindow>> {
        clients
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Get the client's window, rolling it over when the policy's window has
/// elapsed since its start.
fn roll_window(
    entry: std::collections::hash_map::Entry<'_, IpAddr, ClientWindow>,
    policy: QuotaPolicy,
    now: Instant,
) -> &mut ClientWindow {
    let window = entry.or_insert_with(|| ClientWindow {
        window_started: now,
        used_bytes: 0,
    });
    if now.duration_since(window.window_started) >= policy.window() {
        window.window_started = now;
        window.used_bytes = 0;
    }
    window
}

/// Accept loop. Runs until the listener errors fatally.
pub async fn serve(config: ExitConfig, listener: TcpListener) -> Result<(), ExitError> {
    let config = Arc::new(config);
    let ledger = Arc::new(QuotaLedger::new());
    loop {
        let (stream, peer) = listener.accept().await?;
        let config = Arc::clone(&config);
        let ledger = Arc::clone(&ledger);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, peer.ip(), config, ledger).await {
                tracing::debug!(%peer, "connection ended: {error}");
            }
        });
    }
}

async fn handle_connection(
    mut neighbor: TcpStream,
    source: IpAddr,
    config: Arc<ExitConfig>,
    ledger: Arc<QuotaLedger>,
) -> Result<(), ExitError> {
    // 1. Read the request head (bounded, with timeout).
    let raw_head = read_head(&mut neighbor).await?;

    // 2. Admission: allowlist, then quota.
    if !config.allowlist().is_allowed(source) {
        neighbor.write_all(RESP_FORBIDDEN).await.ok();
        tracing::info!(%source, "refused: source not allowed");
        return Ok(());
    }
    if let AdmitDecision::QuotaExhausted { retry_after_secs } = ledger.admit(source, config.quota())
    {
        let body = format!(
            "HTTP/1.1 503 Service Unavailable\r\nRetry-After: {retry_after_secs}\r\n\
             Content-Length: 0\r\nConnection: close\r\n\r\n"
        );
        neighbor.write_all(body.as_bytes()).await.ok();
        tracing::info!(%source, "refused: quota exhausted, retry in {retry_after_secs}s");
        return Ok(());
    }

    // 3. Parse and verify the origin is public (SSRF protection).
    let Ok(head) = parse::parse_request_head(&raw_head) else {
        neighbor.write_all(RESP_BAD_REQUEST).await.ok();
        return Err(ExitError::Protocol("unparseable request head"));
    };
    ensure_origin_is_public(&head, config.origin_policy()).await?;

    // 4. Ack tunnels, then relay — verbatim, both directions.
    if head.target == RequestTarget::Tunnel {
        neighbor.write_all(RESP_TUNNEL_OK).await?;
    }

    let origin = connect_origin(&head).await?;

    // Counted relay: byte counters live in the wrappers, so traffic is
    // accounted even when the connection errors or is aborted midway.
    let mut neighbor_counted = CountingIo::new(neighbor);
    let mut origin_counted = CountingIo::new(origin);
    if head.target == RequestTarget::Forward {
        // The neighbor's head is forwarded as the first bytes of the
        // pipe; origin servers accept absolute-URI request targets.
        origin_counted.write_all(&raw_head).await?;
    }

    let relay = tokio::io::copy_bidirectional(&mut neighbor_counted, &mut origin_counted).await;
    // Uplink traffic = what the household's ISP sees: bytes to the
    // origin (egress) + bytes to the neighbor (ingress from origin).
    let up = origin_counted.written_bytes();
    let down = neighbor_counted.written_bytes();
    ledger.record(source, up + down, config.quota());
    relay?;
    tracing::debug!(
        %source,
        "target={:?} origin={}:{} closed ({up}+{down} bytes)",
        head.target,
        head.host,
        head.port
    );
    Ok(())
}

/// Stream wrapper that counts all bytes read from and written to the
/// inner stream, so quota accounting does not depend on the relay's
/// outcome.
struct CountingIo<T> {
    inner: T,
    read: std::sync::atomic::AtomicU64,
    written: std::sync::atomic::AtomicU64,
}

impl<T> CountingIo<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            read: std::sync::atomic::AtomicU64::new(0),
            written: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn written_bytes(&self) -> u64 {
        self.written.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<T: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for CountingIo<T> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if matches!(result, std::task::Poll::Ready(Ok(()))) {
            let after = buf.filled().len();
            self.read.fetch_add(
                (after - before) as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        result
    }
}

impl<T: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for CountingIo<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let result = std::pin::Pin::new(&mut self.inner).poll_write(cx, buf);
        if let std::task::Poll::Ready(Ok(n)) = result {
            self.written
                .fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Read up to [`MAX_HEAD_BYTES`] bytes until the head terminator
/// `\r\n\r\n`. Returns the raw head *including* the terminator.
async fn read_head(stream: &mut TcpStream) -> Result<Vec<u8>, ExitError> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    let deadline = tokio::time::Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
        if head.len() >= MAX_HEAD_BYTES {
            return Err(ExitError::Protocol("request head too large"));
        }
        match tokio::time::timeout_at(deadline, stream.read(&mut byte)).await {
            Err(_) => return Err(ExitError::Io(std::io::ErrorKind::TimedOut.into())),
            Ok(Ok(0)) => return Err(ExitError::Protocol("connection closed before head ended")),
            Ok(Ok(_)) => {
                head.push(byte[0]);
                if head.ends_with(b"\r\n\r\n") {
                    return Ok(head);
                }
            }
            Ok(Err(error)) => return Err(ExitError::Io(error)),
        }
    }
}

/// DNS resolution happens here so the blocklist sees the *resolved* IPs,
/// not just the hostname the neighbor typed.
async fn ensure_origin_is_public(
    head: &parse::RequestHead,
    policy: OriginPolicy,
) -> Result<(), ExitError> {
    if policy == OriginPolicy::AnyForTesting {
        return Ok(());
    }
    let resolved: Vec<std::net::SocketAddr> =
        tokio::net::lookup_host((head.host.as_str(), head.port))
            .await?
            .collect();
    if resolved.is_empty() {
        return Err(ExitError::Protocol("origin could not be resolved"));
    }
    for addr in resolved {
        if !is_public_origin(addr.ip()) {
            return Err(ExitError::Protocol("origin is not a public address"));
        }
    }
    Ok(())
}

async fn connect_origin(head: &parse::RequestHead) -> Result<TcpStream, ExitError> {
    let stream = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        TcpStream::connect((head.host.as_str(), head.port)),
    )
    .await
    .map_err(|_| ExitError::Io(std::io::ErrorKind::TimedOut.into()))??;
    Ok(stream)
}
