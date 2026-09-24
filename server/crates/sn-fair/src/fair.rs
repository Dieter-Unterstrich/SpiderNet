//! Two-class weighted max-min fairness allocation.
//!
//! Participants are split into two priority classes:
//!
//! 1. **Contributors** ([`Participation::Contributing`]) — households that
//!    offer their own uplink to the pool. They are served first, with
//!    shares shaped by their [`Weight`] via weighted max-min fairness
//!    over the *full* capacity.
//! 2. **Leeches** ([`Participation::Leeching`]) — receive-only
//!    households. Leech mode is explicitly allowed (the network exists
//!    under neighbors, not against them). Leeches have the lowest but
//!    still fair priority: when contributors saturate the capacity,
//!    leeches get (near) nothing; when contributors are undersubscribed,
//!    leeches benefit from the leftovers. Nobody is dropped from the
//!    system.
//!
//! See the crate root for the phase overview.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::FairnessError;
use crate::types::{Capacity, ParticipantId, Rate, Weight};

/// How a household takes part in the neighborhood network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Participation {
    /// The household offers its own uplink to the pool; `weight` shapes
    /// its share of the capacity (1 = equal share).
    Contributing { weight: Weight },
    /// The household only receives (leech mode, explicitly allowed).
    /// Leeches share whatever contributors leave unused, with implicit
    /// equal weights, capped by their demand.
    Leeching,
}

/// A participant of the allocation: an id plus its participation class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Participant {
    id: ParticipantId,
    participation: Participation,
}

impl Participant {
    pub const fn new(id: ParticipantId, participation: Participation) -> Self {
        Self { id, participation }
    }

    pub const fn id(&self) -> ParticipantId {
        self.id
    }

    pub const fn participation(&self) -> Participation {
        self.participation
    }
}

/// Requested rate of a participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demand {
    participant: ParticipantId,
    rate: Rate,
}

impl Demand {
    pub const fn new(participant: ParticipantId, rate: Rate) -> Self {
        Self { participant, rate }
    }

    pub const fn participant(&self) -> ParticipantId {
        self.participant
    }

    pub const fn rate(&self) -> Rate {
        self.rate
    }
}

/// Granted rate for a participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    participant: ParticipantId,
    rate: Rate,
}

impl Allocation {
    pub const fn new(participant: ParticipantId, rate: Rate) -> Self {
        Self { participant, rate }
    }

    pub const fn participant(&self) -> ParticipantId {
        self.participant
    }

    pub const fn rate(&self) -> Rate {
        self.rate
    }
}

/// Result of [`allocate`]: per-participant rates plus the totals.
///
/// Allocations are keyed by participant id in a `BTreeMap`, so iteration
/// is always in ascending id order (deterministic output).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FairShare {
    allocations: BTreeMap<ParticipantId, Rate>,
    total_allocated: Rate,
    unused: Rate,
}

impl FairShare {
    /// Allocations ordered by participant id. Participants without a
    /// demand at all are absent; use [`FairShare::rate_of`] to read a
    /// single participant's rate.
    pub fn allocations(&self) -> &BTreeMap<ParticipantId, Rate> {
        &self.allocations
    }

    /// Allocations as [`Allocation`] values, in participant-id order.
    pub fn iter(&self) -> impl Iterator<Item = Allocation> + '_ {
        self.allocations
            .iter()
            .map(|(&participant, &rate)| Allocation::new(participant, rate))
    }

    /// Allocated rate of one participant (zero if it received nothing).
    pub fn rate_of(&self, participant: ParticipantId) -> Rate {
        self.allocations
            .get(&participant)
            .copied()
            .unwrap_or(Rate::new(0))
    }

    pub const fn total_allocated(&self) -> Rate {
        self.total_allocated
    }

    pub const fn unused(&self) -> Rate {
        self.unused
    }
}

