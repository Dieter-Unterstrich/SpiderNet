//! Typed, validated configuration for the node daemon (`sn-node daemon`).
//!
//! The TOML file is parsed into raw optional sections and then validated
//! into types that carry their invariants ("parsed data is validated"):
//! URIs, keys, interface names and addresses reuse the existing parsers of
//! `sn-node` and `sn-exit`, so an invalid config fails with a clear error
//! at load time instead of a mid-run surprise.
//!
//! File schema (TOML):
//!
//! ```toml
//! [node]
//! admin_endpoint = "tcp://localhost:9001"  # optional; also used as Yggdrasil AdminListen
//! state_dir = "spidernet-state"            # optional; holds node.key + generated yggdrasil.conf
//!
//! [yggdrasil]
//! manage = true          # optional, default true; false = an already-running yggdrasil daemon is used
//! binary = "yggdrasil"   # optional, default "yggdrasil"; only relevant when manage = true
//! peers = []             # optional; list of PeerUri strings (tcp://, tls://, quic://, socks://)
//! listen = []            # optional; list of ListenUri strings (tcp://, tls://, quic://)
//! allow_keys = []        # optional; 64-hex neighbor public keys (AllowedPublicKeys ACL)
//! no_multicast = false   # optional bool
//! ifname = "auto"        # optional: "auto" | "none" | interface name
//!
//! [exit]
//! enabled = false        # optional, default false (false = leech mode)
//! bind = ""              # optional explicit bind address; default "[<yggdrasil-address>]:port"
//! port = 8080            # optional u16, default 8080 (used when bind is unset)
//! allow = []             # optional neighbor overlay addresses (sn-exit --allow semantics)
//! quota_mib = 1024       # optional u64, default 1024; 0 = no quota
//! window_hours = 24      # optional u64, default 24
//! ```

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sn_exit::config::{parse_bind, parse_source, QuotaPolicy, SourceAllowlist};

use crate::config::{parse_ifname, IfName, ListenUri, NodeSettings, PeerUri, PublicKeyHex};
use crate::error::NodeError;
use crate::yggdrasil::{AdminEndpoint, YggAddr};

/// Default state directory (relative to the daemon's working directory).
pub const DEFAULT_STATE_DIR: &str = "spidernet-state";
/// Default yggdrasil binary (looked up in `PATH`).
pub const DEFAULT_YGGDRASIL_BINARY: &str = "yggdrasil";
/// Name of the persisted node private key inside the state directory.
pub const NODE_KEY_FILE: &str = "node.key";
/// Name of the generated Yggdrasil config inside the state directory.
pub const YGGDRASIL_CONF_FILE: &str = "yggdrasil.conf";
/// Default admin endpoint when `[node] admin_endpoint` is unset.
const DEFAULT_ADMIN_ENDPOINT: &str = "tcp://localhost:9001";
/// Default exit port when `[exit] bind` is unset.
const DEFAULT_EXIT_PORT: u16 = 8080;
/// Default per-neighbor quota in MiB.
const DEFAULT_QUOTA_MIB: u64 = 1024;
/// Default quota window in hours.
const DEFAULT_WINDOW_HOURS: u64 = 24;
const MIB: u64 = 1024 * 1024;

/// Where the exit service binds. Resolved at runtime because the overlay
/// address is only known once the overlay is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitBind {
    /// An explicit address from the config.
    Explicit(SocketAddr),
    /// `[<overlay address>]:port` on the node's own overlay address.
    Overlay { port: u16 },
}

impl ExitBind {
    /// Resolve this bind against the node's overlay address.
    pub const fn resolve(self, overlay: YggAddr) -> SocketAddr {
        match self {
            Self::Explicit(addr) => addr,
            Self::Overlay { port } => SocketAddr::new(IpAddr::V6(overlay.inner()), port),
        }
    }
}

