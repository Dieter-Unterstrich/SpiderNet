//! The `SpiderNet` node daemon: supervises the Yggdrasil overlay sidecar
//! and runs the optional exit service in-process.
//!
//! Responsibilities (kill-switch semantics):
//!
//! - manage the yggdrasil sidecar when `[yggdrasil] manage = true`:
//!   persistent node key at `<state_dir>/node.key`, generated config at
//!   `<state_dir>/yggdrasil.conf`, child process with graceful stop;
//! - wait for the overlay address via the admin socket and log it;
//! - run the exit service (`sn-exit`) in-process when
//!   `[exit] enabled = true`; otherwise leech mode (overlay only,
//!   receive without offering an exit);
//! - on shutdown (SIGINT/SIGTERM wired in by the CLI): stop the accept
//!   loop, then stop the sidecar — SIGTERM first, `start_kill` after a
//!   bounded grace period;
//! - if the sidecar dies unexpectedly: shut down and exit with an error
//!   (supervisor duty).
//!
//! [`run_daemon`] takes the shutdown as a plain future so tests can drive
//! it without signals.

use std::future::Future;
use std::path::Path;
use std::pin::{pin, Pin};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::process::Child;

use crate::config::{self, PublicKeyHex};
use crate::daemon_config::{ValidatedDaemonConfig, YGGDRASIL_CONF_FILE};
use crate::error::NodeError;
use crate::yggdrasil::{AdminClient, YggAddr};

/// Printed first by the daemon CLI; the software carries no warranty and
/// accepts no liability (AGPL-3.0, sections 15/16).
pub const NO_WARRANTY_BANNER: &str =
    "NO WARRANTY — SpiderNet sn-node daemon (AGPL-3.0, no liability)";

/// How long the daemon waits for the overlay (admin socket) to come up.
const OVERLAY_WAIT: Duration = Duration::from_secs(60);
/// Interval between admin socket attempts while the overlay is down.
const OVERLAY_POLL: Duration = Duration::from_secs(1);
/// Grace period for SIGTERM-terminated yggdrasil before we kill it.
const TERMINATE_GRACE: Duration = Duration::from_secs(5);

/// Run the node daemon until `shutdown` completes or a fatal error
/// occurs. Returns `Ok(())` on a clean shutdown.
pub async fn run_daemon(
    config: ValidatedDaemonConfig,
    shutdown: impl Future<Output = ()> + Send,
) -> Result<(), NodeError> {
    let mut shutdown = pin!(shutdown);

    log_config_summary(&config);

    let mut child = if config.manage() {
        Some(spawn_sidecar(&config).await?)
    } else {
        None
    };

    let overlay = match wait_for_overlay(&config, &mut child, shutdown.as_mut()).await {
        OverlayOutcome::Up(address) => address,
        OverlayOutcome::Shutdown => {
            terminate_sidecar(&mut child).await;
            return Ok(());
        }
        OverlayOutcome::Failed(error) => {
            kill_child(&mut child).await;
            return Err(error);
        }
    };

    if let Some(exit) = config.exit() {
        let bind = exit.bind().resolve(overlay);
        let listener = match TcpListener::bind(bind).await {
            Ok(listener) => listener,
            Err(source) => {
                kill_child(&mut child).await;
                return Err(NodeError::ExitListen { addr: bind, source });
            }
        };
        tracing::info!("exit listening on {bind}");
        if exit.allowlist().is_loopback_only() {
            tracing::warn!(
                "exit in DEV MODE: no neighbor allowlist — only loopback clients are accepted"
            );
        }

        let exit_config =
            sn_exit::config::ExitConfig::new(bind, exit.allowlist().clone(), exit.quota());
        tokio::select! {
            result = sn_exit::proxy::serve_until(exit_config, listener, shutdown.as_mut()) => {
                if let Err(error) = result {
                    kill_child(&mut child).await;
                    return Err(NodeError::Exit(error));
                }
            }
            status = child_exit(&mut child) => {
                kill_child(&mut child).await;
                return Err(NodeError::SidecarExited(
                    status.unwrap_or_else(|| "unknown".to_string()),
                ));
            }
        }
    } else {
        // Leech mode: overlay only, until signal or sidecar death.
        tokio::select! {
            () = shutdown.as_mut() => {}
            status = child_exit(&mut child) => {
                return Err(NodeError::SidecarExited(
                    status.unwrap_or_else(|| "unknown".to_string()),
                ));
            }
        }
    }

    terminate_sidecar(&mut child).await;
    tracing::info!("SpiderNet node daemon stopped");
    Ok(())
}

