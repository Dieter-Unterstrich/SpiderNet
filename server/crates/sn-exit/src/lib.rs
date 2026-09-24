//! sn-exit — the `SpiderNet` exit service.
//!
//! An exit is one household's opt-in offer to its neighbors: this binary
//! listens on the household's Yggdrasil overlay address and forwards
//! neighbor requests through the household's own internet uplink, under
//! a per-neighbor quota.
//!
//! Design (v0):
//!
//! - **Transparent byte relay.** CONNECT tunnels are relayed byte for
//!   byte (TLS stays end-to-end between neighbor and origin — the exit
//!   never sees plaintext HTTPS), plain proxy requests are relayed the
//!   same way. Range headers and retries keep working because headers
//!   are never rewritten; segmentation stays a client-side concern
//!   (`sn-fetch`).
//! - **SSRF protection.** Origins that resolve to loopback, private,
//!   link-local, ULA or Yggdrasil ranges are refused — a neighbor must
//!   not be able to reach the exit household's LAN, its admin sockets
//!   or other overlay nodes through this service.
//! - **Quota.** Per source address, per fixed window. Enforcement
//!   happens when a connection starts; the traffic a connection actually
//!   carries is accounted when it ends. A single very large connection
//!   can overshoot the quota — documented v0 limitation.
//!
//! Sharing is opt-in by running this binary; stopping it is the
//! kill-switch. Receiving without offering an exit is the explicitly
//! allowed leech mode — no punishment, see `[[Ethos]]` in the docs.
//!
//! `SpiderNet` software carries no warranty and accepts no responsibility
//! for its usage or for transported content (AGPL-3.0, sections 15/16).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod config;
pub mod error;
pub mod parse;
pub mod proxy;
