//! Execution: download planned segments in parallel and reassemble the
//! target file.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::FetchError;
use crate::exit::ExitRegistry;
use crate::plan::FetchPlan;
use crate::types::{ByteRange, ExitId, FileSize, SegmentIndex, Sha256Digest, TargetUrl};

/// Statistics for one finished segment.
#[derive(Debug, Clone)]
pub struct SegmentStats {
    pub index: SegmentIndex,
    pub bytes: u64,
    pub duration: Duration,
}

/// Result of a complete fetch.
#[derive(Debug, Clone)]
pub struct FetchReport {
    pub url: TargetUrl,
    pub output: PathBuf,
    pub file_size: FileSize,
    pub segments: Vec<SegmentStats>,
    pub wall_time: Duration,
    pub sha256: Option<Sha256Digest>,
}

/// Download every planned segment in parallel. Each segment is written
/// to its own part file inside `parts_dir` (named `seg-NNNNNN.part`).
///
/// Failures abort the whole fetch: a partially assembled file is never
/// presented as success.
pub async fn run_segments(
    plan: &FetchPlan,
    exits: &ExitRegistry,
    parts_dir: &Path,
) -> Result<Vec<SegmentStats>, FetchError> {
    tokio::fs::create_dir_all(parts_dir).await?;

    let mut handles = Vec::with_capacity(plan.segments.len());
    for seg in &plan.segments {
        let client = exits.get(seg.exit)?.client().clone();
        handles.push(tokio::spawn(download_segment(
            plan.url.clone(),
            seg.exit,
            seg.index,
            seg.range,
            client,
            parts_dir.to_path_buf(),
        )));
    }

    let mut stats = Vec::with_capacity(handles.len());
    for handle in handles {
        let stats_i = handle
            .await
            .map_err(|err| FetchError::InvalidPlan(format!("segment task panicked: {err}")))??;
        stats.push(stats_i);
    }
    stats.sort_by_key(|s| s.index);
    Ok(stats)
}

/// Download one byte range and write it to `parts_dir/seg-NNNNNN.part`.
async fn download_segment(
    url: TargetUrl,
    exit: ExitId,
    index: SegmentIndex,
    range: ByteRange,
    client: reqwest::Client,
    parts_dir: PathBuf,
) -> Result<SegmentStats, FetchError> {
    let started = Instant::now();

    let part_path = part_file(&parts_dir, index);
    let mut response = client
        .get(url.inner().clone())
        .header(reqwest::header::RANGE, range.http_value())
        .send()
        .await?
        .error_for_status()?;

    match response.status() {
        reqwest::StatusCode::PARTIAL_CONTENT => {}
        status => {
            return Err(FetchError::RangeRejected {
                exit,
                index,
                status: status.as_u16(),
            });
        }
    }

    let mut file = tokio::fs::File::create(&part_path).await?;
    let mut written: u64 = 0;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    file.flush().await?;

    if written != range.len() {
        tokio::fs::remove_file(&part_path).await.ok();
        return Err(FetchError::SegmentLengthMismatch {
            index,
            got: written,
            expected: range.len(),
        });
    }

    Ok(SegmentStats {
        index,
        bytes: written,
        duration: started.elapsed(),
    })
}

/// Reassemble part files (in index order) into the output file while
/// streaming a SHA-256 over the content. Verifies the total length and,
/// if given, the expected digest. Returns the actual digest.
pub async fn assemble(
    parts_dir: &Path,
    output: &Path,
    expected_size: FileSize,
    expected_digest: Option<&Sha256Digest>,
) -> Result<Sha256Digest, FetchError> {
    let mut part_paths = collect_part_files(parts_dir).await?;
    part_paths.sort();

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    let mut output_file = tokio::fs::File::create(output).await?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;

    for part_path in &part_paths {
        let mut part = tokio::fs::File::open(part_path).await?;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let n = tokio::io::AsyncReadExt::read(&mut part, &mut buffer).await?;
            if n == 0 {
                break;
            }
            output_file.write_all(&buffer[..n]).await?;
            hasher.update(&buffer[..n]);
            total += n as u64;
        }
    }
    output_file.flush().await?;

    if total != expected_size.value() {
        return Err(FetchError::TotalLengthMismatch {
            got: total,
            expected: expected_size.value(),
        });
    }

    let digest = Sha256Digest::from_bytes(hasher.finalize().into());
    if let Some(expected) = expected_digest {
        if digest != *expected {
            return Err(FetchError::HashMismatch {
                expected: expected.to_hex(),
                got: digest.to_hex(),
            });
        }
    }
    Ok(digest)
}

/// Collect `seg-NNNNNN.part` files; anything else in `parts_dir` is an
/// error, so we never silently assemble garbage.
async fn collect_part_files(parts_dir: &Path) -> Result<Vec<PathBuf>, FetchError> {
    let mut out = Vec::new();
    let mut entries = tokio::fs::read_dir(parts_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let raw_name = entry.file_name();
        let name = raw_name
            .to_str()
            .ok_or_else(|| FetchError::CorruptPart("<non-utf8 file name>".to_string()))?;
        let middle = name
            .strip_prefix("seg-")
            .and_then(|n| n.strip_suffix(".part"))
            .ok_or_else(|| FetchError::CorruptPart(name.to_string()))?;
        middle
            .parse::<u32>()
            .map_err(|_| FetchError::CorruptPart(name.to_string()))?;
        out.push(entry.path());
    }
    Ok(out)
}

fn part_file(parts_dir: &Path, index: SegmentIndex) -> PathBuf {
    parts_dir.join(format!("seg-{:06}.part", index.value()))
}