/// Allocates `capacity` among `participants` according to their `demands`.
///
/// Phase 1 runs weighted max-min fairness among contributors over the
/// full capacity; phase 2 runs weighted max-min among leeches (implicit
/// equal weights) over the capacity the contributors leave unused.
///
/// Empty `demands` is valid: nothing is allocated, everything stays
/// unused. Participants without a demand (or with demand 0) receive
/// nothing.
///
/// Errors:
/// - [`FairnessError::ZeroCapacity`] — capacity is zero (cannot happen
///   with a validated [`Capacity`])
/// - [`FairnessError::UnknownParticipant`] — a demand references an id
///   that is not among `participants`
/// - [`FairnessError::DuplicateDemand`] — two demands for the same id
/// - [`FairnessError::DuplicateParticipant`] — the same id appears twice
///   among `participants` with the same participation class
/// - [`FairnessError::ConflictingParticipation`] — the same id appears
///   twice among `participants` with different participation classes
pub fn allocate(
    capacity: Capacity,
    participants: &[Participant],
    demands: &[Demand],
) -> Result<FairShare, FairnessError> {
    let mut participation_of: BTreeMap<ParticipantId, Participation> = BTreeMap::new();
    for participant in participants {
        if let Some(previous) =
            participation_of.insert(participant.id(), participant.participation())
        {
            if previous == participant.participation() {
                return Err(FairnessError::DuplicateParticipant(participant.id()));
            }
            return Err(FairnessError::ConflictingParticipation(participant.id()));
        }
    }

    for demand in demands {
        if !participation_of.contains_key(&demand.participant()) {
            return Err(FairnessError::UnknownParticipant(demand.participant()));
        }
    }

    let mut demand_of: BTreeMap<ParticipantId, u64> = BTreeMap::new();
    for demand in demands {
        if demand_of
            .insert(demand.participant(), demand.rate().value())
            .is_some()
        {
            return Err(FairnessError::DuplicateDemand(demand.participant()));
        }
    }

    let mut contributors = Vec::new();
    let mut leeches = Vec::new();
    for (&id, &participation) in &participation_of {
        let demand = demand_of.get(&id).copied().unwrap_or(0);
        match participation {
            Participation::Contributing { weight } => contributors.push(Entry {
                id,
                demand,
                weight: weight.value(),
            }),
            Participation::Leeching => leeches.push(Entry {
                id,
                demand,
                weight: 1,
            }),
        }
    }

    let phase_one = weighted_max_min(contributors, capacity.value());
    let allocated: u128 = phase_one.values().map(|rate| u128::from(*rate)).sum();
    let leftover = capacity.value().saturating_sub(to_u64(allocated));

    let phase_two = weighted_max_min(leeches, leftover);

    let mut allocations = phase_one;
    allocations.extend(phase_two);

    let total: u128 = allocations.values().map(|rate| u128::from(*rate)).sum();
    let total = to_u64(total);
    let unused = capacity.value().saturating_sub(total);

    Ok(FairShare {
        allocations: allocations
            .into_iter()
            .map(|(id, rate)| (id, Rate::new(rate)))
            .collect(),
        total_allocated: Rate::new(total),
        unused: Rate::new(unused),
    })
}

/// Internal per-participant input for the max-min helper.
struct Entry {
    id: ParticipantId,
    demand: u64,
    weight: u32,
}

/// Weighted max-min fairness over `entries` with (at most) `capacity` to
/// distribute. Deterministic: participants are processed in participant-id
/// order and the result is a `BTreeMap`.
///
/// Loop: each round hands out proportional shares
/// `share_i = capacity * weight_i / total_weight` (computed in `u128`,
/// floored). Participants whose demand fits their share are satisfied and
/// leave the active set together with their allocation; the rest re-share
/// the remaining capacity. When nobody's demand fits, everyone gets its
/// floor share and the integer-rounding remainder is distributed
/// deterministically in participant-id order, one unit at a time, only to
/// participants still below their demand.
///
/// Guarantees (asserted by tests): the sum of allocations never exceeds
/// `capacity`; when every remaining demand is at least its computed
/// share, the capacity is distributed in full.
fn weighted_max_min(entries: Vec<Entry>, capacity: u64) -> BTreeMap<ParticipantId, u64> {
    let mut result = BTreeMap::new();
    let mut active: Vec<Entry> = Vec::new();

    // Zero demand is trivially satisfied with an allocation of zero.
    for entry in entries {
        if entry.demand == 0 {
            result.insert(entry.id, 0);
        } else {
            active.push(entry);
        }
    }
    active.sort_by_key(|entry| entry.id);

    let mut capacity = capacity;
    while capacity > 0 && !active.is_empty() {
        let total_weight: u128 = active.iter().map(|entry| u128::from(entry.weight)).sum();
        let shares: Vec<u128> = active
            .iter()
            .map(|entry| u128::from(capacity) * u128::from(entry.weight) / total_weight)
            .collect();

        let mut satisfied = Vec::new();
        for (index, entry) in active.iter().enumerate() {
            if u128::from(entry.demand) <= shares[index] {
                satisfied.push(index);
            }
        }

        if satisfied.is_empty() {
            // Every demand is above its share: hand out the floor shares,
            // then spread the rounding remainder one unit at a time. The
            // remainder is smaller than the number of active participants
            // and every one of them still has room, so it is distributed
            // completely.
            for (index, entry) in active.iter().enumerate() {
                result.insert(entry.id, to_u64(shares[index]));
            }
            let distributed: u128 = shares.iter().sum();
            let mut remainder = u128::from(capacity) - distributed;
            while remainder > 0 {
                let mut handed_out = 0;
                for entry in &active {
                    if remainder == 0 {
                        break;
                    }
                    if let Some(allocation) = result.get_mut(&entry.id) {
                        if *allocation < entry.demand {
                            *allocation += 1;
                            remainder -= 1;
                            handed_out += 1;
                        }
                    }
                }
                if handed_out == 0 {
                    break;
                }
            }
            active.clear();
            break;
        }

        let allocated: u64 = satisfied.iter().map(|&index| active[index].demand).sum();
        let satisfied_set: BTreeSet<usize> = satisfied.into_iter().collect();
        let mut remaining = Vec::new();
        for (index, entry) in active.into_iter().enumerate() {
            if satisfied_set.contains(&index) {
                result.insert(entry.id, entry.demand);
            } else {
                remaining.push(entry);
            }
        }
        active = remaining;
        capacity = capacity.saturating_sub(allocated);
    }

    // Capacity exhausted (or nothing to distribute): whoever is still
    // active receives nothing.
    for entry in active {
        result.insert(entry.id, 0);
    }
    result
}