/// Validated settings for the exit service; present only when
/// `[exit] enabled = true`. Without it the daemon runs in leech mode
/// (overlay only, receive without offering an exit).
#[derive(Debug, Clone)]
pub struct ExitServiceSettings {
    bind: ExitBind,
    allowlist: SourceAllowlist,
    quota: Option<QuotaPolicy>,
}

impl ExitServiceSettings {
    /// Where the exit should bind.
    pub const fn bind(&self) -> ExitBind {
        self.bind
    }

    /// Which neighbor source addresses may use the exit.
    pub const fn allowlist(&self) -> &SourceAllowlist {
        &self.allowlist
    }

    /// Per-neighbor quota, or none when disabled (`quota_mib = 0`).
    pub const fn quota(&self) -> Option<QuotaPolicy> {
        self.quota
    }
}

/// A fully validated daemon config. Construction goes through
/// [`parse`]/[`load`]; every field carries its invariant afterwards.
#[derive(Debug, Clone)]
pub struct ValidatedDaemonConfig {
    admin_endpoint: AdminEndpoint,
    state_dir: PathBuf,
    manage: bool,
    yggdrasil_binary: String,
    peers: Vec<PeerUri>,
    listen: Vec<ListenUri>,
    allowed_keys: Vec<PublicKeyHex>,
    no_multicast: bool,
    if_name: IfName,
    exit: Option<ExitServiceSettings>,
}

impl ValidatedDaemonConfig {
    /// Admin socket endpoint; also used as Yggdrasil `AdminListen` when
    /// the daemon manages the sidecar.
    pub const fn admin_endpoint(&self) -> &AdminEndpoint {
        &self.admin_endpoint
    }

    /// State directory: holds `node.key` and the generated yggdrasil
    /// config.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Whether the daemon manages the yggdrasil sidecar process.
    pub const fn manage(&self) -> bool {
        self.manage
    }

    /// Path or name of the yggdrasil binary.
    pub fn yggdrasil_binary(&self) -> &str {
        &self.yggdrasil_binary
    }

    /// The Yggdrasil settings derived from this config, with the node key
    /// at `<state_dir>/node.key`.
    pub fn node_settings(&self) -> NodeSettings {
        NodeSettings {
            peers: self.peers.clone(),
            listen: self.listen.clone(),
            multicast: Some(if self.no_multicast {
                crate::config::MulticastMode::Disabled
            } else {
                crate::config::MulticastMode::PlatformDefault
            }),
            admin_listen: Some(self.admin_endpoint.clone()),
            if_name: Some(self.if_name.clone()),
            allowed_public_keys: self.allowed_keys.clone(),
            private_key_path: Some(self.state_dir.join(NODE_KEY_FILE)),
        }
    }

    /// Exit service settings, or `None` in leech mode.
    pub const fn exit(&self) -> Option<&ExitServiceSettings> {
        self.exit.as_ref()
    }
}

/// Parse a daemon config from TOML text.
pub fn parse(input: &str) -> Result<ValidatedDaemonConfig, NodeError> {
    let raw: RawDaemonConfig = toml::from_str(input)
        .map_err(|err| NodeError::DaemonConfig(format!("cannot parse config: {err}")))?;
    validate(raw)
}

