//! Execution: download planned segments in parallel (with per-segment
//! retry and exit reassignment) and reassemble the target file.
//!
//! API note: [`SegmentStats`] keeps its original shape and `run_segments`
//! keeps its signature, so existing callers (runner.rs constructs
//! `SegmentStats` literals directly) stay source-compatible. The
//! retry-aware path [`run_segments_with_policy`] returns
//! [`SegmentAttemptStats`] instead, which additionally records the exit
//! that served the successful attempt and how many attempts (retries
//! included) the segment needed; `run_segments` converts via `From`.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::FetchError;
use crate::exit::ExitRegistry;
use crate::plan::FetchPlan;
use crate::progress::{ProgressEvent, ProgressSink};
use crate::types::{ByteRange, ExitId, FileSize, SegmentIndex, Sha256Digest, TargetUrl};

/// Statistics for one finished segment.
#[derive(Debug, Clone)]
pub struct SegmentStats {
    pub index: SegmentIndex,
    pub bytes: u64,
    pub duration: Duration,
}

/// Retry behaviour for segment downloads.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Total tries per segment; attempt 1 is the initial try.
    pub max_attempts: NonZeroU32,
}

impl RetryPolicy {
    #[must_use]
    pub const fn new(max_attempts: NonZeroU32) -> Self {
        Self { max_attempts }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: NonZeroU32::new(3).unwrap_or(NonZeroU32::MIN),
        }
    }
}

/// Statistics for one finished segment, including retry information.
/// `duration` spans all attempts (retries included); `exit` is the exit
/// that served the successful attempt.
#[derive(Debug, Clone)]
pub struct SegmentAttemptStats {
    pub index: SegmentIndex,
    pub exit: ExitId,
    pub bytes: u64,
    pub duration: Duration,
    pub attempts: u32,
}

impl From<SegmentAttemptStats> for SegmentStats {
    fn from(stats: SegmentAttemptStats) -> Self {
        SegmentStats {
            index: stats.index,
            bytes: stats.bytes,
            duration: stats.duration,
        }
    }
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
    let stats = run_segments_with_policy(
        plan,
        exits,
        parts_dir,
        &RetryPolicy::default(),
        &ProgressSink::noop(),
    )
    .await?;
    Ok(stats.into_iter().map(SegmentStats::from).collect())
}

