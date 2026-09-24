//! Integration tests against a running exit server (loopback): plain
//! HTTP forwarding, CONNECT tunneling, quota enforcement and the source
//! allowlist.

#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use sn_exit::config::{ExitConfig, QuotaPolicy, SourceAllowlist};
use sn_exit::proxy::serve;

/// Spawn the exit server on a free loopback port; returns its address.
async fn spawn_exit(config: ExitConfig) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = serve(config, listener).await;
    });
    addr
}

/// Origin server that supports ranges: answers `GET http://origin/…`
/// absolute-URI requests with (parts of) a fixed body.
async fn spawn_origin(data: Arc<Vec<u8>>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let data = Arc::clone(&data);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if socket.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head_text = String::from_utf8_lossy(&head);
                let range = head_text.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if !name.eq_ignore_ascii_case("range") {
                        return None;
                    }
                    let value = value.trim().strip_prefix("bytes=")?;
                    let (start, end) = value.trim().split_once('-')?;
                    Some((start.parse::<u64>().ok()?, end.parse::<u64>().ok()?))
                });

                let (start, end_inclusive, status) = match range {
                    Some((start, end)) => (start, end, "206 Partial Content"),
                    None => (0, data.len() as u64 - 1, "200 OK"),
                };
                let slice = &data[start as usize..=(end_inclusive as usize).min(data.len() - 1)];
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    slice.len()
                );
                socket.write_all(response.as_bytes()).await.ok();
                socket.write_all(slice).await.ok();
                socket.shutdown().await.ok();
            });
        }
    });
    addr
}

/// Echo TCP server for the CONNECT tunnel test (stands in for a TLS
/// origin from the relay's perspective: opaque bytes both ways).
async fn spawn_echo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = [0u8; 512];
                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if socket.write_all(&buffer[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });
    addr
}

async fn read_response_head(stream: &mut TcpStream) -> String {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        let n = stream.read(&mut byte).await.unwrap_or(0);
        assert!(n > 0, "connection closed before response head ended");
        head.push(byte[0]);
    }
    String::from_utf8(head).unwrap()
}

fn dev_config(quota: Option<QuotaPolicy>) -> ExitConfig {
    ExitConfig::new(
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        SourceAllowlist::loopback_only(),
        quota,
    )
    .with_origin_policy(sn_exit::config::OriginPolicy::AnyForTesting)
}

#[tokio::test]
async fn forward_request_relays_absolute_uri_get() {
    let data: Vec<u8> = (0..32_000u32).map(|i| (i % 253) as u8).collect();
    let origin = spawn_origin(Arc::new(data.clone())).await;
    let exit = spawn_exit(dev_config(None)).await;

    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    let request = format!("GET http://{origin}/file.bin HTTP/1.1\r\nHost: origin\r\n\r\n");
    neighbor.write_all(request.as_bytes()).await.unwrap();

    let response_head = read_response_head(&mut neighbor).await;
    assert!(
        response_head.starts_with("HTTP/1.1 200 OK"),
        "{response_head}"
    );

    let mut body = Vec::new();
    neighbor.read_to_end(&mut body).await.unwrap();
    assert_eq!(body, data);
}

#[tokio::test]
async fn forward_request_passes_range_headers_through() {
    let data: Vec<u8> = (0..16_000u32).map(|i| (i % 241) as u8).collect();
    let origin = spawn_origin(Arc::new(data.clone())).await;
    let exit = spawn_exit(dev_config(None)).await;

    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    let request = format!("GET http://{origin}/file.bin HTTP/1.1\r\nRange: bytes=100-199\r\n\r\n");
    neighbor.write_all(request.as_bytes()).await.unwrap();

    let response_head = read_response_head(&mut neighbor).await;
    assert!(
        response_head.starts_with("HTTP/1.1 206 Partial Content"),
        "{response_head}"
    );
    let mut body = Vec::new();
    neighbor.read_to_end(&mut body).await.unwrap();
    assert_eq!(&body, &data[100..=199]);
}

