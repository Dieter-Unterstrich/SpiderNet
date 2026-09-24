//! Domain types: newtypes over raw primitives, validated at construction.

use std::fmt;
use std::num::{NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};

use crate::error::FetchError;

/// Identifier of an exit (an internet uplink offered by a household).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExitId(NonZeroU8);

impl ExitId {
    pub const fn new(value: NonZeroU8) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u8 {
        self.0.get()
    }
}

impl fmt::Display for ExitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exit-{}", self.0.get())
    }
}

/// Relative capacity share of an exit. 1 means equal share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitWeight(NonZeroU32);

impl ExitWeight {
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0.get()
    }
}

/// How many sub-segments each exit's share is split into (parallel
/// connections per exit; more connections can defeat per-connection
/// server throttling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentsPerExit(NonZeroUsize);

impl SegmentsPerExit {
    pub const fn new(value: NonZeroUsize) -> Self {
        Self(value)
    }

    pub const fn value(self) -> usize {
        self.0.get()
    }
}

/// Lower bound for a segment size, to avoid absurdly tiny segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinSegmentBytes(NonZeroU64);

impl MinSegmentBytes {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0.get()
    }
}

/// Total size of a downloadable object, validated to be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSize(u64);

impl FileSize {
    pub fn try_new(value: u64) -> Result<Self, FetchError> {
        if value == 0 {
            return Err(FetchError::ZeroFileSize);
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A half-open-by-one byte range `[start, end_inclusive]`, as HTTP uses
/// inclusive end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    start: u64,
    end_inclusive: u64,
}

impl ByteRange {
    pub fn try_new(start: u64, end_inclusive: u64) -> Result<Self, FetchError> {
        if end_inclusive < start {
            return Err(FetchError::InvalidPlan(format!(
                "range start {start} is after end {end_inclusive}"
            )));
        }
        Ok(Self {
            start,
            end_inclusive,
        })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end_inclusive(self) -> u64 {
        self.end_inclusive
    }

    pub const fn len(self) -> u64 {
        self.end_inclusive - self.start + 1
    }

    pub const fn is_empty(self) -> bool {
        false
    }

    /// Value for the HTTP `Range` header, e.g. `bytes=100-199`.
    pub fn http_value(self) -> String {
        format!("bytes={}-{}", self.start, self.end_inclusive)
    }
}

impl fmt::Display for ByteRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}..={} ({} bytes)",
            self.start,
            self.end_inclusive,
            self.len()
        )
    }
}

/// Position of a segment inside a plan, sequential from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SegmentIndex(u32);

impl SegmentIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for SegmentIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A validated SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self, FetchError> {
        let bytes = hex::decode(hex_str.trim()).map_err(|_| FetchError::InvalidDigest)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| FetchError::InvalidDigest)?;
        Ok(Self(arr))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// A validated target URL. Only http(s) is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUrl(reqwest::Url);

impl TargetUrl {
    pub fn parse(input: &str) -> Result<Self, FetchError> {
        let url =
            reqwest::Url::parse(input).map_err(|_| FetchError::InvalidUrl(input.to_string()))?;
        match url.scheme() {
            "http" | "https" => Ok(Self(url)),
            _ => Err(FetchError::InvalidUrl(input.to_string())),
        }
    }

    pub fn inner(&self) -> &reqwest::Url {
        &self.0
    }
}

impl fmt::Display for TargetUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Result of probing a server's range-request support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeSupport {
    /// Server advertised `Accept-Ranges: bytes` or answered a range probe
    /// with `206 Partial Content`.
    Supported,
    /// Server advertised `Accept-Ranges: none` or answered a range probe
    /// with a full `200` response.
    Unsupported,
    /// Probe was inconclusive; treat as unsupported (safe fallback).
    Unknown,
}

/// How a request physically leaves through an exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Egress {
    /// The household's own uplink: plain outgoing HTTP, no detour.
    Direct,
    /// Through an HTTP proxy — e.g. a neighbor's exit service reachable
    /// over the mesh overlay (typically bound to their Yggdrasil
    /// address).
    Proxy(reqwest::Url),
}

impl Egress {
    /// Parse a proxy URL for [`Egress::Proxy`]; only `http`/`https`
    /// upstreams are accepted.
    pub fn proxy(input: &str) -> Result<Self, FetchError> {
        let url =
            reqwest::Url::parse(input).map_err(|_| FetchError::InvalidUrl(input.to_string()))?;
        match url.scheme() {
            "http" | "https" => Ok(Self::Proxy(url)),
            _ => Err(FetchError::InvalidUrl(input.to_string())),
        }
    }
}

impl fmt::Display for Egress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::Proxy(url) => write!(f, "proxy:{url}"),
        }
    }
}
