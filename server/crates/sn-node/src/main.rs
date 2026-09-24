//! CLI entry point for sn-node.
//!
//! KEINE GARANTIE — this software is provided "as is" without warranty of
//! any kind (AGPL-3.0, sections 15/16). Users are solely responsible for
//! what they transmit or receive with it.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use sn_node::config::{self, ListenUri, MulticastMode, NodeSettings, PeerUri, PublicKeyHex};
use sn_node::daemon::{self, NO_WARRANTY_BANNER};
use sn_node::daemon_config;
use sn_node::yggdrasil::{self, AdminEndpoint};

#[derive(Debug, Parser)]
#[command(
    name = "sn-node",
    about = "SpiderNet node helper: Yggdrasil config generation + overlay status"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a Yggdrasil config file for this SpiderNet node.
    Genconf {
        /// Peering URIs to connect to, comma separated (tcp://, tls://,
        /// quic://, socks://).
        #[arg(long = "peer", value_delimiter = ',')]
        peers: Vec<String>,

        /// Listener URIs for incoming peerings, comma separated
        /// (e.g. `tcp://[::]:1337`).
        #[arg(long = "listen", value_delimiter = ',')]
        listen: Vec<String>,

        /// Admin socket endpoint (tcp://host:port or unix:///path).
        #[arg(long, default_value = "tcp://localhost:9001")]
        admin: String,

        /// Public keys of neighbors allowed to peer with us (64 hex
        /// characters, comma separated). Empty: any node may peer.
        #[arg(long = "allow-key", value_delimiter = ',')]
        allow_key: Vec<String>,

        /// Store the node's private key in an external file instead of
        /// embedding it in the config.
        #[arg(long)]
        private_key_path: Option<PathBuf>,

        /// TUN interface: `auto`, `none` (headless router-only), or a name.
        #[arg(long, default_value = "auto")]
        ifname: String,

        /// Disable multicast (same-LAN) auto-peering.
        #[arg(long)]
        no_multicast: bool,

        /// Path to the yggdrasil binary.
        #[arg(long, default_value = "yggdrasil")]
        yggdrasil_bin: String,

        /// Output path for the generated config.
        #[arg(short, long, default_value = "yggdrasil.conf")]
        out: PathBuf,
    },
    /// Query the local Yggdrasil node via the admin socket.
    Status {
        /// Admin socket endpoint (tcp://host:port or unix:///path).
        #[arg(long, default_value = "tcp://localhost:9001")]
        admin: String,

        /// Operation timeout in seconds.
        #[arg(long, default_value_t = 3)]
        timeout_secs: u64,
    },
    /// Run the node daemon: supervise the Yggdrasil sidecar and the
    /// optional in-process exit service. Stops on SIGINT/SIGTERM.
    Daemon {
        /// Path to the TOML config file.
        #[arg(long, default_value = "node.toml")]
        config: PathBuf,

        /// Validate the config and exit without starting anything.
        #[arg(long)]
        check: bool,
    },
}

fn parse_uris<T>(
    inputs: &[String],
    what: &str,
    parse: fn(&str) -> Result<T, sn_node::error::NodeError>,
) -> Result<Vec<T>, String> {
    inputs
        .iter()
        .map(|input| parse(input).map_err(|err| format!("{what}: {err}")))
        .collect()
}

#[tokio::main]
async fn main() {
    if let Err(report) = run().await {
        eprintln!("NO WARRANTY — SpiderNet sn-node (AGPL-3.0, no liability)");
        eprintln!("error: {report}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();
    match args.command {
        Command::Genconf {
            peers,
            listen,
            admin,
            allow_key,
            private_key_path,
            ifname,
            no_multicast,
            yggdrasil_bin,
            out,
        } => {
            run_genconf(
                peers,
                listen,
                admin,
                allow_key,
                private_key_path,
                ifname,
                no_multicast,
                yggdrasil_bin,
                out,
            )
            .await
        }
        Command::Status {
            admin,
            timeout_secs,
        } => run_status(&admin, timeout_secs).await,
        Command::Daemon { config, check } => run_daemon_command(&config, check).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_genconf(
    peers: Vec<String>,
    listen: Vec<String>,
    admin: String,
    allow_key: Vec<String>,
    private_key_path: Option<PathBuf>,
    ifname: String,
    no_multicast: bool,
    yggdrasil_bin: String,
    out: PathBuf,
) -> Result<(), String> {
    let peers = parse_uris(&peers, "peer", PeerUri::parse)?;
    let listen = parse_uris(&listen, "listen", ListenUri::parse)?;
    let admin_listen = AdminEndpoint::parse(&admin).map_err(|err| err.to_string())?;
    let allowed_public_keys = allow_key
        .iter()
        .map(|key| PublicKeyHex::parse(key).map_err(|err| err.to_string()))
        .collect::<Result<Vec<_>, String>>()?;
    let if_name = config::parse_ifname(&ifname).map_err(|err| err.to_string())?;

    let settings = NodeSettings {
        peers,
        listen,
        multicast: Some(if no_multicast {
            MulticastMode::Disabled
        } else {
            MulticastMode::PlatformDefault
        }),
        admin_listen: Some(admin_listen),
        if_name: Some(if_name),
        allowed_public_keys,
        private_key_path,
    };

    let json = config::generate_config(&yggdrasil_bin, &settings).map_err(|err| err.to_string())?;
    config::write_config(&out, &json).map_err(|err| err.to_string())?;

    println!("NO WARRANTY — SpiderNet sn-node (AGPL-3.0, no liability)");
    println!();
    println!("config written: {}", out.display());
    println!(
        "mode: start with `yggdrasil -useconffile {}`",
        out.display()
    );
    println!();
    println!(
        "NOTE: the config contains this node's PRIVATE key. Keep it at \
         permission 600, never commit it, never share it."
    );
    Ok(())
}

async fn run_status(admin: &str, timeout_secs: u64) -> Result<(), String> {
    let endpoint = AdminEndpoint::parse(admin).map_err(|err| err.to_string())?;
    let timeout = Duration::from_secs(timeout_secs);

    let info = yggdrasil::AdminClient::get_self(&endpoint, timeout)
        .await
        .map_err(|err| err.to_string())?;

    println!("NO WARRANTY — SpiderNet sn-node (AGPL-3.0, no liability)");
    println!();
    println!("address:   {}", info.address);
    if let Some(subnet) = info.subnet.as_ref() {
        println!("subnet:    {subnet}");
    }
    if let Some(version) = info.build_version.as_ref() {
        let name = info.build_name.as_deref().unwrap_or("yggdrasil");
        println!("version:   {name} {version}");
    }
    println!("key:       {}", info.key);

    let peers = yggdrasil::AdminClient::get_peers(&endpoint, timeout)
        .await
        .map_err(|err| err.to_string())?;
    println!("peers:     {}", peers.len());
    for peer in &peers {
        match peer.remote.as_ref() {
            Some(remote) => println!("  {} via {remote}", peer.address),
            None => println!("  {}", peer.address),
        }
    }
    Ok(())
}

async fn run_daemon_command(config_path: &std::path::Path, check: bool) -> Result<(), String> {
    println!("{NO_WARRANTY_BANNER}");

    let validated = daemon_config::load(config_path).map_err(|err| err.to_string())?;
    if check {
        println!("config ok");
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    daemon::run_daemon(validated, shutdown_signal())
        .await
        .map_err(|err| err.to_string())
}

/// Future that resolves when SIGINT or SIGTERM (unix) arrives.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            return;
        }
    }
    tokio::signal::ctrl_c().await.ok();
}
