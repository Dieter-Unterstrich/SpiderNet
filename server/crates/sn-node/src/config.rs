//! Yggdrasil configuration generation for a `SpiderNet` node.
//!
//! Yggdrasil's own `-genconf` output is used as the base (so we never
//! hard-code version-specific defaults); the `SpiderNet` settings are then
//! patched into that JSON base as validated, typed values. The result is
//! written as JSON — Yggdrasil accepts JSON config via `-useconffile`.

use std::path::{Path, PathBuf};

use crate::error::NodeError;
use crate::yggdrasil::AdminEndpoint;

/// A validated Yggdrasil peering URI (`tcp://`, `tls://`, `quic://` or
/// `socks://`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerUri(String);

impl PeerUri {
    pub fn parse(input: &str) -> Result<Self, NodeError> {
        Ok(Self(validate_scheme(input, "peering")?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated listener URI a node binds for incoming peerings
/// (`tcp://[::]:1337` style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenUri(String);

impl ListenUri {
    pub fn parse(input: &str) -> Result<Self, NodeError> {
        let trimmed = input.trim();
        if trimmed.starts_with("socks://") {
            return Err(NodeError::InvalidUri {
                uri: input.to_string(),
                hint: "listeners bind an address: use tcp://, tls:// or quic://",
            });
        }
        Ok(Self(validate_scheme(trimmed, "listen")?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

const PEER_SCHEMES: [&str; 4] = ["tcp://", "tls://", "quic://", "socks://"];

fn validate_scheme(input: &str, kind: &'static str) -> Result<String, NodeError> {
    let trimmed = input.trim();
    let scheme = PEER_SCHEMES
        .iter()
        .find(|scheme| trimmed.starts_with(**scheme));
    match scheme {
        Some(scheme) if trimmed.len() > scheme.len() => Ok(trimmed.to_string()),
        _ => Err(NodeError::InvalidUri {
            uri: input.to_string(),
            hint: match kind {
                "peering" => "peer URIs must start with tcp://, tls://, quic:// or socks://",
                _ => "listen URIs must start with tcp://, tls:// or quic://",
            },
        }),
    }
}

/// A validated Yggdrasil public key: 64 hex characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKeyHex(String);

impl PublicKeyHex {
    pub fn parse(input: &str) -> Result<Self, NodeError> {
        let trimmed = input.trim().to_lowercase();
        let valid = trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit());
        if valid {
            Ok(Self(trimmed))
        } else {
            Err(NodeError::InvalidPublicKey(input.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which TUN interface Yggdrasil should use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IfName {
    /// `auto` — let Yggdrasil pick (default).
    Auto,
    /// `none` — headless router-only mode, no TUN device.
    Headless,
    Named(String),
}

/// Multicast (same-LAN) auto-peering mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MulticastMode {
    /// Leave Yggdrasil's generated default untouched.
    PlatformDefault,
    /// Disable multicast peering entirely.
    Disabled,
    /// Explicit interface entries (regex-matched).
    Interfaces(Vec<MulticastInterface>),
}

/// One multicast peering entry, mirroring Yggdrasil's JSON shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticastInterface {
    /// Interface name regex, e.g. `eth0` or `.*`.
    pub regex: String,
    pub beacon: bool,
    pub listen: bool,
    pub port: u16,
}

/// Everything `SpiderNet` wants to control in the Yggdrasil config.
/// Unlisted fields keep the values Yggdrasil's `-genconf` produced.
#[derive(Debug, Clone, Default)]
pub struct NodeSettings {
    pub peers: Vec<PeerUri>,
    pub listen: Vec<ListenUri>,
    pub multicast: Option<MulticastMode>,
    pub admin_listen: Option<AdminEndpoint>,
    pub if_name: Option<IfName>,
    /// Neighbor keys allowed to peer with us; empty means "allow all".
    pub allowed_public_keys: Vec<PublicKeyHex>,
    pub private_key_path: Option<PathBuf>,
}

/// Run `yggdrasil -genconf -json` and patch the `SpiderNet` settings into
/// the generated base config. Returns the pretty-printed JSON config.
pub fn generate_config(binary: &str, settings: &NodeSettings) -> Result<String, NodeError> {
    let output = std::process::Command::new(binary)
        .arg("-genconf")
        .arg("-json")
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NodeError::YggdrasilBinary {
            binary: binary.to_string(),
            stderr,
        });
    }
    let mut base: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    apply_to_base_config(settings, &mut base)?;
    Ok(serde_json::to_string_pretty(&base)?)
}

/// Patch the `SpiderNet` settings into an existing (parsed) Yggdrasil base
/// config. Pure function so it can be tested without the binary.
pub fn apply_to_base_config(
    settings: &NodeSettings,
    base: &mut serde_json::Value,
) -> Result<(), NodeError> {
    let obj = base
        .as_object_mut()
        .ok_or(NodeError::UnexpectedConfigShape)?;

    if !settings.peers.is_empty() {
        let peers: Vec<&str> = settings.peers.iter().map(PeerUri::as_str).collect();
        obj.insert("Peers".to_string(), serde_json::json!(peers));
    }
    if !settings.listen.is_empty() {
        let listen: Vec<&str> = settings.listen.iter().map(ListenUri::as_str).collect();
        obj.insert("Listen".to_string(), serde_json::json!(listen));
    }
    match settings.multicast.as_ref() {
        None | Some(MulticastMode::PlatformDefault) => {}
        Some(MulticastMode::Disabled) => {
            obj.insert("MulticastInterfaces".to_string(), serde_json::json!([]));
        }
        Some(MulticastMode::Interfaces(interfaces)) => {
            let entries: Vec<serde_json::Value> = interfaces
                .iter()
                .map(|iface| {
                    serde_json::json!({
                        "Beacon": iface.beacon,
                        "Listen": iface.listen,
                        "Port": iface.port,
                        "Regex": iface.regex,
                    })
                })
                .collect();
            obj.insert(
                "MulticastInterfaces".to_string(),
                serde_json::json!(entries),
            );
        }
    }
    if let Some(admin_listen) = settings.admin_listen.as_ref() {
        obj.insert(
            "AdminListen".to_string(),
            serde_json::json!(admin_listen.config_value()),
        );
    }
    if let Some(if_name) = settings.if_name.as_ref() {
        let value = match if_name {
            IfName::Auto => "auto".to_string(),
            IfName::Headless => "none".to_string(),
            IfName::Named(name) => name.clone(),
        };
        obj.insert("IfName".to_string(), serde_json::json!(value));
    }
    if !settings.allowed_public_keys.is_empty() {
        let keys: Vec<&str> = settings
            .allowed_public_keys
            .iter()
            .map(PublicKeyHex::as_str)
            .collect();
        obj.insert("AllowedPublicKeys".to_string(), serde_json::json!(keys));
    }
    if let Some(private_key_path) = settings.private_key_path.as_ref() {
        obj.insert(
            "PrivateKeyPath".to_string(),
            serde_json::json!(private_key_path.display().to_string()),
        );
    }
    Ok(())
}

/// Write the config to `path` and restrict permissions to the owner
/// (the file contains the node's private key — it must never be readable
/// by other users, and it must never be committed to any repository).
pub fn write_config(path: &Path, json: &str) -> Result<(), NodeError> {
    use std::io::Write as _;

    let mut file = std::fs::File::create(path)?;
    file.write_all(json.as_bytes())?;
    file.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn peer_uri_accepts_supported_schemes() {
        for uri in [
            "tcp://host:1337",
            "tls://host:1337",
            "quic://host:1337",
            "socks://p:1/h:2",
        ] {
            assert!(PeerUri::parse(uri).is_ok(), "{uri} should be valid");
        }
    }

    #[test]
    fn peer_uri_rejects_bare_or_unknown_schemes() {
        for uri in ["host:1337", "http://host:80", "tcp://", ""] {
            assert!(PeerUri::parse(uri).is_err(), "{uri} should be invalid");
        }
    }

    #[test]
    fn listen_uri_rejects_socks() {
        assert!(ListenUri::parse("socks://[::]:1337").is_err());
        assert!(ListenUri::parse("tcp://[::]:1337").is_ok());
    }

    #[test]
    fn public_key_requires_64_hex_chars() {
        assert!(PublicKeyHex::parse(&"a".repeat(64)).is_ok());
        assert!(PublicKeyHex::parse(&"a".repeat(63)).is_err());
        assert!(PublicKeyHex::parse(&"z".repeat(64)).is_err());
    }

    #[test]
    fn admin_endpoint_parse_and_roundtrip() {
        let tcp = AdminEndpoint::parse("tcp://localhost:9001").unwrap();
        assert_eq!(
            tcp,
            AdminEndpoint::Tcp {
                host: "localhost".into(),
                port: 9001
            }
        );
        assert_eq!(tcp.config_value(), "tcp://localhost:9001");

        let unix = AdminEndpoint::parse("unix:///var/run/yggdrasil.sock").unwrap();
        assert_eq!(unix.config_value(), "unix:///var/run/yggdrasil.sock");

        assert!(AdminEndpoint::parse("http://localhost:9001").is_err());
        assert!(AdminEndpoint::parse("tcp://localhost").is_err());
    }

    #[test]
    fn patches_land_in_base_config() {
        let mut base: serde_json::Value =
            serde_json::from_str(r#"{"PrivateKey": "secret", "Peers": [], "Listen": []}"#).unwrap();
        let settings = NodeSettings {
            peers: vec![PeerUri::parse("tcp://[200:1234::1]:1337").unwrap()],
            listen: vec![ListenUri::parse("tls://[::]:1337").unwrap()],
            multicast: Some(MulticastMode::Disabled),
            admin_listen: Some(AdminEndpoint::parse("tcp://localhost:9001").unwrap()),
            if_name: Some(IfName::Headless),
            allowed_public_keys: vec![PublicKeyHex::parse(&"ab".repeat(32)).unwrap()],
            private_key_path: Some(PathBuf::from("/etc/spidernet/ygg.key")),
        };
        apply_to_base_config(&settings, &mut base).unwrap();

        assert_eq!(base["Peers"].as_array().unwrap().len(), 1);
        assert_eq!(base["Listen"][0], "tls://[::]:1337");
        assert_eq!(base["MulticastInterfaces"].as_array().unwrap().len(), 0);
        assert_eq!(base["AdminListen"], "tcp://localhost:9001");
        assert_eq!(base["IfName"], "none");
        assert_eq!(base["AllowedPublicKeys"][0], "ab".repeat(32));
        assert_eq!(base["PrivateKeyPath"], "/etc/spidernet/ygg.key");
        // The base's own keys survive the patch.
        assert_eq!(base["PrivateKey"], "secret");
    }

    #[test]
    fn multicast_interfaces_are_serialized_in_yggdrasil_shape() {
        let mut base: serde_json::Value = serde_json::from_str("{}").unwrap();
        let settings = NodeSettings {
            multicast: Some(MulticastMode::Interfaces(vec![MulticastInterface {
                regex: "eth0".to_string(),
                beacon: true,
                listen: true,
                port: 1337,
            }])),
            ..NodeSettings::default()
        };
        apply_to_base_config(&settings, &mut base).unwrap();
        let entry = &base["MulticastInterfaces"][0];
        assert_eq!(entry["Regex"], "eth0");
        assert_eq!(entry["Beacon"], true);
        assert_eq!(entry["Listen"], true);
        assert_eq!(entry["Port"], 1337);
    }
}
