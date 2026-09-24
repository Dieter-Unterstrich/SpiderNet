//! End-to-end test: a `MeshExit` fetches through a real HTTP proxy.
//!
//! The fake proxy is a minimal HTTP forward proxy: it accepts absolute-URI
//! requests (`GET http://origin/file.bin HTTP/1.1`), extracts the `Range`
//! header and serves the requested slice from an in-memory buffer —
//! exactly the shape of an `sn-exit` service bound to a neighbor's
//! Yggdrasil address (minus the overlay transport, which is transparent
//! to HTTP).

#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::num::{NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use sn_fetch::exit::{ExitRegistry, MeshExit};
use sn_fetch::plan::{ExitSpecs, FetchPlan};
use sn_fetch::types::{
    Egress, ExitId, ExitWeight, FileSize, MinSegmentBytes, SegmentsPerExit, TargetUrl,
};

/// Minimal forward proxy serving one fixed object from memory.
async fn spawn_proxy(data: Arc<Vec<u8>>) -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let data = Arc::clone(&data);
            tokio::spawn(async move {
                // Read the request head.
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    if socket.read(&mut byte).await.unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(byte[0]);
                }
                let head_text = String::from_utf8_lossy(&head);

                // First line: `GET http://origin/path HTTP/1.1`.
                let first_line = head_text.lines().next().unwrap_or_default();
                if !first_line.starts_with("GET http://") {
                    socket
                        .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                        .await
                        .ok();
                    return;
                }

                // Extract the requested byte range (if any). Header
                // names arrive lowercased from some clients.
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
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n",
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

#[tokio::test]
async fn mesh_exit_downloads_through_the_proxy() {
    let data: Vec<u8> = (0..64_000u32).map(|i| (i % 251) as u8).collect();
    let data = Arc::new(data.clone());
    let proxy_addr = spawn_proxy(Arc::clone(&data)).await;

    let proxy_url = format!("http://{proxy_addr}");
    let specs: ExitSpecs = vec![(
        ExitId::new(NonZeroU8::new(1).expect("1 is non-zero")),
        ExitWeight::new(NonZeroU32::new(1).expect("1 is non-zero")),
        Egress::proxy(&proxy_url).expect("proxy url is valid"),
    )];

    // The origin URL is fictional — the proxy serves the content; this
    // proves the request really went through the proxy, not straight to
    // an origin server.
    let target = TargetUrl::parse("http://origin.invalid/file.bin").expect("valid url");
    let file_size = FileSize::try_new(data.len() as u64).expect("non-zero in test");

    let plan = FetchPlan::build(
        target.clone(),
        file_size,
        &specs,
        SegmentsPerExit::new(NonZeroUsize::new(2).expect("2 is non-zero")),
        MinSegmentBytes::new(NonZeroU64::new(1024).expect("1024 is non-zero")),
    )
    .expect("plan builds");
    plan.validate().expect("plan validates");

    let mut registry = ExitRegistry::empty();
    registry.register(Arc::new(
        MeshExit::new(
            ExitId::new(NonZeroU8::new(1).expect("1 is non-zero")),
            reqwest::Url::parse(&proxy_url).expect("proxy url is valid"),
        )
        .expect("mesh exit builds"),
    ));

    let parts = TempDir::new().expect("tempdir in test");
    let stats = sn_fetch::exec::run_segments(&plan, &registry, parts.path())
        .await
        .expect("segments succeed through the proxy");
    let total: u64 = stats.iter().map(|s| s.bytes).sum();
    assert_eq!(total, data.len() as u64);

    let output = parts.path().join("mesh.bin");
    let expected_digest =
        sn_fetch::types::Sha256Digest::from_bytes(Sha256::digest(data.as_ref().as_slice()).into());
    let digest = sn_fetch::exec::assemble(parts.path(), &output, file_size, Some(&expected_digest))
        .await
        .expect("assembly verifies");
    assert_eq!(digest, expected_digest);
}

/// The proxy's address (IPv4, arbitrary port) is not a valid proxy URL
/// unless it has an http scheme — Egress::proxy must reject junk.
#[test]
fn egress_rejects_non_http_proxy_schemes() {
    assert!(Egress::proxy("ftp://proxy:2121").is_err());
    assert!(Egress::proxy("not a url").is_err());
    assert!(Egress::proxy("http://127.0.0.1:8080").is_ok());
    assert!(Egress::proxy("https://proxy.example:3128").is_ok());
}
