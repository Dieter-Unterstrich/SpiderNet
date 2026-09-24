//! Typed, validated configuration for the exit service.

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::error::ExitError;

/// Quota: how many bytes a neighbor may push through the exit per window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaPolicy {
    max_bytes: u64,
    window: Duration,
}

impl QuotaPolicy {
    pub fn try_new(max_bytes: u64, window: Duration) -> Result<Self, ExitError> {
        if max_bytes == 0 {
            return Err(ExitError::InvalidQuota("max_bytes must be non-zero"));
        }
        if window.is_zero() {
            return Err(ExitError::InvalidQuota("window must be non-zero"));
        }
        Ok(Self { max_bytes, window })
    }

    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }

    pub const fn window(self) -> Duration {
        self.window
    }
}

/// Which source addresses (neighbors) may use this exit. Empty list =
/// loopback only (development mode); real deployments always list the
/// neighbors' overlay addresses explicitly.
#[derive(Debug, Clone)]
pub struct SourceAllowlist {
    allowed: HashSet<IpAddr>,
    allow_loopback: bool,
}

impl SourceAllowlist {
    /// Loopback-only allowlist (development mode).
    pub fn loopback_only() -> Self {
        Self {
            allowed: HashSet::new(),
            allow_loopback: true,
        }
    }

    /// Explicit allowlist; loopback is allowed additionally in dev mode
    /// (when no explicit entries exist).
    pub fn new(allowed: Vec<IpAddr>) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
            allow_loopback: false,
        }
    }

    pub fn is_allowed(&self, source: IpAddr) -> bool {
        source.is_loopback() && self.allow_loopback || self.allowed.contains(&source)
    }

    pub fn len(&self) -> usize {
        self.allowed.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }

    pub fn is_loopback_only(&self) -> bool {
        self.allow_loopback
    }
}

/// Which origins a neighbor may reach through this exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginPolicy {
    /// Production: only public internet addresses (see
    /// [`is_public_origin`]). Protects the exit household's LAN, its
    /// admin sockets and overlay addresses from neighbor requests.
    PublicOnly,
    /// Test-only: any origin, including loopback. Never use in
    /// production — it removes the SSRF protection entirely.
    AnyForTesting,
}

/// Everything the exit needs to run.
#[derive(Debug, Clone)]
pub struct ExitConfig {
    bind: SocketAddr,
    allowlist: SourceAllowlist,
    quota: Option<QuotaPolicy>,
    origin_policy: OriginPolicy,
}

impl ExitConfig {
    pub fn new(bind: SocketAddr, allowlist: SourceAllowlist, quota: Option<QuotaPolicy>) -> Self {
        Self {
            bind,
            allowlist,
            quota,
            origin_policy: OriginPolicy::PublicOnly,
        }
    }

    /// Override the origin policy (`AnyForTesting` only in tests).
    #[must_use]
    pub const fn with_origin_policy(mut self, policy: OriginPolicy) -> Self {
        self.origin_policy = policy;
        self
    }

    pub const fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn allowlist(&self) -> &SourceAllowlist {
        &self.allowlist
    }

    pub const fn quota(&self) -> Option<QuotaPolicy> {
        self.quota
    }

    pub const fn origin_policy(&self) -> OriginPolicy {
        self.origin_policy
    }
}

/// SSRF protection: an origin is only fetchable if it is *public*
/// internet. Loopback, private/LAN, link-local, CGNAT, ULA, multicast,
/// unspecified and Yggdrasil overlay ranges are refused — neighbors must
/// not be able to reach the exit household's own LAN or admin sockets,
/// and the exit is for internet egress, not overlay-internal traffic.
pub fn is_public_origin(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                // 100.64/10 CGNAT (`is_shared()` is still unstable)
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
                || v4.octets()[0] == 0) // 0.0.0.0/8 "this network"
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fe80::/10 link-local
                || (segments[0] & 0xffc0) == 0xfe80
                // fc00::/7 unique-local (also would cover Yggdrasil's
                // 200::/7? No: 200::/7 is 0x0200-0x03ff, fc00::/7 is
                // 0xfc00-0xfdff — distinct. Checked separately below.)
                || (segments[0] & 0xfe00) == 0xfc00
                // 200::/7 Yggdrasil overlay (0x0200-0x03ff)
                || (segments[0] & 0xfe00) == 0x0200
                // ::ffff:x.y.z.w IPv4-mapped — judge by the v4 part
                || (segments[0] == 0 && segments[1] == 0 && segments[2] == 0 && segments[3] == 0 && segments[4] == 0 && segments[5] == 0xffff && {
                    let mapped = IpAddr::V4(std::net::Ipv4Addr::new(
                        (segments[6] >> 8) as u8,
                        (segments[6] & 0xff) as u8,
                        (segments[7] >> 8) as u8,
                        (segments[7] & 0xff) as u8,
                    ));
                    !is_public_origin(mapped)
                }))
        }
    }
}

