//! Client for the Yggdrasil admin socket.
//!
//! Protocol (per the official docs): JSON stanza + `\n` over a TCP or unix
//! socket; the default endpoint is `localhost:9001`. A response always has
//! a `"status"` field (`"success"` or `"error"`), optionally a `"response"`
//! section and an `"error"` field. By default Yggdrasil closes the
//! connection after answering, so this client sends one request per
//! connection.

use std::net::Ipv6Addr;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::NodeError;

/// Where the admin socket lives, in Yggdrasil endpoint syntax
/// (`tcp://host:port` or `unix:///path/to/socket`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminEndpoint {
    Tcp { host: String, port: u16 },
    Unix { path: PathBuf },
}

impl AdminEndpoint {
    /// Yggdrasil's default admin endpoint: `tcp://localhost:9001`.
    pub fn default_local() -> Self {
        Self::Tcp {
            host: "localhost".to_string(),
            port: 9001,
        }
    }

    pub fn parse(input: &str) -> Result<Self, NodeError> {
        let trimmed = input.trim();
        if let Some(rest) = trimmed.strip_prefix("tcp://") {
            let (host, port) = rest
                .rsplit_once(':')
                .ok_or_else(|| NodeError::InvalidAdminEndpoint(trimmed.to_string()))?;
            let port: u16 = port
                .parse()
                .map_err(|_| NodeError::InvalidAdminEndpoint(trimmed.to_string()))?;
            if host.is_empty() {
                return Err(NodeError::InvalidAdminEndpoint(trimmed.to_string()));
            }
            Ok(Self::Tcp {
                host: host.to_string(),
                port,
            })
        } else if let Some(rest) = trimmed.strip_prefix("unix://") {
            if rest.is_empty() {
                return Err(NodeError::InvalidAdminEndpoint(trimmed.to_string()));
            }
            Ok(Self::Unix {
                path: PathBuf::from(rest),
            })
        } else {
            Err(NodeError::InvalidAdminEndpoint(trimmed.to_string()))
        }
    }

    /// Value for the `AdminListen` field of a Yggdrasil config file.
    pub fn config_value(&self) -> String {
        match self {
            Self::Tcp { host, port } => format!("tcp://{host}:{port}"),
            Self::Unix { path } => format!("unix://{}", path.display()),
        }
    }
}

/// The node's own Yggdrasil address — a validated IPv6 address inside the
/// Yggdrasil range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YggAddr(Ipv6Addr);

impl YggAddr {
    pub const fn new(addr: Ipv6Addr) -> Self {
        Self(addr)
    }

    pub const fn inner(self) -> Ipv6Addr {
        self.0
    }
}

impl std::fmt::Display for YggAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One connection to the admin socket; one request per connection.
pub struct AdminClient {
    stream: AdminStream,
}

enum AdminStream {
    Tcp(tokio::net::TcpStream),
    Unix(tokio::net::UnixStream),
}

async fn connect_stream(endpoint: &AdminEndpoint) -> Result<AdminStream, NodeError> {
    match endpoint {
        AdminEndpoint::Tcp { host, port } => tokio::net::TcpStream::connect((host.as_str(), *port))
            .await
            .map(AdminStream::Tcp)
            .map_err(NodeError::Io),
        AdminEndpoint::Unix { path } => tokio::net::UnixStream::connect(path)
            .await
            .map(AdminStream::Unix)
            .map_err(NodeError::Io),
    }
}

impl AdminClient {
    pub async fn connect(endpoint: &AdminEndpoint, timeout: Duration) -> Result<Self, NodeError> {
        let stream = tokio::time::timeout(timeout, connect_stream(endpoint))
            .await
            .map_err(|_| NodeError::Timeout)??;
        Ok(Self { stream })
    }

    /// Send one request verb (e.g. `getself`, `getpeers`) and return the
    /// raw response section.
    pub async fn request(&mut self, verb: &str) -> Result<serde_json::Value, NodeError> {
        // `verb` is a compile-time-known constant at every call site, not
        // user input — no injection risk.
        let request = format!("{{\"request\":\"{verb}\"}}\n");
        let bytes = request.as_bytes();

        match &mut self.stream {
            AdminStream::Tcp(stream) => {
                tokio::io::AsyncWriteExt::write_all(stream, bytes).await?;
                let mut buffer = Vec::new();
                tokio::io::AsyncReadExt::read_to_end(stream, &mut buffer).await?;
                Self::parse_response(&buffer, verb)
            }
            AdminStream::Unix(stream) => {
                tokio::io::AsyncWriteExt::write_all(stream, bytes).await?;
                let mut buffer = Vec::new();
                tokio::io::AsyncReadExt::read_to_end(stream, &mut buffer).await?;
                Self::parse_response(&buffer, verb)
            }
        }
    }

