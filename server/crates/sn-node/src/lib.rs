//! sn-node — `SpiderNet` node helper for the Yggdrasil overlay.
//!
//! `SpiderNet` is software for neighborhood fiber meshes: households peer
//! their internet uplinks and pool bandwidth. This crate is the node-side
//! glue for the overlay layer:
//!
//! - [`config`]: generate a Yggdrasil configuration file from validated,
//!   typed `SpiderNet` settings (peers, listeners, admin socket, allowed
//!   neighbor keys).
//! - [`yggdrasil`]: client for the Yggdrasil admin socket (JSON over
//!   TCP/unix, default `localhost:9001`) to query node status (`getSelf`,
//!   `getPeers`).
//!
//! Yggdrasil provides encrypted end-to-end routing between households.
//! It is **not** an anonymity network, and `SpiderNet` never claims to
//! provide anonymity.
//!
//! `SpiderNet` software carries no warranty and accepts no responsibility
//! for its usage or for transported content (AGPL-3.0, sections 15/16).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod config;
pub mod error;
pub mod yggdrasil;
