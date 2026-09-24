//! Integration tests for the node daemon, against a *fake* yggdrasil
//! (a shell script; real yggdrasil is not assumed to be installed) and
//! a fake admin socket server speaking the documented protocol.
//!
//! Unix-only: the fake yggdrasil is a shell script and the sidecar
//! handling relies on POSIX process semantics.

#![cfg(unix)]
#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sn_node::daemon_config::{self, ExitBind, ValidatedDaemonConfig};
use sn_node::error::NodeError;
use tokio::net::{TcpListener, TcpStream};

const FAKE_PRIVATE_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

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

/// What the fake yggdrasil does once started with `-useconffile`.
#[derive(Clone, Copy)]
enum SidecarBehavior {
    /// Keep running until killed (test shutdown paths).
    RunForever,
    /// Exit shortly after start (test the supervisor death path).
    DieSoon,
}

/// Fake admin server: answers `getself`/`getpeers` request lines, then
/// closes the connection (mirrors real yggdrasil's one-request behavior).
async fn fake_admin_server() -> SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
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
            let request = String::from_utf8_lossy(&line);
            let response = if request.contains("getpeers") {
                PEERS_RESPONSE
            } else {
                SELF_RESPONSE
            };
            socket.write_all(response.as_bytes()).await.ok();
            socket.shutdown().await.ok();
        }
    });
    addr
}