    /// Query the node's own status (address, subnet, key, build info).
    pub async fn get_self(
        endpoint: &AdminEndpoint,
        timeout: Duration,
    ) -> Result<SelfInfo, NodeError> {
        let mut client = Self::connect(endpoint, timeout).await?;
        let value = client.request("getself").await?;
        SelfInfo::from_response(&value)
    }

    /// Query the node's active peer sessions.
    pub async fn get_peers(
        endpoint: &AdminEndpoint,
        timeout: Duration,
    ) -> Result<Vec<PeerInfo>, NodeError> {
        let mut client = Self::connect(endpoint, timeout).await?;
        let value = client.request("getpeers").await?;
        peers_from_response(&value)
    }

    fn parse_response(buffer: &[u8], verb: &str) -> Result<serde_json::Value, NodeError> {
        let envelope: serde_json::Value = serde_json::from_slice(buffer)?;
        let status = envelope
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                NodeError::AdminProtocol(format!("{verb}: response has no `status` field"))
            })?;
        if status != "success" {
            let error = envelope
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown error");
            return Err(NodeError::AdminFailed(error.to_string()));
        }
        envelope
            .get("response")
            .cloned()
            .ok_or_else(|| NodeError::AdminProtocol(format!("{verb}: response has no data")))
    }
}

/// Information about the local Yggdrasil node (`getSelf`).
#[derive(Debug, Clone)]
pub struct SelfInfo {
    pub address: YggAddr,
    pub key: String,
    pub subnet: Option<String>,
    pub coords: Option<serde_json::Value>,
    pub build_name: Option<String>,
    pub build_version: Option<String>,
}

impl SelfInfo {
    /// Parse the `getSelf` response section: exactly one record, keyed by
    /// the node's IPv6 address.
    pub fn from_response(value: &serde_json::Value) -> Result<Self, NodeError> {
        let obj = value.as_object().ok_or_else(|| {
            NodeError::AdminProtocol("getSelf: response section is not an object".to_string())
        })?;
        let (address, record) = obj.iter().next().ok_or_else(|| {
            NodeError::AdminProtocol("getSelf: response contains no record".to_string())
        })?;
        let address = YggAddr::new(address.parse::<Ipv6Addr>().map_err(|_| {
            NodeError::AdminProtocol(format!("getSelf: `{address}` is not an IPv6 address"))
        })?);

        let str_field = |name: &str| {
            record
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };

        Ok(Self {
            address,
            key: str_field("key").ok_or_else(|| {
                NodeError::AdminProtocol("getSelf: record has no `key`".to_string())
            })?,
            subnet: str_field("subnet"),
            coords: record.get("coords").cloned(),
            build_name: str_field("build_name"),
            build_version: str_field("build_version"),
        })
    }
}

/// One active peer session (`getPeers`).
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub address: YggAddr,
    pub key: Option<String>,
    /// The peering URI of the remote peer, when reported.
    pub remote: Option<String>,
    pub coords: Option<serde_json::Value>,
}

/// Parse the `getPeers` response section: records keyed by IPv6 address.
/// The first record typically refers to the current node itself.
pub fn peers_from_response(value: &serde_json::Value) -> Result<Vec<PeerInfo>, NodeError> {
    let obj = value.as_object().ok_or_else(|| {
        NodeError::AdminProtocol("getPeers: response section is not an object".to_string())
    })?;
    let mut peers = Vec::with_capacity(obj.len());
    for (address, record) in obj {
        let address = YggAddr::new(address.parse::<Ipv6Addr>().map_err(|_| {
            NodeError::AdminProtocol(format!("getPeers: `{address}` is not an IPv6 address"))
        })?);
        peers.push(PeerInfo {
            address,
            key: record
                .get("key")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            remote: record
                .get("remote")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            coords: record.get("coords").cloned(),
        });
    }
    Ok(peers)
}
