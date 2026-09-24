//! sn-fair — weighted max-min fairness scheduler for `SpiderNet`.
//!
//! Allocates a shared capacity (e.g. the pooled uplink bandwidth of a
//! neighborhood) among participants with a two-class weighted max-min
//! fairness scheme:
//!
//! 1. **Contributors** (households that offer capacity to the network,
//!    with a configurable weight) are served first, via weighted max-min
//!    fairness over the full capacity.
//! 2. **Leeches** (receive-only households; explicitly allowed — the
//!    network exists under neighbors, not against them) share whatever
//!    the contributors leave unused, weighted max-min with implicit equal
//!    weights, capped by their demand.
//!
//! Rationale: leeches have the lowest but still fair priority. When
//! contributors saturate the capacity, leeches get (near) nothing;
//! otherwise they benefit from the leftovers. No one is dropped from the
//! system.
//!
//! Pure computation: no I/O, no async runtime.
//!
//! `SpiderNet` software carries no warranty and accepts no responsibility
//! for its usage or for transported content (AGPL-3.0, sections 15/16 —
//! responsibility for usage lies with the users).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// deny is the goal before the first release
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

pub mod error;
pub mod fair;
pub mod types;

pub use error::FairnessError;
pub use fair::{allocate, Allocation, Demand, FairShare, Participant, Participation};
pub use types::{Capacity, ParticipantId, Rate, Weight};