#[tokio::test]
async fn connect_tunnel_relays_bytes_both_ways() {
    let echo = spawn_echo_server().await;
    let exit = spawn_exit(dev_config(None)).await;

    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    let request = format!("CONNECT {echo} HTTP/1.1\r\nHost: {echo}\r\n\r\n");
    neighbor.write_all(request.as_bytes()).await.unwrap();

    let response_head = read_response_head(&mut neighbor).await;
    assert!(
        response_head.starts_with("HTTP/1.1 200"),
        "tunnel must be established: {response_head}"
    );

    // The tunnel is open: echo some opaque "TLS-like" bytes through it.
    let payload: Vec<u8> = (0..=255u8).cycle().take(700).collect();
    neighbor.write_all(&payload).await.unwrap();
    let mut echoed = vec![0u8; payload.len()];
    neighbor.read_exact(&mut echoed).await.unwrap();
    assert_eq!(echoed, payload);
    let _ = neighbor.shutdown().await;
}

#[tokio::test]
async fn quota_exhaustion_returns_503_with_retry_after() {
    let data = Arc::new(vec![42u8; 8_000]);
    let origin = spawn_origin(Arc::clone(&data)).await;
    // 1 KiB quota, 1 h window: the first connection (~8 KB) exhausts it.
    let quota = QuotaPolicy::try_new(1024, Duration::from_secs(3600)).unwrap();
    let exit = spawn_exit(dev_config(Some(quota))).await;

    let fetch = |exit: SocketAddr, origin: SocketAddr| async move {
        let mut neighbor = TcpStream::connect(exit).await.unwrap();
        let request = format!("GET http://{origin}/file.bin HTTP/1.1\r\nHost: o\r\n\r\n");
        neighbor.write_all(request.as_bytes()).await.unwrap();
        let head = read_response_head(&mut neighbor).await;
        let mut body = Vec::new();
        let _ = neighbor.read_to_end(&mut body).await;
        (head, body)
    };

    let (first, body) = fetch(exit, origin).await;
    assert!(
        first.starts_with("HTTP/1.1 200 OK"),
        "first request must pass: {first}"
    );
    assert_eq!(body.len(), 8_000, "first response body must be complete");

    // `record()` runs after the first connection is fully torn down —
    // poll briefly until the ledger has accounted the traffic.
    let mut second = String::new();
    for _ in 0..100 {
        let (head, _body) = fetch(exit, origin).await;
        second = head;
        if second.starts_with("HTTP/1.1 503") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        second.starts_with("HTTP/1.1 503"),
        "second request must be refused: {second}"
    );
    assert!(second.contains("Retry-After:"), "{second}");
}

#[tokio::test]
async fn disallowed_source_gets_403() {
    // Explicit allowlist WITHOUT loopback → loopback clients refused.
    let config = ExitConfig::new(
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        SourceAllowlist::new(vec![IpAddr::V6(Ipv6Addr::new(0x200, 1, 0, 0, 0, 0, 0, 2))]),
        None,
    );
    let exit = spawn_exit(config).await;

    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    neighbor
        .write_all(b"GET http://93.184.216.34/x HTTP/1.1\r\nHost: o\r\n\r\n")
        .await
        .unwrap();
    let response = read_response_head(&mut neighbor).await;
    assert!(response.starts_with("HTTP/1.1 403"), "{response}");
}

#[tokio::test]
async fn origin_form_request_gets_400() {
    let exit = spawn_exit(dev_config(None)).await;
    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    neighbor
        .write_all(b"GET /file.bin HTTP/1.1\r\nHost: o\r\n\r\n")
        .await
        .unwrap();
    let response = read_response_head(&mut neighbor).await;
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
}

#[tokio::test]
async fn oversized_head_gets_no_hang_and_no_panic() {
    let exit = spawn_exit(dev_config(None)).await;
    let mut neighbor = TcpStream::connect(exit).await.unwrap();
    let big_line = "x".repeat(20 * 1024);
    let request = format!("GET http://{big_line}/ HTTP/1.1\r\n\r\n");
    neighbor.write_all(request.as_bytes()).await.unwrap();
    // The server must close or answer — either way, no hang.
    let mut buffer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), neighbor.read_to_end(&mut buffer))
        .await
        .expect("no hang on oversized head");
}