/// Parse a daemon config from the file at `path`.
pub fn load(path: &Path) -> Result<ValidatedDaemonConfig, NodeError> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        NodeError::DaemonConfig(format!("cannot read config file {}: {err}", path.display()))
    })?;
    parse(&text)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDaemonConfig {
    #[serde(default)]
    node: RawNodeSection,
    #[serde(default)]
    yggdrasil: RawYggdrasilSection,
    #[serde(default)]
    exit: RawExitSection,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNodeSection {
    admin_endpoint: Option<String>,
    state_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawYggdrasilSection {
    manage: Option<bool>,
    binary: Option<String>,
    #[serde(default)]
    peers: Option<Vec<String>>,
    #[serde(default)]
    listen: Option<Vec<String>>,
    #[serde(default)]
    allow_keys: Option<Vec<String>>,
    no_multicast: Option<bool>,
    ifname: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExitSection {
    enabled: Option<bool>,
    bind: Option<String>,
    port: Option<u16>,
    #[serde(default)]
    allow: Option<Vec<String>>,
    quota_mib: Option<u64>,
    window_hours: Option<u64>,
}

fn validate(raw: RawDaemonConfig) -> Result<ValidatedDaemonConfig, NodeError> {
    let RawDaemonConfig {
        node,
        yggdrasil,
        exit,
    } = raw;

    let admin_endpoint = match node.admin_endpoint.as_deref() {
        None => AdminEndpoint::parse(DEFAULT_ADMIN_ENDPOINT)
            .map_err(|err| NodeError::DaemonConfig(err.to_string()))?,
        Some(value) => AdminEndpoint::parse(value)
            .map_err(|err| NodeError::DaemonConfig(format!("node.admin_endpoint: {err}")))?,
    };
    let state_dir = node
        .state_dir
        .map_or_else(|| PathBuf::from(DEFAULT_STATE_DIR), PathBuf::from);

    let manage = yggdrasil.manage.unwrap_or(true);
    let yggdrasil_binary = yggdrasil
        .binary
        .unwrap_or_else(|| DEFAULT_YGGDRASIL_BINARY.to_string());

    let peers = parse_each(
        yggdrasil.peers.as_deref(),
        "yggdrasil.peers",
        PeerUri::parse,
    )?;
    let listen = parse_each(
        yggdrasil.listen.as_deref(),
        "yggdrasil.listen",
        ListenUri::parse,
    )?;
    let allowed_keys = parse_each(
        yggdrasil.allow_keys.as_deref(),
        "yggdrasil.allow_keys",
        PublicKeyHex::parse,
    )?;
    let no_multicast = yggdrasil.no_multicast.unwrap_or(false);
    let if_name = yggdrasil.ifname.as_deref().unwrap_or("auto");
    let if_name = parse_ifname(if_name)
        .map_err(|err| NodeError::DaemonConfig(format!("yggdrasil.ifname: {err}")))?;

    let exit = validate_exit(&exit)?;

    if !manage && exit.is_none() {
        return Err(NodeError::DaemonConfig(
            "nothing to do: set `[yggdrasil] manage = true` or `[exit] enabled = true`".to_string(),
        ));
    }

    Ok(ValidatedDaemonConfig {
        admin_endpoint,
        state_dir,
        manage,
        yggdrasil_binary,
        peers,
        listen,
        allowed_keys,
        no_multicast,
        if_name,
        exit,
    })
}

fn validate_exit(raw: &RawExitSection) -> Result<Option<ExitServiceSettings>, NodeError> {
    let enabled = raw.enabled.unwrap_or(false);

    let explicit_bind = match raw.bind.as_deref() {
        None | Some("") => None,
        Some(value) => Some(
            parse_bind(value)
                .map_err(|err| NodeError::DaemonConfig(format!("exit.bind: {err}")))?,
        ),
    };
    let allow_sources: Vec<IpAddr> = parse_each(raw.allow.as_deref(), "exit.allow", parse_source)?;

    // Without a neighbor allowlist only loopback clients would be
    // accepted — that is acceptable only for development, where an
    // explicit loopback bind makes the intent unmistakable.
    if enabled && allow_sources.is_empty() {
        let dev_mode = explicit_bind.is_some_and(|addr| addr.ip().is_loopback());
        if !dev_mode {
            return Err(NodeError::DaemonConfig(
                "exit enabled but no neighbor allowlist: add `exit.allow` entries, \
                 or (dev only) set `exit.bind` to a loopback address"
                    .to_string(),
            ));
        }
    }

    let allowlist = if allow_sources.is_empty() {
        SourceAllowlist::loopback_only()
    } else {
        SourceAllowlist::new(allow_sources)
    };

    let quota = match raw.quota_mib.unwrap_or(DEFAULT_QUOTA_MIB) {
        0 => None,
        mib => Some(
            QuotaPolicy::try_new(
                mib.saturating_mul(MIB),
                Duration::from_secs(
                    raw.window_hours
                        .unwrap_or(DEFAULT_WINDOW_HOURS)
                        .saturating_mul(3600),
                ),
            )
            .map_err(|err| NodeError::DaemonConfig(format!("exit.quota: {err}")))?,
        ),
    };

    let bind = match explicit_bind {
        Some(addr) => ExitBind::Explicit(addr),
        None => ExitBind::Overlay {
            port: raw.port.unwrap_or(DEFAULT_EXIT_PORT),
        },
    };

    if enabled {
        Ok(Some(ExitServiceSettings {
            bind,
            allowlist,
            quota,
        }))
    } else {
        Ok(None)
    }
}

/// Parse every entry with `parse`, wrapping failures in a single
/// config error that names the offending section.
fn parse_each<T, E: std::fmt::Display>(
    inputs: Option<&[String]>,
    what: &str,
    parse: fn(&str) -> Result<T, E>,
) -> Result<Vec<T>, NodeError> {
    let inputs = inputs.unwrap_or(&[]);
    inputs
        .iter()
        .map(|input| parse(input).map_err(|err| NodeError::DaemonConfig(format!("{what}: {err}"))))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn error_for(toml_text: &str) -> String {
        parse(toml_text)
            .expect_err("config should be invalid")
            .to_string()
    }

    #[test]
    fn full_config_maps_into_validated_types() {
        let config = parse(
            r#"
            [node]
            admin_endpoint = "tcp://localhost:9001"
            state_dir = "/var/lib/spidernet"

            [yggdrasil]
            manage = true
            binary = "/usr/local/bin/yggdrasil"
            peers = ["tcp://[200:1234::1]:1337", "socks://p:1/h:2"]
            listen = ["tls://[::]:1337"]
            allow_keys = ["00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"]
            no_multicast = true
            ifname = "eth0"

            [exit]
            enabled = true
            bind = "127.0.0.1:18099"
            allow = []
            quota_mib = 2048
            window_hours = 12
            "#,
        )
        .unwrap();

        assert_eq!(
            config.admin_endpoint(),
            &AdminEndpoint::parse("tcp://localhost:9001").unwrap()
        );
        assert_eq!(config.state_dir(), Path::new("/var/lib/spidernet"));
        assert!(config.manage());
        assert_eq!(config.yggdrasil_binary(), "/usr/local/bin/yggdrasil");

        let settings = config.node_settings();
        assert_eq!(settings.peers.len(), 2);
        assert_eq!(settings.peers[0].as_str(), "tcp://[200:1234::1]:1337");
        assert_eq!(settings.peers[1].as_str(), "socks://p:1/h:2");
        assert_eq!(settings.listen.len(), 1);
        assert_eq!(settings.listen[0].as_str(), "tls://[::]:1337");
        assert_eq!(settings.allowed_public_keys.len(), 1);
        assert!(matches!(
            settings.if_name,
            Some(IfName::Named(ref name)) if name == "eth0"
        ));
        assert_eq!(
            settings.private_key_path.as_deref(),
            Some(Path::new("/var/lib/spidernet/node.key"))
        );

        let exit = config.exit().unwrap();
        assert_eq!(
            exit.bind(),
            ExitBind::Explicit("127.0.0.1:18099".parse().unwrap())
        );
        assert!(exit.allowlist().is_loopback_only());
        let quota = exit.quota().unwrap();
        assert_eq!(quota.max_bytes(), 2048 * 1024 * 1024);
        assert_eq!(quota.window(), Duration::from_hours(12));
    }

    #[test]
    fn minimal_config_defaults_to_leech_mode() {
        let config = parse("[exit]\nenabled = false\n").unwrap();
        assert!(config.manage());
        assert_eq!(config.yggdrasil_binary(), DEFAULT_YGGDRASIL_BINARY);
        assert_eq!(config.state_dir(), Path::new(DEFAULT_STATE_DIR));
        assert!(config.exit().is_none());
        assert!(config.node_settings().peers.is_empty());
    }

    #[test]
    fn load_missing_file_is_an_error() {
        let error = load(Path::new("/nonexistent/spidernet-node.toml")).unwrap_err();
        assert!(error.to_string().contains("cannot read config file"));
    }

    #[test]
    fn load_valid_file_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("node.toml");
        std::fs::write(&path, "[yggdrasil]\nmanage = true\n").unwrap();
        assert!(load(&path).is_ok());
    }

    #[test]
    fn broken_toml_gives_readable_error() {
        let error = error_for("[node]\nadmin_endpoint = \n");
        assert!(error.contains("cannot parse config"), "{error}");
    }

    #[test]
    fn exit_without_allowlist_requires_loopback_bind() {
        let no_bind = error_for("[exit]\nenabled = true\n");
        assert!(
            no_bind.contains("exit enabled but no neighbor allowlist"),
            "{no_bind}"
        );

        let non_loopback = error_for("[exit]\nenabled = true\nbind = \"192.168.1.5:8080\"\n");
        assert!(
            non_loopback.contains("exit enabled but no neighbor allowlist"),
            "{non_loopback}"
        );
    }

    #[test]
    fn exit_with_loopback_bind_is_valid_dev_mode() {
        let config = parse("[exit]\nenabled = true\nbind = \"127.0.0.1:18099\"\n").unwrap();
        let exit = config.exit().unwrap();
        assert!(exit.allowlist().is_loopback_only());
        assert_eq!(
            exit.bind(),
            ExitBind::Explicit("127.0.0.1:18099".parse().unwrap())
        );
    }

    #[test]
    fn manage_false_and_exit_disabled_is_rejected() {
        let error = error_for("[yggdrasil]\nmanage = false\n");
        assert!(error.contains("nothing to do"), "{error}");
    }

    #[test]
    fn quota_zero_disables_quota() {
        let config =
            parse("[exit]\nenabled = true\nbind = \"[::1]:8080\"\nquota_mib = 0\n").unwrap();
        assert_eq!(config.exit().unwrap().quota(), None);
    }

    #[test]
    fn overlay_bind_defaults_to_port_8080_and_can_be_overridden() {
        let config = parse("[exit]\nenabled = true\nallow = [\"200:5678::2\"]\n").unwrap();
        let bind = config.exit().unwrap().bind();
        assert_eq!(bind, ExitBind::Overlay { port: 8080 });

        let overlay: std::net::Ipv6Addr = "200:1111::1".parse().unwrap();
        assert_eq!(
            bind.resolve(YggAddr::new(overlay)),
            SocketAddr::new(IpAddr::V6(overlay), 8080)
        );

        let config =
            parse("[exit]\nenabled = true\nallow = [\"200:5678::2\"]\nport = 9099\n").unwrap();
        let bind = config.exit().unwrap().bind();
        assert_eq!(
            bind.resolve(YggAddr::new(overlay)),
            SocketAddr::new(IpAddr::V6(overlay), 9099)
        );
    }

    #[test]
    fn invalid_entries_surface_reused_type_errors() {
        for invalid in [
            "[yggdrasil]\npeers = [\"banana\"]\n",
            "[yggdrasil]\nlisten = [\"socks://[::]:1337\"]\n",
            "[yggdrasil]\nallow_keys = [\"nothex\"]\n",
            "[yggdrasil]\nifname = \"\"\n",
            "[node]\nadmin_endpoint = \"banana\"\n",
            "[exit]\nallow = [\"banana\"]\n",
            "[exit]\nbind = \"banana\"\nenabled = true\n",
            "[exit]\nport = 70000\n",
        ] {
            let error = parse(invalid).expect_err("should be invalid").to_string();
            assert!(error.contains("invalid daemon config"), "{error}");
        }
    }
}
