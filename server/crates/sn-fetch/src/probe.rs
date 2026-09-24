//! Probing: find out whether a server supports HTTP range requests and
//! how large the object is, before planning a segmented download.

use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH};
use reqwest::StatusCode;

use crate::error::FetchError;
use crate::types::{FileSize, RangeSupport, TargetUrl};

/// What a HEAD request (plus fallback range probe) told us about the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeInfo {
    pub range_support: RangeSupport,
    pub content_length: Option<FileSize>,
}

/// Probe the target: HEAD first; if the `Accept-Ranges` header is absent
/// or inconclusive, fall back to a 1-byte GET with a `Range` header and
/// check for `206 Partial Content`.
pub async fn probe(client: &reqwest::Client, url: &TargetUrl) -> Result<ProbeInfo, FetchError> {
    let head = client
        .head(url.inner().clone())
        .send()
        .await?
        .error_for_status()?;

    let content_length = head
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let advertised = match head
        .headers()
        .get(ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
    {
        Some("bytes") => Some(RangeSupport::Supported),
        Some("none") => Some(RangeSupport::Unsupported),
        _ => None,
    };

    let range_support = match advertised {
        Some(support) => support,
        // HEAD was inconclusive: ask directly with a 1-byte range.
        None => {
            let resp = client
                .get(url.inner().clone())
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await?;

            match resp.status() {
                StatusCode::PARTIAL_CONTENT => RangeSupport::Supported,
                StatusCode::OK => RangeSupport::Unsupported,
                _ => RangeSupport::Unknown,
            }
        }
    };

    Ok(ProbeInfo {
        range_support,
        content_length: content_length.and_then(|len| FileSize::try_new(len).ok()),
    })
}