/// All values in the allocation arithmetic are bounded by the `u64`
/// capacity, so this conversion cannot truncate; `u64::MAX` is a safe
/// fallback that keeps the code panic-free.
fn to_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::num::{NonZeroU16, NonZeroU32};

    use super::*;

    fn id(value: u16) -> ParticipantId {
        ParticipantId::new(NonZeroU16::new(value).unwrap())
    }

    fn contributing(participant: u16, weight: u32) -> Participant {
        Participant::new(
            id(participant),
            Participation::Contributing {
                weight: Weight::new(NonZeroU32::new(weight).unwrap()),
            },
        )
    }

    fn leeching(participant: u16) -> Participant {
        Participant::new(id(participant), Participation::Leeching)
    }

    fn demand(participant: u16, rate: u64) -> Demand {
        Demand::new(id(participant), Rate::new(rate))
    }

    fn capacity(value: u64) -> Capacity {
        Capacity::try_new(value).unwrap()
    }

    fn rate_of(share: &FairShare, participant: u16) -> u64 {
        share.rate_of(id(participant)).value()
    }

    #[test]
    fn equal_demands_equal_weights_split_equally() {
        let share = allocate(
            capacity(800),
            &[contributing(1, 1), contributing(2, 1)],
            &[demand(1, 800), demand(2, 800)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 400);
        assert_eq!(rate_of(&share, 2), 400);
        assert_eq!(share.total_allocated().value(), 800);
        assert_eq!(share.unused().value(), 0);
    }

    #[test]
    fn small_demand_is_satisfied_first_then_remainder_is_redistributed() {
        let share = allocate(
            capacity(900),
            &[contributing(1, 1), contributing(2, 1)],
            &[demand(1, 100), demand(2, 900)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 100);
        assert_eq!(rate_of(&share, 2), 800);
    }

    #[test]
    fn weights_shape_the_split_two_to_one() {
        let share = allocate(
            capacity(900),
            &[contributing(1, 2), contributing(2, 1)],
            &[demand(1, 900), demand(2, 900)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 600);
        assert_eq!(rate_of(&share, 2), 300);
    }

    #[test]
    fn rounding_remainder_is_distributed_in_id_order() {
        let share = allocate(
            capacity(1001),
            &[contributing(1, 1), contributing(2, 1)],
            &[demand(1, 1000), demand(2, 1000)],
        )
        .unwrap();
        // Shares are 500 + 500, the extra unit goes to the lower id.
        assert_eq!(rate_of(&share, 1), 501);
        assert_eq!(rate_of(&share, 2), 500);
        assert_eq!(share.total_allocated().value(), 1001);
    }

    #[test]
    fn saturated_contributor_leaves_nothing_for_the_leech() {
        let share = allocate(
            capacity(100),
            &[contributing(1, 1), leeching(2)],
            &[demand(1, 200), demand(2, 50)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 100);
        assert_eq!(rate_of(&share, 2), 0);
    }

    #[test]
    fn undersubscribed_contributor_leaves_leftover_for_the_leech() {
        let share = allocate(
            capacity(100),
            &[contributing(1, 1), leeching(2)],
            &[demand(1, 60), demand(2, 100)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 60);
        assert_eq!(rate_of(&share, 2), 40);
    }

    #[test]
    fn two_leeches_split_leftover_equally_capped_by_demand() {
        let share = allocate(
            capacity(100),
            &[contributing(1, 1), leeching(2), leeching(3)],
            &[demand(1, 50), demand(2, 100), demand(3, 20)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 50);
        // Leftover 50: leech 3 (demand 20) is satisfied first, leech 2
        // gets the rest.
        assert_eq!(rate_of(&share, 2), 30);
        assert_eq!(rate_of(&share, 3), 20);
    }

    #[test]
    fn capacity_below_all_demands_yields_proportional_weighted_split() {
        let share = allocate(
            capacity(30),
            &[contributing(1, 2), contributing(2, 1)],
            &[demand(1, 100), demand(2, 100)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 20);
        assert_eq!(rate_of(&share, 2), 10);
        assert_eq!(share.unused().value(), 0);
    }

    #[test]
    fn zero_demand_receives_zero_without_consuming_capacity() {
        let share = allocate(
            capacity(800),
            &[contributing(1, 1), contributing(2, 1)],
            &[demand(1, 0), demand(2, 500)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 0);
        assert_eq!(share.allocations().get(&id(1)), Some(&Rate::new(0)));
        assert_eq!(rate_of(&share, 2), 500);
        assert_eq!(share.unused().value(), 300);
    }

    #[test]
    fn empty_demands_allocates_nothing_and_everything_is_unused() {
        let share = allocate(capacity(500), &[contributing(1, 1), leeching(2)], &[]).unwrap();
        assert_eq!(share.total_allocated().value(), 0);
        assert_eq!(share.unused().value(), 500);
        assert!(share.allocations().values().all(|rate| rate.value() == 0));
    }

    #[test]
    fn unknown_participant_in_demand_is_rejected() {
        let error = allocate(capacity(100), &[contributing(1, 1)], &[demand(9, 50)]).unwrap_err();
        assert!(matches!(error, FairnessError::UnknownParticipant(_)));
    }

    #[test]
    fn duplicate_demand_is_rejected() {
        let error = allocate(
            capacity(100),
            &[contributing(1, 1)],
            &[demand(1, 50), demand(1, 20)],
        )
        .unwrap_err();
        assert!(matches!(error, FairnessError::DuplicateDemand(_)));
    }

    #[test]
    fn duplicate_participant_with_same_class_is_rejected() {
        let error = allocate(
            capacity(100),
            &[contributing(1, 1), contributing(1, 1)],
            &[],
        )
        .unwrap_err();
        assert!(matches!(error, FairnessError::DuplicateParticipant(_)));
    }

    #[test]
    fn duplicate_participant_with_different_class_is_rejected() {
        let error = allocate(capacity(100), &[contributing(1, 1), leeching(1)], &[]).unwrap_err();
        assert!(matches!(error, FairnessError::ConflictingParticipation(_)));
    }

    #[test]
    fn all_demands_at_least_share_distributes_capacity_in_full() {
        let share = allocate(
            capacity(1000),
            &[contributing(1, 1), contributing(2, 1), contributing(3, 2)],
            &[demand(1, 1000), demand(2, 1000), demand(3, 1000)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 250);
        assert_eq!(rate_of(&share, 2), 250);
        assert_eq!(rate_of(&share, 3), 500);
        assert_eq!(share.total_allocated().value(), 1000);
        assert_eq!(share.unused().value(), 0);
    }

    /// Roadmap pilot scenario: two household downloads (contributing,
    /// equal) plus one leech competing for the pooled uplinks.
    #[test]
    fn pilot_scenario_two_households_and_a_leech() {
        let share = allocate(
            capacity(600),
            &[contributing(1, 1), contributing(2, 1), leeching(3)],
            &[demand(1, 250), demand(2, 250), demand(3, 150)],
        )
        .unwrap();
        assert_eq!(rate_of(&share, 1), 250);
        assert_eq!(rate_of(&share, 2), 250);
        assert_eq!(rate_of(&share, 3), 100);
        assert_eq!(share.total_allocated().value(), 600);
        assert_eq!(share.unused().value(), 0);
    }
}
