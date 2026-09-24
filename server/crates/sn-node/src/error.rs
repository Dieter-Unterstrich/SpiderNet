//! Error type for all sn-node operations.

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("yggdrasil binary `{binary}` failed: {stderr}")]
    YggdrasilBinary { binary: String, stderr: String },

    #[error("invalid uri `{uri}`: {hint}")]
    InvalidUri { uri: String, hint: &'static str },

    #[error("invalid admin endpoint `{0}`: expected `tcp://host:port` or `unix:///path`")]
    InvalidAdminEndpoint(String),

    #[error("invalid public key `{0}`: expected 64 hex characters")]
    InvalidPublicKey(String),

    #[error("generated config base has unexpected shape (expected a JSON object)")]
    UnexpectedConfigShape,

    #[error("admin protocol violation: {0}")]
    AdminProtocol(String),

    #[error("admin socket answered with an error: {0}")]
    AdminFailed(String),

    #[error("admin socket operation timed out")]
    Timeout,

    #[error("invalid daemon config: {0}")]
    DaemonConfig(String),

    #[error("overlay did not come up within {timeout_secs}s (admin endpoint {endpoint})")]
    OverlayNotUp { endpoint: String, timeout_secs: u64 },

    #[error("yggdrasil sidecar exited unexpectedly: {0}")]
    SidecarExited(String),

    #[error("exit service cannot listen on {addr}: {source}")]
    ExitListen {
        addr: std::net::SocketAddr,
        source: std::io::Error,
    },

    #[error("exit service error: {0}")]
    Exit(#[from] sn_exit::error::ExitError),

    #[error("invalid interface name `{0}`: expected `auto`, `none` or an interface name")]
    InvalidIfName(String),
}
