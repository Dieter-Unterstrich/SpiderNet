//! High-level orchestration: probe → plan → execute → assemble.
//!
//! Non-range-supporting servers fall back to a plain single-stream
//! download (no pooling gain, correct result).

use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::Digest;
use tokio::io::AsyncWriteExt;

use crate::error::FetchError;
use crate::exec::{self, FetchReport, RetryPolicy, SegmentStats};
use crate::exit::{ExitRegistry, HttpExit, LocalExit, MeshExit};
use crate::plan::ExitSpecs;
use crate::probe::{self, ProbeInfo};
use crate::progress::ProgressSink;
use crate::types::{
    Egress, FileSize, MinSegmentBytes, RangeSupport, SegmentsPerExit, Sha256Digest, TargetUrl,
};

/// All knobs for one fetch.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub output: PathBuf,
    pub exits: ExitSpecs,
    pub segments_per_exit: SegmentsPerExit,
    pub min_segment: MinSegmentBytes,
    pub expected_sha256: Option<Sha256Digest>,
    /// How often a segment is retried (preferably on another exit).
    pub retries: RetryPolicy,
}

/// Run a full fetch and produce a report.
pub async fn run(
    target: &TargetUrl,
    options: &FetchOptions,
    exits: &ExitRegistry,
    parts_dir: &Path,
) -> Result<FetchReport, FetchError> {
    let probe_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;
    let info = probe::probe(&probe_client, target).await?;

    match info.range_support {
        RangeSupport::Supported => {
            Box::pin(run_ranged(target, options, exits, parts_dir, info)).await
        }
        RangeSupport::Unsupported | RangeSupport::Unknown => {
            tracing::warn!(
                "server does not (confirmably) support range requests; \
                 falling back to single-stream download without pooling"
            );
            Box::pin(run_whole(target, options, exits, parts_dir, info)).await
        }
    }
}

/// Ranged path: probe must have told us the size; plan, execute, assemble.
async fn run_ranged(
    target: &TargetUrl,
    options: &FetchOptions,
    exits: &ExitRegistry,
    parts_dir: &Path,
    info: ProbeInfo,
) -> Result<FetchReport, FetchError> {
    let file_size = info.content_length.ok_or(FetchError::ZeroFileSize)?;

    let plan = crate::plan::FetchPlan::build(
        target.clone(),
        file_size,
        &options.exits,
        options.segments_per_exit,
        options.min_segment,
    )?;
    tracing::info!("plan: {} segment(s)", plan.segments.len());

    let started = Instant::now();
    let segment_stats: Vec<SegmentStats> = exec::run_segments_with_policy(
        &plan,
        exits,
        parts_dir,
        &options.retries,
        &ProgressSink::tracing_log(),
    )
    .await?
    .into_iter()
    .map(std::convert::From::from)
    .collect();
    let digest = exec::assemble(
        parts_dir,
        &options.output,
        file_size,
        options.expected_sha256.as_ref(),
    )
    .await?;
    let wall_time = started.elapsed();

    Ok(FetchReport {
        url: target.clone(),
        output: options.output.clone(),
        file_size,
        segments: segment_stats,
        wall_time,
        sha256: Some(digest),
    })
}

/// Fallback path: single stream, whole file.
async fn run_whole(
    target: &TargetUrl,
    options: &FetchOptions,
    exits: &ExitRegistry,
    _parts_dir: &Path, // symmetric signature; unused without segments
    info: ProbeInfo,
) -> Result<FetchReport, FetchError> {
    let exit_id = options
        .exits
        .first()
        .map(|(id, _, _)| *id)
        .ok_or_else(|| FetchError::InvalidPlan("no exits given".to_string()))?;
    let client = exits.get(exit_id)?.client().clone();

    let started = Instant::now();
    let response = client
        .get(target.inner().clone())
        .send()
        .await?
        .error_for_status()?;

    let mut file = tokio::fs::File::create(&options.output).await?;
    let mut hasher = sha2::Sha256::new();
    let mut written: u64 = 0;
    let mut response = response;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        written += chunk.len() as u64;
    }
    file.flush().await?;

    if let Some(expected_len) = info.content_length {
        if written != expected_len.value() {
            return Err(FetchError::TotalLengthMismatch {
                got: written,
                expected: expected_len.value(),
            });
        }
    }
    if written == 0 {
        return Err(FetchError::ZeroFileSize);
    }

    let digest = Sha256Digest::from_bytes(hasher.finalize().into());
    if let Some(expected) = options.expected_sha256.as_ref() {
        if digest != *expected {
            return Err(FetchError::HashMismatch {
                expected: expected.to_hex(),
                got: digest.to_hex(),
            });
        }
    }

    Ok(FetchReport {
        url: target.clone(),
        output: options.output.clone(),
        file_size: FileSize::try_new(written)?,
        segments: vec![SegmentStats {
            index: crate::types::SegmentIndex::new(0),
            bytes: written,
            duration: started.elapsed(),
        }],
        wall_time: started.elapsed(),
        sha256: Some(digest),
    })
}

/// Build a registry from exit specs: `Egress::Direct` exits connect
/// locally (own uplink), `Egress::Proxy` exits route through the given
/// proxy — typically a neighbor's exit service over the Yggdrasil
/// overlay.
pub fn registry_from_specs(specs: &ExitSpecs) -> Result<ExitRegistry, FetchError> {
    let mut registry = ExitRegistry::empty();
    for (id, _, egress) in specs {
        let exit: std::sync::Arc<dyn HttpExit> = match egress {
            Egress::Direct => std::sync::Arc::new(LocalExit::new(*id)?),
            Egress::Proxy(url) => std::sync::Arc::new(MeshExit::new(*id, url.clone())?),
        };
        registry.register(exit);
    }
    Ok(registry)
}
