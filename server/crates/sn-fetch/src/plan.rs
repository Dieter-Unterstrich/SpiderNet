//! Planning: split a file into weighted byte-range segments across exits.

use crate::error::FetchError;
use crate::types::{
    ByteRange, ExitId, ExitWeight, FileSize, MinSegmentBytes, SegmentIndex, SegmentsPerExit,
    TargetUrl,
};
use std::num::NonZeroUsize;

/// One planned segment: a byte range assigned to one exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPlan {
    pub index: SegmentIndex,
    pub range: ByteRange,
    pub exit: ExitId,
}

/// A validated download plan.
#[derive(Debug, Clone)]
pub struct FetchPlan {
    pub url: TargetUrl,
    pub file_size: FileSize,
    pub segments: Vec<SegmentPlan>,
}

/// Exits with their capacity weights.
pub type ExitSpecs = Vec<(ExitId, ExitWeight)>;

impl FetchPlan {
    /// Build a validated plan.
    ///
    /// Exits are weighted by capacity; each exit's share is further split
    /// into `segments_per_exit` sub-segments. Segments smaller than
    /// `min_segment` are merged into a neighbour so we never spawn
    /// absurdly tiny requests — unless the whole file is that small.
    pub fn build(
        url: TargetUrl,
        file_size: FileSize,
        exits: &ExitSpecs,
        segments_per_exit: SegmentsPerExit,
        min_segment: MinSegmentBytes,
    ) -> Result<Self, FetchError> {
        if exits.is_empty() {
            return Err(FetchError::InvalidPlan("no exits given".to_string()));
        }

        let mut exits = exits.clone();

        // Drop the weakest exits (smallest weight; ties: higher id goes
        // first) until every remaining exit can hold at least one
        // minimum segment.
        while exits.len() > 1
            && u128::from(file_size.value())
                < u128::from(exits.len() as u64) * u128::from(min_segment.value())
        {
            let weakest = exits
                .iter()
                .enumerate()
                .min_by_key(|(_, (id, weight))| (weight.value(), std::cmp::Reverse(id.value())))
                .map_or(0, |(pos, _)| pos);
            exits.remove(weakest);
        }
        // Deterministic boundary assignment order.
        exits.sort_by_key(|(id, _)| *id);

        // If even one segment per exit would be too small, reduce to one
        // segment per exit.
        let spe_limit = usize::try_from(
            u128::from(file_size.value())
                / (u128::from(exits.len() as u64) * u128::from(min_segment.value())),
        )
        .unwrap_or(0);
        let spe = if spe_limit >= segments_per_exit.value() {
            segments_per_exit
        } else {
            SegmentsPerExit::new(NonZeroUsize::MIN)
        };

        let mut assignments: Vec<(ByteRange, ExitId)> = Vec::new();

        if exits.len() == 1 && spe.value() == 1 {
            let only = ByteRange::try_new(0, file_size.value() - 1)?;
            assignments.push((only, exits[0].0));
        } else {
            // Weighted split across exits, then even sub-split per exit.
            let units: Vec<u128> = exits
                .iter()
                .map(|(_, w)| u128::from(w.value()) * spe.value() as u128)
                .collect();
            let total_units: u128 = units.iter().sum();

            let size = u128::from(file_size.value());
            let mut cursor: u128 = 0;
            let mut start: u64 = 0;

            for (i, units_i) in units.iter().enumerate() {
                let is_last = i == exits.len() - 1;
                let end_exclusive: u128 = if is_last {
                    size
                } else {
                    cursor + (size * units_i) / total_units
                };

                let exit_end: u64 = end_exclusive
                    .try_into()
                    .map_err(|_| FetchError::InvalidPlan("boundary overflow".to_string()))?;
                let exit_end_inclusive = exit_end.saturating_sub(1);

                if exit_end_inclusive >= start {
                    for range in even_subsplit(start, exit_end_inclusive, spe.value()) {
                        assignments.push((range, exits[i].0));
                    }
                    start = exit_end;
                }

                cursor = end_exclusive;
            }

            // The last exit covers to the end, so nothing can be left
            // over; stay honest and extend the last assignment anyway.
            if start < file_size.value() {
                if let Some((last_range, _)) = assignments.last_mut() {
                    *last_range = ByteRange::try_new(last_range.start(), file_size.value() - 1)?;
                } else {
                    let only = ByteRange::try_new(0, file_size.value() - 1)?;
                    assignments.push((only, exits[0].0));
                }
            }
        }

        // Merge-fix: no segment below the minimum size (except a lone
        // tiny segment for a tiny file).
        let mut assignments = merge_tiny_segments(assignments, min_segment);

        let segments: Vec<SegmentPlan> = assignments
            .drain(..)
            .enumerate()
            .map(|(i, (range, exit))| {
                let index = u32::try_from(i)
                    .map(SegmentIndex::new)
                    .map_err(|_| FetchError::InvalidPlan("more than 2^32 segments".to_string()))?;
                Ok::<SegmentPlan, FetchError>(SegmentPlan { index, range, exit })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let plan = Self {
            url,
            file_size,
            segments,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Check invariants: contiguous, gap-free, complete coverage of the
    /// file, sequential indices, non-empty ranges.
    pub fn validate(&self) -> Result<(), FetchError> {
        let mut cursor: u64 = 0;
        for (expected_index, seg) in self.segments.iter().enumerate() {
            let expected_index = u32::try_from(expected_index)
                .map_err(|_| FetchError::InvalidPlan("more than 2^32 segments".to_string()))?;
            if seg.index.value() != expected_index {
                return Err(FetchError::InvalidPlan(format!(
                    "segment index out of order: {} != {expected_index}",
                    seg.index
                )));
            }
            if seg.range.start() != cursor {
                return Err(FetchError::InvalidPlan(format!(
                    "gap or overlap at segment {}: starts at {} but cursor is at {cursor}",
                    seg.index,
                    seg.range.start()
                )));
            }
            if seg.range.is_empty() {
                return Err(FetchError::InvalidPlan(format!(
                    "segment {} is empty",
                    seg.index
                )));
            }
            cursor = seg
                .range
                .end_inclusive()
                .checked_add(1)
                .ok_or_else(|| FetchError::InvalidPlan("range end overflow".to_string()))?;
        }
        if cursor != self.file_size.value() {
            return Err(FetchError::InvalidPlan(format!(
                "plan covers up to {cursor} but file size is {}",
                self.file_size
            )));
        }
        Ok(())
    }
}

/// Split `[start, end_inclusive]` into `parts` nearly-even sub-ranges,
/// ordered; the first `remainder` parts get one extra byte.
///
/// The caller guarantees `parts >= 1` and `end_inclusive >= start`, so
/// every part is non-empty and `try_new` cannot fail.
fn even_subsplit(start: u64, end_inclusive: u64, parts: usize) -> Vec<ByteRange> {
    let total_len = end_inclusive - start + 1;
    let base = total_len / parts as u64;
    let remainder = total_len % parts as u64;

    let mut out = Vec::with_capacity(parts);
    let mut cursor = start;
    for i in 0..parts {
        let len = base + u64::from((i as u64) < remainder);
        let range_end = cursor + len - 1;
        match ByteRange::try_new(cursor, range_end) {
            Ok(range) => out.push(range),
            // Unreachable by construction; keep the sub-range out instead
            // of panicking (validate() would then reject the plan).
            Err(_) => return Vec::new(),
        }
        cursor = range_end + 1;
    }
    out
}

/// Merge any segment smaller than `min_segment` into a neighbour
/// (prefer the next, fall back to the previous). A single remaining
/// segment is left as-is, however small.
fn merge_tiny_segments(
    mut assignments: Vec<(ByteRange, ExitId)>,
    min_segment: MinSegmentBytes,
) -> Vec<(ByteRange, ExitId)> {
    while let Some(pos) = assignments
        .iter()
        .position(|(range, _)| range.len() < min_segment.value())
    {
        if assignments.len() == 1 {
            break; // nothing left to merge into
        }

        let merged: (ByteRange, ExitId) = if pos + 1 < assignments.len() {
            // Merge into the next segment, keeping its exit.
            let next = &assignments[pos + 1];
            let Ok(range) = ByteRange::try_new(assignments[pos].0.start(), next.0.end_inclusive())
            else {
                // Unreachable: next.start == pos.end + 1, so the merged
                // range is always well-formed. Stop instead of panicking;
                // validate() would reject a broken plan.
                break;
            };
            (range, next.1)
        } else {
            // Last segment is too small: merge into the previous one.
            let prev_pos = pos - 1;
            let prev = &assignments[prev_pos];
            let Ok(range) = ByteRange::try_new(prev.0.start(), assignments[pos].0.end_inclusive())
            else {
                break;
            };
            (range, prev.1)
        };

        if pos + 1 < assignments.len() {
            assignments[pos + 1] = merged;
            assignments.remove(pos);
        } else {
            assignments[pos - 1] = merged;
            assignments.remove(pos);
        }
    }
    assignments
}