fn log_config_summary(config: &ValidatedDaemonConfig) {
    let overlay = if config.manage() {
        "managed sidecar"
    } else {
        "external (yggdrasil.manage = false)"
    };
    let exit = match config.exit() {
        Some(_) => "enabled (sharing the uplink)",
        None => "disabled (leech mode)",
    };
    tracing::info!(
        admin = %config.admin_endpoint().config_value(),
        state_dir = %config.state_dir().display(),
        overlay,
        exit,
        "SpiderNet node daemon starting"
    );
}

/// Prepare the state directory (node key + generated Yggdrasil config)
/// and spawn the sidecar process.
async fn spawn_sidecar(config: &ValidatedDaemonConfig) -> Result<Child, NodeError> {
    let state_dir = config.state_dir().to_path_buf();
    tokio::fs::create_dir_all(&state_dir).await?;
    let key_path = state_dir.join(crate::daemon_config::NODE_KEY_FILE);

    let settings = config.node_settings();
    // One `yggdrasil -genconf -json` run provides both the base config
    // and — on first boot — the fresh node key.
    let json = config::generate_config(config.yggdrasil_binary(), &settings)?;
    let base: serde_json::Value = serde_json::from_str(&json)?;

    if key_path.exists() {
        // The node identity must survive restarts: reuse the persisted
        // key, never regenerate it.
        //
        // The base JSON still embeds a fresh `PrivateKey`, but per the
        // official configuration reference PrivateKeyPath takes
        // precedence when both are set
        // (https://yggdrasil-network.github.io/configurationref.html),
        // so yggdrasil runs with the persisted key.
        let existing = tokio::fs::read_to_string(&key_path).await?;
        let key = existing.trim();
        PublicKeyHex::parse(key).map_err(|err| {
            NodeError::DaemonConfig(format!(
                "existing node key {} is invalid ({err}); the node identity \
                 must stay stable, so it is not regenerated",
                key_path.display()
            ))
        })?;
        tracing::info!("node key loaded from {}", key_path.display());
    } else {
        let generated = base
            .get("PrivateKey")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                NodeError::DaemonConfig(format!(
                    "`{} -genconf -json` output has no `PrivateKey` field",
                    config.yggdrasil_binary()
                ))
            })?;
        let key = PublicKeyHex::parse(generated)
            .map_err(|err| NodeError::DaemonConfig(format!("generated node key: {err}")))?;
        write_private_key(&key_path, key.as_str())?;
        tracing::info!(
            "node key generated and stored at {} (mode 600)",
            key_path.display()
        );
    }

    let conf_path = state_dir.join(YGGDRASIL_CONF_FILE);
    config::write_config(&conf_path, &json)?;
    tracing::info!("yggdrasil config written to {}", conf_path.display());

    let mut command = tokio::process::Command::new(config.yggdrasil_binary());
    command.arg("-useconffile").arg(&conf_path);
    let child = command.spawn()?;
    tracing::info!("yggdrasil sidecar spawned (pid {:?})", child.id());
    Ok(child)
}

