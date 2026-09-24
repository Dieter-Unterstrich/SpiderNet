//! Retry behaviour against a local HTTP server: a flaky first attempt
//! (retried, reassigned to another exit), a consistently failing exit
//! (reassigned), retry exhaustion (whole fetch fails, no output file)
//! and a non-retryable status (immediate abort).

#![forbid(unsafe_code)]
// Tests are allowed to unwrap/expect; production code denies them via
// workspace lints (server/Cargo.toml).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::num::{NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};
use std::path::Path;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use sn_fetch::error::FetchError;
use sn_fetch::exec::{self, RetryPolicy};
use sn_fetch::exit::{ExitRegistry, LocalExit};
use sn_fetch::plan::{ExitSpecs, FetchPlan};
use sn_fetch::progress::{BytesTracker, ProgressEvent, ProgressSink};
use sn_fetch::types::{
    Egress, ExitId, ExitWeight, FileSize, MinSegmentBytes, SegmentsPerExit, TargetUrl,
};

use common::{BrokenProxyExit, FailMode, ServerConfig, TestServer};

fn spec(id: u8) -> (ExitId, ExitWeight, Egress) {
    (
        ExitId::new(NonZeroU8::new(id).expect("test ids are 1..=255")),
        ExitWeight::new(NonZeroU32::new(1).expect("1 is non-zero")),
        Egress::Direct,
    )
}

fn exit_id(id: u8) -> ExitId {
    ExitId::new(NonZeroU8::new(id).expect("test ids are 1..=255"))
}

fn local_registry(specs: &ExitSpecs) -> ExitRegistry {
    let mut registry = ExitRegistry::empty();
    for (id, _, _) in specs {
        registry.register(Arc::new(LocalExit::new(*id).expect("local exit")));
    }
    registry
}

/// Single-exit plan + registry, for exhaustion tests.
fn single_exit_setup(
    url: &TargetUrl,
    file_size: u64,
    min_segment: u64,
) -> (FetchPlan, ExitRegistry) {
    let specs: ExitSpecs = vec![spec(1)];
    let plan = FetchPlan::build(
        url.clone(),
        FileSize::try_new(file_size).expect("non-zero in test"),
        &specs,
        SegmentsPerExit::new(NonZeroUsize::MIN),
        MinSegmentBytes::new(NonZeroU64::new(min_segment).expect("non-zero in test")),
    )
    .expect("plan builds");
    let registry = local_registry(&specs);
    (plan, registry)
}

type RecordedEvents = Arc<Mutex<Vec<ProgressEvent>>>;

fn recording_sink() -> (ProgressSink, RecordedEvents) {
    let events: RecordedEvents = Arc::new(Mutex::new(Vec::new()));
    let sink = ProgressSink::new({
        let events = Arc::clone(&events);
        move |event| {
            events
                .lock()
                .expect("mutex poisoned in test")
                .push(event.clone())
        }
    });
    (sink, events)
}

fn failed_events(events: &RecordedEvents) -> Vec<(u32, u32, String)> {
    events
        .lock()
        .expect("mutex poisoned in test")
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::SegmentFailed {
                index,
                exit: _,
                attempt,
                error,
            } => Some((index.value(), *attempt, error.clone())),
            _ => None,
        })
        .collect()
}

