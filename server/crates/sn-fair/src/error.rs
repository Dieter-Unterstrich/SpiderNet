//! Error type for the fairness scheduler.

use crate::types::ParticipantId;

#[derive(Debug, thiserror::Error)]
pub enum FairnessError {
    #[error("capacity must be greater than zero")]
    ZeroCapacity,

    #[error("invalid participant id `{0}`: expected a decimal number greater than zero")]
    InvalidParticipantId(String),

    #[error("demand references unknown participant {0}")]
    UnknownParticipant(ParticipantId),

    #[error("duplicate demand for participant {0}")]
    DuplicateDemand(ParticipantId),

    #[error("duplicate participant {0}")]
    DuplicateParticipant(ParticipantId),

    #[error("conflicting participation classes for participant {0}")]
    ConflictingParticipation(ParticipantId),
}
