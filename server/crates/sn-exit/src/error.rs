//! Error type for all sn-exit operations.

#[derive(Debug, thiserror::Error)]
pub enum ExitError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid bind address `{0}`")]
    InvalidBind(String),

    #[error("invalid allowed source address `{0}`")]
    InvalidSource(String),

    #[error("invalid quota: {0}")]
    InvalidQuota(&'static str),

    #[error("protocol violation: {0}")]
    Protocol(&'static str),
}
