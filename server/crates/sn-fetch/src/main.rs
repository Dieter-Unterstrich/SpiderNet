//! CLI entry point for the sn-fetch `PoC`.
//!
//! KEINE GARANTIE — this software is provided "as is" without warranty of
//! any kind (AGPL-3.0, sections 15/16). Users are solely responsible for
//! what they transmit or receive with it.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

use std::num::{NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize};
use std::path::PathBuf;

use clap::Parser;
use sn_fetch::runner::{self, FetchOptions};
use sn_fetch::types::{
    Egress, ExitId, ExitWeight, MinSegmentBytes, SegmentsPerExit, Sha256Digest, TargetUrl,
};

#[derive(Debug, Parser)]
#[command(name = "sn-fetch", about = "SpiderNet segmented downloader (PoC)")]
struct Args {
    /// URL of the file to download (http/https).
    url: String,

    /// Output file path (default: file name from the URL, in cwd).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Exits as `id=weight` pairs, comma separated, e.g. `1=2,2=1`.
    /// A proxy route turns an exit into a neighbor's uplink over the
    /// mesh overlay: `id=weight@http://[ygg-addr]:8080`. Without a
    /// proxy, the exit is this machine's own connection.
    /// `PoC`: weights only shape the plan; a real proxy service
    /// (`sn-exit`) comes with the mesh integration.
    #[arg(short = 'e', long = "exit", default_value = "1=1")]
    exits: String,

    /// Total tries per segment (initial try + retries; failed segments
    /// are retried on a different exit when available).
    #[arg(long, default_value_t = 3)]
    max_attempts: u32,

    /// Parallel sub-segments per exit.
    #[arg(short = 's', long, default_value_t = 1)]
    segments_per_exit: usize,

    /// Minimum segment size in MiB (smaller segments get merged).
    #[arg(long, default_value_t = 4)]
    min_segment_mib: u64,

    /// Expected SHA-256 of the complete file (hex).
    #[arg(long)]
    sha256: Option<String>,

    /// Only probe the server and print what we learned.
    #[arg(long)]
    probe_only: bool,
}

fn parse_exits(spec: &str) -> Result<Vec<(ExitId, ExitWeight, Egress)>, String> {
    spec.split(',')
        .map(|pair| {
            let (id, rest) = pair
                .split_once('=')
                .ok_or_else(|| format!("invalid exit spec `{pair}`, expected `id=weight`"))?;
            let id = id
                .trim()
                .parse::<u8>()
                .ok()
                .and_then(NonZeroU8::new)
                .ok_or_else(|| format!("invalid exit id `{id}` (must be a non-zero number)"))?;
            let (weight, egress) = match rest.split_once('@') {
                Some((weight, proxy)) => {
                    let egress = Egress::proxy(proxy.trim())
                        .map_err(|err| format!("invalid proxy in exit spec `{pair}`: {err}"))?;
                    (weight, egress)
                }
                None => (rest, Egress::Direct),
            };
            let weight = weight
                .trim()
                .parse::<u32>()
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or_else(|| format!("invalid exit weight `{weight}` (must be non-zero)"))?;
            Ok((ExitId::new(id), ExitWeight::new(weight), egress))
        })
        .collect()
}

fn default_output(url: &TargetUrl) -> PathBuf {
    let mut segments = match url.inner().path_segments() {
        Some(segments) => segments.clone(),
        None => return PathBuf::from("download.bin"),
    };
    let name = segments
        .next_back()
        .filter(|name| !name.is_empty())
        .map_or_else(|| "download.bin".to_string(), str::to_string);
    PathBuf::from(name)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    if let Err(report) = run().await {
        eprintln!("NO WARRANTY — SpiderNet sn-fetch PoC (AGPL-3.0, no liability)");
        eprintln!("error: {report}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();
    let target = TargetUrl::parse(&args.url).map_err(|err| err.to_string())?;

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| err.to_string())?;

    if args.probe_only {
        let info = sn_fetch::probe::probe(&client, &target)
            .await
            .map_err(|err| err.to_string())?;
        println!("url:            {target}");
        println!("range support:  {:?}", info.range_support);
        match info.content_length {
            Some(size) => println!("content-length: {size} bytes"),
            None => println!("content-length: unknown"),
        }
        return Ok(());
    }

    let exit_specs = parse_exits(&args.exits)?;
    let spe =
        NonZeroUsize::new(args.segments_per_exit).ok_or("segments_per_exit must be at least 1")?;
    let min_segment = MinSegmentBytes::new(
        NonZeroU64::new(args.min_segment_mib.saturating_mul(1024 * 1024))
            .ok_or("min_segment_mib must be at least 1")?,
    );
    let max_attempts =
        NonZeroU32::new(args.max_attempts).ok_or("max_attempts must be at least 1")?;

    let expected = match &args.sha256 {
        Some(hex) => Some(Sha256Digest::from_hex(hex).map_err(|err| err.to_string())?),
        None => None,
    };

    let options = FetchOptions {
        output: args
            .output
            .clone()
            .unwrap_or_else(|| default_output(&target)),
        exits: exit_specs.clone(),
        segments_per_exit: SegmentsPerExit::new(spe),
        min_segment,
        expected_sha256: expected,
        retries: sn_fetch::exec::RetryPolicy::new(max_attempts),
    };

    let registry = runner::registry_from_specs(&exit_specs).map_err(|err| err.to_string())?;
    let parts_dir = std::env::temp_dir().join("sn-fetch-parts");

    let report = runner::run(&target, &options, &registry, &parts_dir)
        .await
        .map_err(|err| err.to_string())?;

    println!("NO WARRANTY — SpiderNet sn-fetch PoC (AGPL-3.0, no liability)");
    println!();
    println!("url:      {}", report.url);
    println!("output:   {}", report.output.display());
    println!("size:     {} bytes", report.file_size);
    match report.sha256 {
        Some(digest) => println!("sha256:   {digest}"),
        None => println!("sha256:   not computed"),
    }
    println!(
        "time:     {:.2?} s → effective {:.1} MiB/s",
        report.wall_time.as_secs_f64(),
        bytes_to_mib(report.file_size.value()) / report.wall_time.as_secs_f64()
    );
    println!("segments:");
    for segment in &report.segments {
        let mib_per_s =
            bytes_to_mib(segment.bytes) / segment.duration.as_secs_f64().max(f64::EPSILON);
        println!(
            "  #{}: {} bytes in {:.2?} s ({mib_per_s:.1} MiB/s)",
            segment.index, segment.bytes, segment.duration
        );
    }
    Ok(())
}

/// Bytes to MiB (display-only; precision loss is irrelevant here).
#[allow(clippy::cast_precision_loss)]
fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
