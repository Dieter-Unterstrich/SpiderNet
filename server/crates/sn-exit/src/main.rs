//! CLI entry point for sn-exit.
//!
//! KEINE GARANTIE — this software is provided "as is" without warranty of
//! any kind (AGPL-3.0, sections 15/16). Users are solely responsible for
//! what they transmit or receive with it.

use std::time::Duration;

use clap::Parser;
use sn_exit::config::{self, ExitConfig, QuotaPolicy, SourceAllowlist};

/// Per-neighbor quota is disabled when 0 is passed (no limit).
const QUOTA_DISABLED: u64 = 0;

#[derive(Debug, Parser)]
#[command(
    name = "sn-exit",
    about = "SpiderNet exit service: quota-controlled uplink proxy for neighbors"
)]
struct Args {
    /// Address to listen on — ideally the household's Yggdrasil address,
    /// e.g. `[200:1234::1]:8080`.
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,

    /// Neighbor addresses allowed to use this exit (repeatable, e.g.
    /// `200:5678::2`). Without any `--allow`, only loopback clients are
    /// accepted (development mode).
    #[arg(long = "allow", value_delimiter = ',')]
    allow: Vec<String>,

    /// Quota per neighbor per window, in MiB. `0` disables the quota.
    #[arg(long, default_value_t = 1024)]
    quota_mib: u64,

    /// Quota window in hours (fixed window per neighbor).
    #[arg(long, default_value_t = 24)]
    window_hours: u64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(report) = run().await {
        eprintln!("NO WARRANTY — SpiderNet sn-exit (AGPL-3.0, no liability)");
        eprintln!("error: {report}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();

    let bind = config::parse_bind(&args.bind).map_err(|err| err.to_string())?;
    let allowlist = if args.allow.is_empty() {
        SourceAllowlist::loopback_only()
    } else {
        let sources = args
            .allow
            .iter()
            .map(|addr| config::parse_source(addr).map_err(|err| err.to_string()))
            .collect::<Result<Vec<_>, String>>()?;
        SourceAllowlist::new(sources)
    };
    let quota = if args.quota_mib == QUOTA_DISABLED {
        None
    } else {
        let max_bytes = args.quota_mib.saturating_mul(1024 * 1024);
        Some(
            QuotaPolicy::try_new(max_bytes, Duration::from_secs(args.window_hours * 3600))
                .map_err(|err| err.to_string())?,
        )
    };

    let config = ExitConfig::new(bind, allowlist, quota);
    let listener = tokio::net::TcpListener::bind(config.bind())
        .await
        .map_err(|err| format!("cannot bind {}: {err}", config.bind()))?;

    println!("NO WARRANTY — SpiderNet sn-exit (AGPL-3.0, no liability)");
    println!();
    if config.allowlist().is_loopback_only() {
        println!("DEV MODE: no --allow given — only loopback clients are accepted.");
    }
    match config.quota() {
        Some(policy) => println!(
            "quota:    {} MiB per {}h per neighbor (enforced at connection start)",
            policy.max_bytes() / (1024 * 1024),
            policy.window().as_secs() / 3600,
        ),
        None => println!("quota:    disabled — neighbors may use unlimited uplink traffic"),
    }
    println!(
        "sharing:  this household now offers its uplink (stop sn-exit to revoke = kill-switch)"
    );
    println!("listen:   {}", config.bind());

    sn_exit::proxy::serve(config, listener)
        .await
        .map_err(|err| err.to_string())
}
