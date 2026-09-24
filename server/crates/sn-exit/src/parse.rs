//! Minimal, strict HTTP/1.1 proxy-head parsing.
//!
//! The exit is a byte relay, so it only needs to understand the request
//! line (method + target) to find the origin — everything else is
//! forwarded verbatim inside the pipe. Strictness beats flexibility
//! here: anything that is not a clean proxy request line is refused.

use crate::error::ExitError;

/// How the client addressed the origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestTarget {
    /// CONNECT authority form: `host:port` (TLS and any other TCP).
    Tunnel,
    /// Absolute URI: `http://host[:port]/…` (plain HTTP via proxy).
    Forward,
}

/// One parsed proxy request head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHead {
    pub target: RequestTarget,
    pub host: String,
    pub port: u16,
}

/// Largest accepted request head; anything above is refused instead of
/// buffered without bound.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;

/// Parse the request line out of a raw head buffer. The buffer may
/// include the trailing `\r\n\r\n`; only the first line is examined.
///
/// `CONNECT host:port HTTP/1.1` → tunnel to `host:port`.
/// `GET http://host[:port]/path HTTP/1.1` → forward to `host:port`.
pub fn parse_request_head(head: &[u8]) -> Result<RequestHead, ExitError> {
    let head_text = std::str::from_utf8(head)
        .map_err(|_| ExitError::Protocol("request head is not valid utf-8"))?;
    let request_line = head_text
        .lines()
        .next()
        .ok_or(ExitError::Protocol("empty request head"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_target = parts.next().unwrap_or_default();

    match method {
        "CONNECT" => {
            let (host, port) = parse_authority(raw_target)?;
            Ok(RequestHead {
                target: RequestTarget::Tunnel,
                host,
                port,
            })
        }
        "GET" | "HEAD" | "POST" | "PUT" | "DELETE" | "OPTIONS" => {
            let rest = raw_target.strip_prefix("http://").ok_or(ExitError::Protocol(
                "non-CONNECT requests must use absolute http:// URIs (proxy-capable client required)",
            ))?;
            let authority = rest.split('/').next().unwrap_or_default();
            let (host, port) = parse_authority(authority)?;
            Ok(RequestHead {
                target: RequestTarget::Forward,
                host,
                port,
            })
        }
        _ => Err(ExitError::Protocol("unsupported method")),
    }
}

/// Parse `host:port` or `[ipv6]:port`. Default port 80 when absent.
fn parse_authority(authority: &str) -> Result<(String, u16), ExitError> {
    let authority = authority.trim();
    if authority.is_empty() {
        return Err(ExitError::Protocol("empty authority"));
    }

    // [ipv6]:port
    if let Some(rest) = authority.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or(ExitError::Protocol("unterminated [ipv6] authority"))?;
        let host = &rest[..close];
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| ExitError::Protocol("bad ipv6 authority"))?;
        let port = rest[close + 1..].strip_prefix(':').map_or(Ok(80), |p| {
            p.parse::<u16>()
                .map_err(|_| ExitError::Protocol("bad port"))
        })?;
        return Ok((host.to_string(), port));
    }

    // No brackets — IPv4 or hostname. rsplit, not split: only the last
    // `:` separates the port.
    let Some((host, port)) = authority.rsplit_once(':') else {
        if authority.contains(':') {
            return Err(ExitError::Protocol("bare ipv6 needs brackets and a port"));
        }
        return Ok((authority.to_string(), 80));
    };
    let port: u16 = port.parse().map_err(|_| ExitError::Protocol("bad port"))?;
    if host.is_empty() {
        return Err(ExitError::Protocol("empty host"));
    }
    Ok((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn head(line: &str) -> Vec<u8> {
        format!("{line}\r\nHost: example.com\r\n\r\n").into_bytes()
    }

    #[test]
    fn parses_connect_authority() {
        let parsed = parse_request_head(&head("CONNECT example.com:443 HTTP/1.1")).unwrap();
        assert_eq!(
            parsed,
            RequestHead {
                target: RequestTarget::Tunnel,
                host: "example.com".to_string(),
                port: 443,
            }
        );
    }

    #[test]
    fn parses_connect_ipv6_authority() {
        let parsed = parse_request_head(&head("CONNECT [200:1234::1]:8080 HTTP/1.1")).unwrap();
        assert_eq!(parsed.host, "200:1234::1");
        assert_eq!(parsed.port, 8080);
    }

    #[test]
    fn parses_absolute_uri_forward() {
        let parsed =
            parse_request_head(&head("GET http://example.com/dir/file.bin HTTP/1.1")).unwrap();
        assert_eq!(
            parsed,
            RequestHead {
                target: RequestTarget::Forward,
                host: "example.com".to_string(),
                port: 80,
            }
        );

        let parsed = parse_request_head(&head("GET http://example.com:8080/a HTTP/1.1")).unwrap();
        assert_eq!(parsed.host, "example.com");
        assert_eq!(parsed.port, 8080);
    }

    #[test]
    fn rejects_origin_form_and_junk() {
        // Origin-form (no absolute URI) is unusable for a proxy.
        assert!(parse_request_head(&head("GET /file.bin HTTP/1.1")).is_err());
        assert!(parse_request_head(&head("GET https://example.com/x HTTP/1.1")).is_err());
        assert!(parse_request_head(&head("FTP http://example.com/ HTTP/1.1")).is_err());
        assert!(parse_request_head(&head("CONNECT :443 HTTP/1.1")).is_err());
        assert!(parse_request_head(&head("")).is_err());
        assert!(parse_request_head(&[0xff, 0xfe, 0xfd]).is_err()); // not utf-8
    }
}
