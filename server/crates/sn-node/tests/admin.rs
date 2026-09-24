//! Integration tests for the Yggdrasil admin socket client, against a
//! fake admin server speaking the documented protocol.

#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use sn_node::error::NodeError;
use sn_node::yggdrasil::{peers_from_response, AdminClient, AdminEndpoint, SelfInfo};

const SELF_RESPONSE: &str = concat!(
    r#"{"request": {"request": "getself"}, "status": "success", "response": {"#,
    r#""200:1111:2222:3333:4444:5555:6666:7777": {"#,
    r#""address": "200:1111:2222:3333:4444:5555:6666:7777","#,
    r#""key": "0000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1111","#,
    r#""build_name": "yggdrasil", "build_version": "0.5.12","#,
    r#""subnet": "300:1111:2222:3333::/64", "coords": [1, 2, 3]}}}"#
);

const PEERS_RESPONSE: &str = concat!(
    r#"{"request": {"request": "getpeers"}, "status": "success", "response": {"#,
    r#""200:1111:2222:3333:4444:5555:6666:7777": {"coords": [1]}, "#,
    r#""200:9999:2222:3333:4444:5555:6666:7777": {"#,
    r#""coords": [1, 2], "port": 1, "key": "ffff...", "#,
    r#""remote": "tcp://[fe80::1%25eth0]:1337"}}}"#
);

const ERROR_RESPONSE: &str =
    r#"{"request": {"request": "getself"}, "status": "error", "error": "boom"}"#;

/// Fake admin server: answers each connection's request line, then closes.
async fn fake_admin_server(response: &'static str) -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut line = Vec::new();
            let mut byte = [0u8; 1];
            while !line.ends_with(b"\n") {
                if socket.read(&mut byte).await.unwrap_or(0) == 0 {
                    break;
                }
                line.push(byte[0]);
            }
            socket.write_all(response.as_bytes()).await.ok();
            socket.shutdown().await.ok();
        }
    });
    addr
}

fn tcp_endpoint(addr: std::net::SocketAddr) -> AdminEndpoint {
    AdminEndpoint::Tcp {
        host: addr.ip().to_string(),
        port: addr.port(),
    }
}

#[tokio::test]
async fn get_self_parses_address_subnet_and_version() {
    let endpoint = tcp_endpoint(fake_admin_server(SELF_RESPONSE).await);
    let info = SelfInfo::from_response(
        &AdminClient::connect(&endpoint, Duration::from_secs(2))
            .await
            .unwrap()
            .request("getself")
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        info.address.inner().to_string(),
        "200:1111:2222:3333:4444:5555:6666:7777"
    );
    assert_eq!(info.subnet.as_deref(), Some("300:1111:2222:3333::/64"));
    assert_eq!(info.build_version.as_deref(), Some("0.5.12"));
}

#[tokio::test]
async fn get_peers_lists_remote_uris() {
    let endpoint = tcp_endpoint(fake_admin_server(PEERS_RESPONSE).await);
    let value = AdminClient::connect(&endpoint, Duration::from_secs(2))
        .await
        .unwrap()
        .request("getpeers")
        .await
        .unwrap();
    let peers = peers_from_response(&value).unwrap();

    assert_eq!(peers.len(), 2);
    assert_eq!(
        peers[1].remote.as_deref(),
        Some("tcp://[fe80::1%25eth0]:1337")
    );
}

#[tokio::test]
async fn admin_error_is_surfaced() {
    let endpoint = tcp_endpoint(fake_admin_server(ERROR_RESPONSE).await);
    let mut client = AdminClient::connect(&endpoint, Duration::from_secs(2))
        .await
        .unwrap();
    let err = client.request("getself").await.unwrap_err();
    assert!(matches!(err, NodeError::AdminFailed(ref text) if text == "boom"));
}

#[tokio::test]
async fn connection_refused_becomes_io_or_timeout_error() {
    // Port 1 on localhost: virtually guaranteed to be closed.
    let endpoint = AdminEndpoint::Tcp {
        host: "127.0.0.1".into(),
        port: 1,
    };
    let result = AdminClient::get_self(&endpoint, Duration::from_millis(500)).await;
    assert!(matches!(
        result,
        Err(NodeError::Io(_)) | Err(NodeError::Timeout)
    ));
}