/// Write a shell script that impersonates the yggdrasil binary: it
/// answers `-genconf -json` with a fixed minimal config and busy-waits
/// (or dies) on `-useconffile`. The script path is returned.
fn write_fake_yggdrasil(dir: &Path, behavior: SidecarBehavior) -> PathBuf {
    let pidfile = dir.join("fake-yggdrasil.pid");
    let wait = match behavior {
        SidecarBehavior::RunForever => "exec sleep 600",
        SidecarBehavior::DieSoon => "exec sleep 0.3",
    };
    let script = format!(
        r#"#!/bin/sh
# fake yggdrasil for the SpiderNet daemon tests
if [ "$1" = "-genconf" ]; then
  printf '%s' '{{"PrivateKey": "{FAKE_PRIVATE_KEY}", "Peers": [], "Listen": []}}'
  exit 0
fi
if [ "$1" = "-useconffile" ]; then
  echo $$ > '{pidfile}'
  {wait}
fi
exit 1
"#,
        pidfile = pidfile.display(),
    );
    let path = dir.join("fake-yggdrasil");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Build a validated daemon config pointing at the fake infrastructure.
fn test_config(
    admin: SocketAddr,
    binary: &Path,
    state_dir: &Path,
    exit_toml: &str,
) -> ValidatedDaemonConfig {
    let text = format!(
        "[node]\nadmin_endpoint = \"tcp://{}:{}\"\nstate_dir = \"{}\"\n\
         [yggdrasil]\nbinary = \"{}\"\n\
         [exit]\n{exit_toml}\n",
        admin.ip(),
        admin.port(),
        state_dir.display(),
        binary.display(),
    );
    daemon_config::parse(&text).unwrap()
}

/// Poll `check` until it returns true or a deadline passes.
async fn wait_until(check: impl Fn() -> bool) -> bool {
    for _ in 0..200 {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Poll a TCP connect until the daemon's exit listener is up.
async fn exit_is_accepting(addr: SocketAddr) -> bool {
    wait_until_async(|| async { TcpStream::connect(addr).await.is_ok() }).await
}

async fn wait_until_async<F>(check: impl Fn() -> F) -> bool
where
    F: std::future::Future<Output = bool>,
{
    for _ in 0..200 {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// A shutdown future driven by a shared flag (avoids a tokio `sync`
/// dependency in tests; the 10 ms polling is plenty for tests).
async fn flag_shutdown(flag: Arc<AtomicBool>) {
    loop {
        if flag.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// A shutdown flag plus a setter closure that flips it.
fn shutdown_flag() -> (Arc<AtomicBool>, impl Fn() + Send + Clone + 'static) {
    let flag = Arc::new(AtomicBool::new(false));
    let setter = {
        let flag = Arc::clone(&flag);
        move || flag.store(true, Ordering::Relaxed)
    };
    (flag, setter)
}

#[tokio::test]
async fn node_key_is_generated_once_and_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let admin = fake_admin_server().await;
    let binary = write_fake_yggdrasil(tmp.path(), SidecarBehavior::RunForever);
    let state_dir = tmp.path().join("state");
    let config = test_config(admin, &binary, &state_dir, "enabled = false");

    // First run: the daemon generates the node key, the yggdrasil config
    // and spawns the fake sidecar, then waits for shutdown.
    let (flag, shutdown) = shutdown_flag();
    let handle = tokio::spawn(sn_node::daemon::run_daemon(
        config.clone(),
        flag_shutdown(flag),
    ));

    let key_path = state_dir.join("node.key");
    assert!(
        wait_for_file(&key_path).await,
        "node key was not generated in time"
    );
    let first_key = std::fs::read_to_string(&key_path).unwrap();
    assert_eq!(first_key.trim(), FAKE_PRIVATE_KEY);
    assert_eq!(
        std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
        0o600,
        "node key must be owner-only"
    );

    let conf = std::fs::read_to_string(state_dir.join("yggdrasil.conf")).unwrap();
    assert!(
        conf.contains("\"PrivateKeyPath\"") && conf.contains(&key_path.display().to_string()),
        "generated config must reference the node key file:\n{conf}"
    );
    assert!(
        conf.contains(&format!(
            "\"AdminListen\": \"tcp://{}:{}\"",
            admin.ip(),
            admin.port()
        )),
        "generated config must use the configured admin endpoint:\n{conf}"
    );

    shutdown();
    let result = tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "first run failed: {result:?}");

    // Second run: the key must be reused, not regenerated.
    let (flag, shutdown) = shutdown_flag();
    let handle = tokio::spawn(sn_node::daemon::run_daemon(config, flag_shutdown(flag)));
    shutdown();
    let result = tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "second run failed: {result:?}");

    let second_key = std::fs::read_to_string(&key_path).unwrap();
    assert_eq!(second_key, first_key, "node identity must survive restarts");
}

async fn wait_for_file(path: &Path) -> bool {
    wait_until(|| path.exists()).await
}

#[tokio::test]
async fn exit_service_listens_and_accepts_connections() {
    let tmp = tempfile::tempdir().unwrap();
    let admin = fake_admin_server().await;
    let binary = write_fake_yggdrasil(tmp.path(), SidecarBehavior::RunForever);
    let state_dir = tmp.path().join("state");

    // Grab a free port, then release it for the daemon to bind.
    let scratch = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let exit_addr = scratch.local_addr().unwrap();
    drop(scratch);

    let config = test_config(
        admin,
        &binary,
        &state_dir,
        &format!("enabled = true\nbind = \"127.0.0.1:{}\"", exit_addr.port()),
    );

    let (flag, shutdown) = shutdown_flag();
    let handle = tokio::spawn(sn_node::daemon::run_daemon(config, flag_shutdown(flag)));

    assert!(
        exit_is_accepting(exit_addr).await,
        "the exit service did not start listening in time"
    );

    shutdown();
    let result = tokio::time::timeout(Duration::from_secs(30), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "daemon run failed: {result:?}");
}

#[tokio::test]
async fn immediate_shutdown_ends_run_daemon_cleanly() {
    // No sidecar (manage = false): an already-running yggdrasil is
    // assumed; the daemon must still exit cleanly on shutdown.
    let config = daemon_config::parse(
        "[yggdrasil]\nmanage = false\n[exit]\nenabled = true\nbind = \"127.0.0.1:0\"\n",
    )
    .unwrap();
    assert!(matches!(
        config.exit().unwrap().bind(),
        ExitBind::Explicit(_)
    ));

    let result = tokio::time::timeout(
        Duration::from_secs(30),
        sn_node::daemon::run_daemon(config, std::future::ready(())),
    )
    .await
    .unwrap();
    assert!(result.is_ok(), "daemon should end cleanly: {result:?}");
}

#[tokio::test]
async fn sidecar_death_fails_the_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    // Admin endpoint points nowhere: only the supervisor's child-death
    // detection can end the run.
    let config = daemon_config::parse(&format!(
        "[node]\nadmin_endpoint = \"tcp://127.0.0.1:1\"\nstate_dir = \"{}\"\n\
         [yggdrasil]\nbinary = \"{}\"\n[exit]\nenabled = false\n",
        tmp.path().join("state").display(),
        write_fake_yggdrasil(tmp.path(), SidecarBehavior::DieSoon).display(),
    ))
    .unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(30),
        sn_node::daemon::run_daemon(config, std::future::pending()),
    )
    .await
    .unwrap();
    assert!(
        matches!(result, Err(NodeError::SidecarExited(_))),
        "expected SidecarExited error, got {result:?}"
    );
}
