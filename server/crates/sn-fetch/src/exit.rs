//! Exit abstraction: an "exit" is one internet uplink through which
//! segment requests can be sent.
//!
//! PoC v0 ships only [`LocalExit`] (the household's own connection, no
//! detour). The registry/trait structure is deliberately ready so a
//! future exit implementation can route HTTP through a neighbour over
//! the mesh (e.g. via a per-exit proxy or tunnel) without changing the
//! planner or executor.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::error::FetchError;
use crate::types::ExitId;

/// An internet uplink able to issue HTTP requests.
pub trait HttpExit: Send + Sync {
    fn id(&self) -> ExitId;

    /// Client whose connections go out through this exit.
    fn client(&self) -> &reqwest::Client;
}

/// The household's own uplink: plain outgoing HTTP, no detour.
#[derive(Debug)]
pub struct LocalExit {
    id: ExitId,
    client: reqwest::Client,
}

impl LocalExit {
    pub fn new(id: ExitId) -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { id, client })
    }
}

impl HttpExit for LocalExit {
    fn id(&self) -> ExitId {
        self.id
    }

    fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

/// All exits available for a fetch, keyed by id.
#[derive(Default, Clone)]
pub struct ExitRegistry {
    exits: BTreeMap<ExitId, Arc<dyn HttpExit>>,
}

impl ExitRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn register(&mut self, exit: Arc<dyn HttpExit>) {
        self.exits.insert(exit.id(), exit);
    }

    pub fn get(&self, id: ExitId) -> Result<&Arc<dyn HttpExit>, FetchError> {
        self.exits.get(&id).ok_or(FetchError::UnknownExit(id))
    }

    pub fn ids(&self) -> Vec<ExitId> {
        self.exits.keys().copied().collect()
    }
}
