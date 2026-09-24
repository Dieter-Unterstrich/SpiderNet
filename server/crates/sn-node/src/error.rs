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
}