/// Names of leftover part files in `parts_dir`.
fn part_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("parts dir readable in test")
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn flaky_segment_retried_on_other_exit() {
    let data: Vec<u8> = (0..(2 * 1024 * 1024)).map(|i| (i % 253) as u8).collect();
    let server = TestServer::start_with(
        data.clone(),
        true,
        ServerConfig {
            fail_mode: FailMode::FirstN(1),
        },
    );
    let url = TargetUrl::parse(&server.url("/file.bin")).expect("valid url");

    let specs: ExitSpecs = vec![spec(1), spec(2)];
    let plan = FetchPlan::build(
        url,
        FileSize::try_new(data.len() as u64).expect("non-zero in test"),
        &specs,
        SegmentsPerExit::new(NonZeroUsize::new(2).expect("2 is non-zero")),
        MinSegmentBytes::new(NonZeroU64::new(1024).expect("1024 is non-zero")),
    )
    .expect("plan builds");
    let registry = local_registry(&specs);

    let parts = TempDir::new().expect("tempdir in test");
    let (progress, events) = recording_sink();
    let tracker = BytesTracker::new();
    let combined = ProgressSink::fanout(&progress, &tracker.sink());

    let stats = exec::run_segments_with_policy(
        &plan,
        &registry,
        parts.path(),
        &RetryPolicy::default(),
        &combined,
    )
    .await
    .expect("fetch succeeds despite one flaky attempt");

    assert_eq!(stats.len(), plan.segments.len());
    let total: u64 = stats.iter().map(|s| s.bytes).sum();
    assert_eq!(total, data.len() as u64);
    assert_eq!(
        tracker.total(),
        data.len() as u64,
        "tracker must see all finished bytes"
    );

    let retried: Vec<_> = stats.iter().filter(|s| s.attempts > 1).collect();
    assert_eq!(retried.len(), 1, "exactly one segment needed a retry");
    let retried = retried[0];
    let planned_exit = plan
        .segments
        .iter()
        .find(|seg| seg.index == retried.index)
        .expect("planned segment")
        .exit;
    assert_ne!(retried.exit, planned_exit, "retry must use another exit");
    assert_eq!(retried.attempts, 2);

    let started: Vec<(u8, u32)> = events
        .lock()
        .expect("mutex poisoned in test")
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::SegmentStarted {
                index,
                exit,
                attempt,
            } if index == &retried.index => Some((exit.value(), *attempt)),
            _ => None,
        })
        .collect();
    assert_eq!(
        started,
        vec![(planned_exit.value(), 1), (retried.exit.value(), 2)],
        "the retried segment must start once per attempt, on different exits"
    );
    let failed = failed_events(&events);
    assert_eq!(
        failed,
        vec![(retried.index.value(), 1, failed[0].2.clone())],
        "exactly one failed attempt, recorded on attempt 1"
    );
    assert!(
        failed[0].2.contains("500"),
        "failure reason should mention the 500 status: {}",
        failed[0].2
    );
}

#[tokio::test]
async fn exhausted_retries_fail_without_output() {
    let server = TestServer::start_with(
        vec![7u8; 32 * 1024],
        true,
        ServerConfig {
            fail_mode: FailMode::Always,
        },
    );
    let url = TargetUrl::parse(&server.url("/file.bin")).expect("valid url");
    let (plan, registry) = single_exit_setup(&url, 32 * 1024, 32 * 1024);
    assert_eq!(plan.segments.len(), 1);

    let parts = TempDir::new().expect("tempdir in test");
    let (progress, events) = recording_sink();
    let tracker = BytesTracker::new();
    let combined = ProgressSink::fanout(&progress, &tracker.sink());

    let result = exec::run_segments_with_policy(
        &plan,
        &registry,
        parts.path(),
        &RetryPolicy::default(),
        &combined,
    )
    .await;

    let err = result.expect_err("fetch must fail when every attempt fails");
    assert!(
        matches!(
            err,
            FetchError::SegmentAttemptsExhausted { attempts: 3, .. }
        ),
        "unexpected error: {err}"
    );
    assert_eq!(
        tracker.total(),
        0,
        "nothing finished, tracker stays at zero"
    );

    let failed: Vec<u32> = failed_events(&events)
        .into_iter()
        .map(|(_, attempt, error)| {
            assert!(error.contains("500"), "failure reason: {error}");
            attempt
        })
        .collect();
    assert_eq!(failed, vec![1, 2, 3], "default policy means three attempts");

    assert!(
        part_files(parts.path()).is_empty(),
        "failed part files must be cleaned up between attempts"
    );
    let output = parts.path().join("output.bin");
    assert!(
        !output.exists(),
        "no output file may be created for a failed fetch"
    );
}

#[tokio::test]
async fn single_attempt_policy_fails_fast() {
    let server = TestServer::start_with(
        vec![5u8; 16 * 1024],
        true,
        ServerConfig {
            fail_mode: FailMode::Always,
        },
    );
    let url = TargetUrl::parse(&server.url("/file.bin")).expect("valid url");
    let (plan, registry) = single_exit_setup(&url, 16 * 1024, 16 * 1024);

    let parts = TempDir::new().expect("tempdir in test");
    let (progress, events) = recording_sink();
    let policy = RetryPolicy::new(NonZeroU32::new(1).expect("1 is non-zero"));

    let result =
        exec::run_segments_with_policy(&plan, &registry, parts.path(), &policy, &progress).await;

    let err = result.expect_err("fetch must fail after the single attempt");
    assert!(
        matches!(
            err,
            FetchError::SegmentAttemptsExhausted { attempts: 1, .. }
        ),
        "unexpected error: {err}"
    );
    assert_eq!(failed_events(&events).len(), 1);
    assert!(part_files(parts.path()).is_empty());
}

