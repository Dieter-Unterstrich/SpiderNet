//! sn-fetch — segmented multi-exit downloader for `SpiderNet`.
//!
//! `PoC` v0: probes a URL for HTTP range support, plans weighted byte-range
//! segments across exits, downloads segments in parallel and reassembles
//! the target file with SHA-256 verification.
//!
//! `SpiderNet` software carries no warranty and accepts no responsibility
//! for its usage or for transported content (AGPL-3.0, sections 15/16).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// deny is the goal before the first release
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod error;
pub mod exec;
pub mod exit;
pub mod plan;
pub mod probe;
pub mod progress;
pub mod runner;
pub mod types;
