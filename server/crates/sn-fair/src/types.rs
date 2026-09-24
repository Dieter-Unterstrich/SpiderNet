//! Domain types: newtypes over raw primitives, validated at construction.

use std::fmt;
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use std::str::FromStr;

use crate::error::FairnessError;

/// Identifier of a participant (a household) in the fairness allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticipantId(NonZeroU16);

impl ParticipantId {
    pub const fn new(value: NonZeroU16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0.get()
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

/// Parses a decimal id; rejects zero and non-numeric input.
impl FromStr for ParticipantId {
    type Err = FairnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = s.trim().parse::<u16>().ok().and_then(NonZeroU16::new);
        parsed
            .map(Self::new)
            .ok_or_else(|| FairnessError::InvalidParticipantId(s.to_string()))
    }
}

/// Relative share shaping of a contributing participant. 1 means equal
/// share; a participant with weight 2 gets twice the share of one with
/// weight 1 (as far as demands allow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weight(NonZeroU32);

impl Weight {
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0.get()
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self(NonZeroU32::MIN)
    }
}

/// A traffic rate, unit-agnostic: bytes per second, Mbit/s — whatever
/// unit the caller works with (the CLI treats it as Mbit/s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rate(u64);

impl Rate {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Total poolable capacity shared by all participants; strictly positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Capacity(NonZeroU64);

impl Capacity {
    pub fn try_new(value: u64) -> Result<Self, FairnessError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(FairnessError::ZeroCapacity)
    }

    pub const fn value(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for Capacity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn participant_id_parses_decimal_and_rejects_zero_and_garbage() {
        assert_eq!("7".parse::<ParticipantId>().unwrap().value(), 7);
        assert_eq!("  42 ".parse::<ParticipantId>().unwrap().value(), 42);
        assert!("0".parse::<ParticipantId>().is_err());
        assert!("abc".parse::<ParticipantId>().is_err());
        assert!("-1".parse::<ParticipantId>().is_err());
        assert!("70000".parse::<ParticipantId>().is_err());
        assert!("".parse::<ParticipantId>().is_err());
    }

    #[test]
    fn weight_defaults_to_one() {
        assert_eq!(Weight::default().value(), 1);
    }

    #[test]
    fn capacity_rejects_zero() {
        assert!(Capacity::try_new(0).is_err());
        assert_eq!(Capacity::try_new(5).unwrap().value(), 5);
    }
}
