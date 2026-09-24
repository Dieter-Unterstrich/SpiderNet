//! Error type for all sn-fetch operations.

use crate::types::{ExitId, SegmentIndex};

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid url `{0}`: scheme must be http or https")]
    InvalidUrl(String),

    #[error("file size must be greater than zero")]
    ZeroFileSize,

    #[error("segment {index}: received {got} bytes, expected {expected}")]
    SegmentLengthMismatch {
        index: SegmentIndex,
        got: u64,
        expected: u64,
    },

    #[error("segment {exit}/{index}: server ignored range request (status {status})")]
    RangeRejected {
        exit: ExitId,
        index: SegmentIndex,
        status: u16,
    },

    #[error("downloaded {got} bytes, expected {expected}")]
    TotalLengthMismatch { got: u64, expected: u64 },

    #[error("sha256 mismatch: expected {expected}, got {got}")]
    HashMismatch { expected: String, got: String },

    #[error("invalid plan: {0}")]
    InvalidPlan(String),

    #[error("unknown exit `{0}` referenced in plan")]
    UnknownExit(ExitId),

    #[error("invalid part file name `{0}`")]
    CorruptPart(String),

    #[error("invalid sha256 hex string")]
    InvalidDigest,
}