/// Parse an address from CLI (`--bind`, `--allow`). Returns a typed error
/// instead of panicking on junk input.
pub fn parse_bind(input: &str) -> Result<SocketAddr, ExitError> {
    input
        .parse()
        .map_err(|_| ExitError::InvalidBind(input.to_string()))
}

pub fn parse_source(input: &str) -> Result<IpAddr, ExitError> {
    input
        .parse()
        .map_err(|_| ExitError::InvalidSource(input.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn quota_rejects_zero() {
        assert!(QuotaPolicy::try_new(0, Duration::from_secs(60)).is_err());
        assert!(QuotaPolicy::try_new(100, Duration::ZERO).is_err());
        assert!(QuotaPolicy::try_new(100, Duration::from_secs(60)).is_ok());
    }

    #[test]
    fn allowlist_loopback_default_and_explicit_entries() {
        let dev = SourceAllowlist::loopback_only();
        assert!(dev.is_allowed(std::net::IpAddr::from([127, 0, 0, 1])));
        assert!(!dev.is_allowed(std::net::IpAddr::from([192, 168, 1, 2])));

        let explicit = SourceAllowlist::new(vec![std::net::IpAddr::from([
            0x200u16, 1, 0, 0, 0, 0, 0, 1,
        ])]);
        assert!(!explicit.is_allowed(std::net::IpAddr::from([127, 0, 0, 1])));
        assert!(explicit.is_allowed(std::net::IpAddr::from([0x200, 1, 0, 0, 0, 0, 0, 1])));
    }

    #[test]
    fn public_origin_rejects_private_and_local_targets() {
        // Public internet: allowed.
        assert!(is_public_origin(std::net::IpAddr::from([93, 184, 216, 34])));
        assert!(is_public_origin(
            "2606:2800:220:1:248:1893:25c8:1946"
                .parse::<std::net::Ipv6Addr>()
                .unwrap()
                .into()
        ));

        // Refused: loopback, RFC1918, link-local, CGNAT, docs, multicast.
        for bad_v4 in [
            [127, 0, 0, 1],
            [10, 0, 0, 5],
            [172, 16, 3, 4],
            [192, 168, 1, 1],
            [169, 254, 9, 9],
            [100, 64, 0, 1],
            [192, 0, 2, 55],
            [224, 0, 0, 1],
            [0, 0, 0, 0],
        ] {
            assert!(
                !is_public_origin(std::net::IpAddr::from(bad_v4)),
                "v4 {bad_v4:?} must be refused"
            );
        }

        // Refused: v6 loopback, link-local, ULA, yggdrasil 200::/7,
        // IPv4-mapped loopback.
        for bad_v6 in [
            "::1",
            "fe80::1",
            "fc00::1",
            "200:1234::1",
            "::ffff:127.0.0.1",
        ] {
            let v6 = bad_v6.parse::<std::net::Ipv6Addr>().unwrap();
            assert!(
                !is_public_origin(std::net::IpAddr::V6(v6)),
                "v6 {bad_v6} must be refused"
            );
        }
    }

    #[test]
    fn parse_bind_and_source() {
        assert_eq!(
            parse_bind("[::1]:8080").unwrap(),
            SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 8080)
        );
        assert_eq!(
            parse_bind("127.0.0.1:8080").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], 8080))
        );
        assert!(parse_bind("nope").is_err());

        assert_eq!(
            parse_source("200:1:2:3:4:5:6:7").unwrap(),
            std::net::IpAddr::V6(std::net::Ipv6Addr::new(0x200, 1, 2, 3, 4, 5, 6, 7))
        );
        assert!(parse_source("banana").is_err());
    }
}