/// Download every planned segment in parallel, retrying failed segments
/// according to `policy` — preferably on a different exit — and report
/// progress to `progress`. Each segment is written to its own part file
/// inside `parts_dir` (named `seg-NNNNNN.part`); the part file of a
/// failed attempt is removed before the next attempt.
///
/// Failures abort the whole fetch: a segment that exhausts all attempts
/// fails the fetch with [`FetchError::SegmentAttemptsExhausted`], a
/// non-retryable failure aborts immediately. A partially assembled file
/// is never presented as success.
pub async fn run_segments_with_policy(
    plan: &FetchPlan,
    exits: &ExitRegistry,
    parts_dir: &Path,
    policy: &RetryPolicy,
    progress: &ProgressSink,
) -> Result<Vec<SegmentAttemptStats>, FetchError> {
    tokio::fs::create_dir_all(parts_dir).await?;

    // Validate that every planned exit exists, then snapshot the
    // clients of all registered exits: tasks must fetch clients by id
    // without borrowing the registry ('static spawn requirement).
    for seg in &plan.segments {
        exits.get(seg.exit)?;
    }
    let mut clients: BTreeMap<ExitId, reqwest::Client> = BTreeMap::new();
    for id in exits.ids() {
        clients.insert(id, exits.get(id)?.client().clone());
    }
    let clients = Arc::new(clients);
    let selector = Arc::new(ExitSelector::new(exits));

    let mut handles = Vec::with_capacity(plan.segments.len());
    for seg in &plan.segments {
        let task = SegmentTask {
            url: plan.url.clone(),
            index: seg.index,
            range: seg.range,
            planned_exit: seg.exit,
            clients: Arc::clone(&clients),
            selector: Arc::clone(&selector),
            parts_dir: parts_dir.to_path_buf(),
            policy: *policy,
            progress: progress.clone(),
        };
        handles.push(tokio::spawn(download_with_retry(task)));
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

/// Everything one segment task needs, bundled to keep the task
/// signature small.
struct SegmentTask {
    url: TargetUrl,
    index: SegmentIndex,
    range: ByteRange,
    planned_exit: ExitId,
    clients: Arc<BTreeMap<ExitId, reqwest::Client>>,
    selector: Arc<ExitSelector>,
    parts_dir: PathBuf,
    policy: RetryPolicy,
    progress: ProgressSink,
}

/// Download one segment with up to `policy.max_attempts` attempts,
/// reassigning to a different exit after a retryable failure (while the
/// registry offers more than one exit).
async fn download_with_retry(task: SegmentTask) -> Result<SegmentAttemptStats, FetchError> {
    let SegmentTask {
        url,
        index,
        range,
        planned_exit,
        clients,
        selector,
        parts_dir,
        policy,
        progress,
    } = task;
    let max_attempts = policy.max_attempts.get();
    let started = Instant::now();
    let mut exit = planned_exit;
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let client = clients
            .get(&exit)
            .ok_or(FetchError::UnknownExit(exit))?
            .clone();
        progress.on_event(&ProgressEvent::SegmentStarted {
            index,
            exit,
            attempt,
        });
        tracing::debug!(segment = index.value(), exit = %exit, attempt, "segment attempt started");

        match download_attempt(&url, exit, index, range, client, &parts_dir).await {
            Ok(bytes) => {
                let duration = started.elapsed();
                progress.on_event(&ProgressEvent::SegmentFinished {
                    index,
                    exit,
                    bytes,
                    duration,
                });
                tracing::info!(
                    segment = index.value(),
                    exit = %exit,
                    attempts = attempt,
                    bytes,
                    "segment finished"
                );
                return Ok(SegmentAttemptStats {
                    index,
                    exit,
                    bytes,
                    duration,
                    attempts: attempt,
                });
            }
            Err(err) => {
                let retryable = is_retryable(&err);
                let err_text = err.to_string();
                progress.on_event(&ProgressEvent::SegmentFailed {
                    index,
                    exit,
                    attempt,
                    error: err_text,
                });
                remove_part_file(&parts_dir, index).await;

                if !retryable {
                    tracing::warn!(
                        segment = index.value(),
                        exit = %exit,
                        attempt,
                        "segment failed with non-retryable error: {err}"
                    );
                    return Err(err);
                }
                selector.mark_failed(exit);
                if attempt >= max_attempts {
                    tracing::warn!(
                        segment = index.value(),
                        attempts = attempt,
                        "segment failed after all attempts were exhausted: {err}"
                    );
                    return Err(FetchError::SegmentAttemptsExhausted {
                        index,
                        attempts: attempt,
                        last_error: Box::new(err),
                    });
                }
                tracing::warn!(
                    segment = index.value(),
                    exit = %exit,
                    attempt,
                    "segment attempt failed; retrying: {err}"
                );
                exit = selector.pick_retry(exit);
            }
        }
    }
}

/// Retryable: transport-level failures (network, timeouts), 5xx
/// responses and length mismatches. Non-retryable: 4xx range rejections
/// (e.g. 416), corrupt parts, plan or digest errors — retrying those on
/// another exit cannot succeed.
fn is_retryable(error: &FetchError) -> bool {
    match error {
        FetchError::Http(err) => {
            !err.is_status() || err.status().is_some_and(|status| status.is_server_error())
        }
        FetchError::Io(_) | FetchError::SegmentLengthMismatch { .. } => true,
        FetchError::RangeRejected { status, .. } => *status >= 500,
        FetchError::InvalidUrl(_)
        | FetchError::ZeroFileSize
        | FetchError::TotalLengthMismatch { .. }
        | FetchError::HashMismatch { .. }
        | FetchError::InvalidPlan(_)
        | FetchError::UnknownExit(_)
        | FetchError::CorruptPart(_)
        | FetchError::InvalidDigest
        | FetchError::SegmentAttemptsExhausted { .. } => false,
    }
}

/// Download one byte range and write it to `parts_dir/seg-NNNNNN.part`.
/// Returns the number of bytes written. A failed attempt can leave a
/// partial part file behind; the retry loop removes it before the next
/// attempt.
async fn download_attempt(
    url: &TargetUrl,
    exit: ExitId,
    index: SegmentIndex,
    range: ByteRange,
    client: reqwest::Client,
    parts_dir: &Path,
) -> Result<u64, FetchError> {
    let part_path = part_file(parts_dir, index);
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
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(FetchError::SegmentLengthMismatch {
            index,
            got: written,
            expected: range.len(),
        });
    }

    Ok(written)
}

/// Chooses exits for retry attempts across all segment tasks of one
/// run. Preference: the exit that failed least recently (exits that
/// never failed first, ties broken by ascending id). The exit an
/// attempt just failed on is avoided while at least one other exit
/// exists; with a single-exit registry, retries stay on that exit.
#[derive(Debug)]
struct ExitSelector {
    ids: Vec<ExitId>,
    last_failed: Mutex<BTreeMap<ExitId, Instant>>,
}

impl ExitSelector {
    fn new(exits: &ExitRegistry) -> Self {
        Self {
            ids: exits.ids(),
            last_failed: Mutex::new(BTreeMap::new()),
        }
    }

    fn mark_failed(&self, id: ExitId) {
        let mut guard = self.lock();
        guard.insert(id, Instant::now());
    }

    fn pick_retry(&self, avoid: ExitId) -> ExitId {
        let guard = self.lock();
        let mut best: Option<ExitId> = None;
        for &id in &self.ids {
            if id == avoid {
                continue;
            }
            best = Some(match best {
                None => id,
                Some(current) => {
                    let prefer_id = match (guard.get(&current), guard.get(&id)) {
                        // tie or current never failed: keep the lower id
                        (None, None | Some(_)) => false,
                        (Some(_), None) => true, // id never failed: better
                        (Some(current_failed), Some(id_failed)) => *id_failed < *current_failed,
                    };
                    if prefer_id {
                        id
                    } else {
                        current
                    }
                }
            });
        }
        best.unwrap_or(avoid) // single-exit registry: retry on the same exit
    }

    /// std mutexes can be poisoned by a panicking holder; since the
    /// map is only advisory (exit preference), keep using its contents.
    fn lock(&self) -> MutexGuard<'_, BTreeMap<ExitId, Instant>> {
        match self.last_failed.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
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

/// Best-effort removal of a failed attempt's part file.
async fn remove_part_file(parts_dir: &Path, index: SegmentIndex) {
    let _ = tokio::fs::remove_file(part_file(parts_dir, index)).await;
}

fn part_file(parts_dir: &Path, index: SegmentIndex) -> PathBuf {
    parts_dir.join(format!("seg-{:06}.part", index.value()))
}
