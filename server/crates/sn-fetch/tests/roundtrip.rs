//! Integration tests: probe, planning, parallel segmented download,
//! reassembly and hash verification against a local HTTP server — plus
//! the fallback path for range-hostile servers.

#![forbid(unsafe_code)]

mod common;

use std::num::{NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use sn_fetch::error::FetchError;
use sn_fetch::exec;
use sn_fetch::exit::LocalExit;
use sn_fetch::plan::{ExitSpecs, FetchPlan};
use sn_fetch::probe;
use sn_fetch::types::{
    ByteRange, ExitId, ExitWeight, FileSize, MinSegmentBytes, RangeSupport, SegmentsPerExit,
    Sha256Digest,
};

fn exit_specs(n: u8) -> ExitSpecs {
    (1..=n)
        .map(|i| {
            (
                ExitId::new(NonZeroU8::new(i).expect("test ids are 1..=255")),
                ExitWeight::new(NonZeroU32::new(1).expect("1 is non-zero")),
            )
        })
        .collect()
}

#[tokio::test]
async fn probe_detects_range_support() {
    let data = vec![42u8; 1024];
    let server = common::TestServer::start(data, true);
    let url = sn_fetch::types::TargetUrl::parse(&server.url("/file.bin")).expect("valid url");
    let client = reqwest::Client::new();

    let info = probe::probe(&client, &url).await.expect("probe succeeds");
    assert_eq!(info.range_support, RangeSupport::Supported);
    assert_eq!(info.content_length.map(|s| s.value()), Some(1024));
}

#[tokio::test]
async fn segmented_download_reassembles_and_hashes() {
    let data: Vec<u8> = (0..(2 * 1024 * 1024)).map(|i| (i % 251) as u8).collect();
    let expected_digest = Sha256Digest::from_bytes(Sha256::digest(&data).into());

    let server = common::TestServer::start(data.clone(), true);
    let url = sn_fetch::types::TargetUrl::parse(&server.url("/file.bin")).expect("valid url");

    let specs = exit_specs(2);
    let plan = FetchPlan::build(
        url.clone(),
        FileSize::try_new(data.len() as u64).expect("non-zero in test"),
        &specs,
        SegmentsPerExit::new(NonZeroUsize::new(2).expect("2 is non-zero")),
        MinSegmentBytes::new(NonZeroU64::new(1024).expect("1024 is non-zero")),
    )
    .expect("plan builds");
    plan.validate().expect("plan validates");

    let mut registry = sn_fetch::exit::ExitRegistry::empty();
    for id in specs.iter().map(|(id, _)| *id) {
        registry.register(Arc::new(LocalExit::new(id).expect("local exit")));
    }

    let parts = TempDir::new().expect("tempdir in test");
    let stats = exec::run_segments(&plan, &registry, parts.path())
        .await
        .expect("segments succeed");
    assert_eq!(stats.len(), 4);
    let total_downloaded: u64 = stats.iter().map(|s| s.bytes).sum();
    assert_eq!(total_downloaded, data.len() as u64);

    let output = parts.path().join("assembled.bin");
    let digest = exec::assemble(
        parts.path(),
        &output,
        FileSize::try_new(data.len() as u64).expect("non-zero in test"),
        Some(&expected_digest),
    )
    .await
    .expect("assembly verifies");

    assert_eq!(digest, expected_digest);
    let written = std::fs::read(&output).expect("output readable in test");
    assert_eq!(written, data);
}

#[tokio::test]
async fn range_hostile_server_falls_back_to_single_stream() {
    let data: Vec<u8> = (0..256_000u32).map(|i| (i % 197) as u8).collect();
    let server = common::TestServer::start(data.clone(), false);
    let url_str = server.url("/file.bin");
    let target = sn_fetch::types::TargetUrl::parse(&url_str).expect("valid url");

    let specs = exit_specs(1);
    let options = sn_fetch::runner::FetchOptions {
        output: std::env::temp_dir().join("sn-fetch-test-fallback.bin"),
        exits: specs.clone(),
        segments_per_exit: SegmentsPerExit::new(NonZeroUsize::new(1).expect("1 is non-zero")),
        min_segment: MinSegmentBytes::new(NonZeroU64::new(1024).expect("1024 is non-zero")),
        expected_sha256: None,
    };
    let registry = sn_fetch::runner::local_registry(&specs).expect("registry");
    let parts = TempDir::new().expect("tempdir in test");

    let report = sn_fetch::runner::run(&target, &options, &registry, parts.path())
        .await
        .expect("fallback download succeeds");

    assert_eq!(report.file_size.value(), 256_000);
    let written = std::fs::read(&options.output).expect("output readable in test");
    assert_eq!(written, data);
    let _ = std::fs::remove_file(&options.output);
}

#[test]
fn plan_is_contiguous_and_weighted() {
    let url = sn_fetch::types::TargetUrl::parse("http://example.com/file.bin").expect("valid url");
    let size = FileSize::try_new(10_000).expect("non-zero in test");
    let specs: ExitSpecs = vec![
        (
            ExitId::new(NonZeroU8::new(1).expect("1 is non-zero")),
            ExitWeight::new(NonZeroU32::new(3).expect("3 is non-zero")),
        ),
        (
            ExitId::new(NonZeroU8::new(2).expect("2 is non-zero")),
            ExitWeight::new(NonZeroU32::new(1).expect("1 is non-zero")),
        ),
    ];

    let plan = FetchPlan::build(
        url,
        size,
        &specs,
        SegmentsPerExit::new(NonZeroUsize::new(1).expect("1 is non-zero")),
        MinSegmentBytes::new(NonZeroU64::new(100).expect("100 is non-zero")),
    )
    .expect("plan builds");

    plan.validate().expect("plan validates");

    let exit1_total: u64 = plan
        .segments
        .iter()
        .filter(|seg| seg.exit.value() == 1)
        .map(|seg| seg.range.len())
        .sum();
    let exit2_total: u64 = plan
        .segments
        .iter()
        .filter(|seg| seg.exit.value() == 2)
        .map(|seg| seg.range.len())
        .sum();
    assert_eq!(exit1_total + exit2_total, 10_000);
    assert!(
        exit1_total > exit2_total,
        "heavier exit must get the larger share"
    );
}

#[test]
fn byte_range_http_value() {
    let range = ByteRange::try_new(100, 199).expect("valid range");
    assert_eq!(range.len(), 100);
    assert_eq!(range.http_value(), "bytes=100-199");
}

#[test]
fn invalid_url_is_rejected() {
    assert!(matches!(
        sn_fetch::types::TargetUrl::parse("ftp://example.com/file"),
        Err(FetchError::InvalidUrl(_))
    ));
}