#[tokio::test]
async fn failing_exit_reassigned_to_working_exit() {
    let data: Vec<u8> = (0..(1024 * 1024)).map(|i| (i % 241) as u8).collect();
    let expected_digest = sn_fetch::types::Sha256Digest::from_bytes(Sha256::digest(&data).into());
    let server = TestServer::start(data.clone(), true);
    let url = TargetUrl::parse(&server.url("/file.bin")).expect("valid url");

    let specs: ExitSpecs = vec![spec(1), spec(2)];
    let plan = FetchPlan::build(
        url,
        FileSize::try_new(data.len() as u64).expect("non-zero in test"),
        &specs,
        SegmentsPerExit::new(NonZeroUsize::MIN),
        MinSegmentBytes::new(NonZeroU64::new(1024).expect("1024 is non-zero")),
    )
    .expect("plan builds");

    let mut registry = ExitRegistry::empty();
    registry.register(Arc::new(BrokenProxyExit::new(exit_id(1))));
    registry.register(Arc::new(LocalExit::new(exit_id(2)).expect("local exit")));

    let parts = TempDir::new().expect("tempdir in test");
    let (progress, events) = recording_sink();

    let stats = exec::run_segments_with_policy(
        &plan,
        &registry,
        parts.path(),
        &RetryPolicy::default(),
        &progress,
    )
    .await
    .expect("fetch succeeds by reassigning off the broken exit");

    for stat in &stats {
        let planned = plan
            .segments
            .iter()
            .find(|seg| seg.index == stat.index)
            .expect("planned segment");
        if planned.exit.value() == 1 {
            assert_eq!(
                stat.attempts, 2,
                "segment planned on the broken exit needs exactly one retry"
            );
            assert_eq!(stat.exit.value(), 2, "retry must land on the working exit");
        } else {
            assert_eq!(stat.attempts, 1);
            assert_eq!(stat.exit.value(), 2);
        }
    }

    let started_exits: Vec<u8> = events
        .lock()
        .expect("mutex poisoned in test")
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::SegmentStarted { index, exit, .. } if index.value() == 0 => {
                Some(exit.value())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        started_exits,
        vec![1, 2],
        "segment 0 must be tried on the broken exit first, then reassigned"
    );

    let output = parts.path().join("out.bin");
    let digest = exec::assemble(
        parts.path(),
        &output,
        FileSize::try_new(data.len() as u64).expect("non-zero in test"),
        Some(&expected_digest),
    )
    .await
    .expect("assembly verifies");
    assert_eq!(digest, expected_digest);
}

#[tokio::test]
async fn non_retryable_status_aborts_immediately() {
    let server = TestServer::start_with(
        vec![3u8; 32 * 1024],
        true,
        ServerConfig {
            fail_mode: FailMode::AlwaysStatus(416),
        },
    );
    let url = TargetUrl::parse(&server.url("/file.bin")).expect("valid url");
    let (plan, registry) = single_exit_setup(&url, 32 * 1024, 32 * 1024);

    let parts = TempDir::new().expect("tempdir in test");
    let (progress, events) = recording_sink();

    let result = exec::run_segments_with_policy(
        &plan,
        &registry,
        parts.path(),
        &RetryPolicy::default(),
        &progress,
    )
    .await;

    let err = result.expect_err("416 must abort the fetch");
    assert!(
        matches!(
            err,
            FetchError::Http(ref http_err)
                if http_err.status() == Some(reqwest::StatusCode::RANGE_NOT_SATISFIABLE)
        ),
        "the raw 416 must surface unchanged, got: {err}"
    );

    // Non-retryable: one started attempt, one failure event, no retry.
    let guard = events.lock().expect("mutex poisoned in test");
    let started = guard
        .iter()
        .filter(|event| matches!(event, ProgressEvent::SegmentStarted { .. }))
        .count();
    assert_eq!(started, 1, "non-retryable failure must not be retried");
    let failed = guard
        .iter()
        .filter(|event| matches!(event, ProgressEvent::SegmentFailed { .. }))
        .count();
    assert_eq!(failed, 1);
    drop(guard);

    assert!(
        part_files(parts.path()).is_empty(),
        "failed part files must be cleaned up"
    );
}