/// Write the node's private key with owner-only permissions (mode 600).
fn write_private_key(path: &Path, key: &str) -> Result<(), NodeError> {
    use std::io::Write as _;

    let mut file = std::fs::File::create(path)?;
    file.write_all(key.as_bytes())?;
    file.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Result of waiting for the overlay.
enum OverlayOutcome {
    Up(YggAddr),
    /// Shutdown was requested while waiting.
    Shutdown,
    Failed(NodeError),
}

/// Poll the admin socket until the overlay reports the node's own
/// address. Aborts on shutdown and on unexpected sidecar death; the
/// sidecar child (if any) is reaped by the `child.wait()` arm.
async fn wait_for_overlay(
    config: &ValidatedDaemonConfig,
    child: &mut Option<Child>,
    mut shutdown: Pin<&mut (impl Future<Output = ()> + Send)>,
) -> OverlayOutcome {
    let admin = config.admin_endpoint();
    let deadline = tokio::time::Instant::now() + OVERLAY_WAIT;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return OverlayOutcome::Failed(NodeError::OverlayNotUp {
                endpoint: admin.config_value(),
                timeout_secs: OVERLAY_WAIT.as_secs(),
            });
        }

        let attempt_timeout = remaining.min(OVERLAY_POLL);
        tokio::select! {
            biased;
            () = shutdown.as_mut() => return OverlayOutcome::Shutdown,
            status = child_exit(&mut *child) => {
                return OverlayOutcome::Failed(NodeError::SidecarExited(
                    status.unwrap_or_else(|| "unknown".to_string()),
                ));
            }
            result = AdminClient::get_self(admin, attempt_timeout) => match result {
                Ok(info) => {
                    tracing::info!("overlay is up: address {}", info.address);
                    if let Ok(peers) = AdminClient::get_peers(admin, attempt_timeout).await {
                        tracing::info!("overlay peers: {}", peers.len());
                    } else {
                        tracing::info!("overlay peers: not reported yet");
                    }
                    return OverlayOutcome::Up(info.address);
                }
                Err(_) => tokio::time::sleep(OVERLAY_POLL).await,
            }
        }
    }
}

/// Resolve when the sidecar process exits; never resolves without one.
/// Returns a human-readable exit status description.
async fn child_exit(child: &mut Option<Child>) -> Option<String> {
    if let Some(child) = child {
        return Some(match child.wait().await {
            Ok(status) => status.to_string(),
            Err(error) => format!("wait failed: {error}"),
        });
    }
    std::future::pending::<()>().await;
    None
}

/// Stop the sidecar gracefully: SIGTERM via `kill`, bounded grace, then
/// a hard kill.
async fn terminate_sidecar(child: &mut Option<Child>) {
    let Some(child) = child.as_mut() else {
        return;
    };
    let Some(pid) = child.id() else {
        tracing::info!("yggdrasil sidecar already gone");
        return;
    };

    tracing::info!("stopping yggdrasil sidecar (pid {pid}): sending SIGTERM");
    match std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!("`kill -TERM {pid}` failed: {status}"),
        Err(error) => tracing::warn!("could not run `kill -TERM {pid}`: {error}"),
    }

    match tokio::time::timeout(TERMINATE_GRACE, child.wait()).await {
        Ok(Ok(status)) => tracing::info!("yggdrasil sidecar stopped ({status})"),
        Ok(Err(error)) => tracing::warn!("waiting for yggdrasil failed: {error}"),
        Err(_) => {
            tracing::warn!(
                "yggdrasil did not exit within {}s; killing",
                TERMINATE_GRACE.as_secs()
            );
            if let Err(error) = child.start_kill() {
                tracing::warn!("start_kill failed: {error}");
            }
            child.wait().await.ok();
            tracing::info!("yggdrasil sidecar killed");
        }
    }
}

/// Kill the sidecar immediately (failure path); a no-op when it is
/// already gone.
async fn kill_child(child: &mut Option<Child>) {
    let Some(child) = child.as_mut() else {
        return;
    };
    if child.id().is_some() {
        tracing::warn!("killing yggdrasil sidecar after failure");
        if let Err(error) = child.start_kill() {
            tracing::warn!("start_kill failed: {error}");
        }
    }
    child.wait().await.ok();
}
