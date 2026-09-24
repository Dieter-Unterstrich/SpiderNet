//! CLI for the sn-fair fairness scheduler.
//!
//! KEINE GARANTIE — this software is provided "as is" without warranty of
//! any kind (AGPL-3.0, sections 15/16). Users are solely responsible for
//! what they transmit or receive with it.
//!
//! Pilot scope: simulates how pooled neighborhood capacity is split —
//! e.g. two household downloads plus a leech competing for the pooled
//! uplinks.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
// PoC scope: doc-annotation noise, not correctness. Revisit at release.
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::process::ExitCode;

use clap::Parser;
use sn_fair::fair::{Demand, Participant, Participation};
use sn_fair::types::{Capacity, ParticipantId, Rate, Weight};
use sn_fair::{allocate, FairShare};

/// Parses `id=contributing[:weight]` or `id=leeching`.
fn parse_participant_spec(spec: &str) -> Result<(ParticipantId, Participation), String> {
    let (id_raw, class_raw) = spec.split_once('=').ok_or_else(|| {
        format!("invalid participant spec `{spec}`, expected `id=contributing[:weight]` or `id=leeching`")
    })?;
    let participant = id_raw
        .trim()
        .parse::<ParticipantId>()
        .map_err(|err| format!("invalid participant spec `{spec}`: {err}"))?;
    let class = class_raw.trim();
    let participation = match class.strip_prefix("contributing") {
        Some(rest) => {
            let weight = if rest.is_empty() {
                Weight::default()
            } else if let Some(weight_raw) = rest.strip_prefix(':') {
                weight_raw
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .and_then(NonZeroU32::new)
                    .map(Weight::new)
                    .ok_or_else(|| {
                        format!("invalid weight `{weight_raw}` in `{spec}` (must be a non-zero number)")
                    })?
            } else {
                return Err(format!(
                    "invalid participation `{class}` in `{spec}`, expected `contributing[:weight]` or `leeching`"
                ));
            };
            Participation::Contributing { weight }
        }
        None if class == "leeching" => Participation::Leeching,
        None => {
            return Err(format!(
                "invalid participation `{class}` in `{spec}`, expected `contributing[:weight]` or `leeching`"
            ))
        }
    };
    Ok((participant, participation))
}

/// Parses `id=rate`.
fn parse_demand_spec(spec: &str) -> Result<(ParticipantId, Rate), String> {
    let (id_raw, rate_raw) = spec
        .split_once('=')
        .ok_or_else(|| format!("invalid demand spec `{spec}`, expected `id=rate`"))?;
    let participant = id_raw
        .trim()
        .parse::<ParticipantId>()
        .map_err(|err| format!("invalid demand spec `{spec}`: {err}"))?;
    let rate = rate_raw.trim().parse::<u64>().map(Rate::new).map_err(|_| {
        format!("invalid rate `{rate_raw}` in `{spec}` (must be a non-negative number)")
    })?;
    Ok((participant, rate))
}

#[derive(Debug, Parser)]
#[command(
    name = "sn-fair",
    version,
    about = "SpiderNet weighted max-min fairness scheduler (pilot)"
)]
struct Args {
    /// Total pooled capacity, in Mbit/s.
    #[arg(long)]
    capacity: u64,

    /// Participant: `id=contributing[:weight]` (offers its uplink,
    /// weight shapes the share, default weight 1) or `id=leeching`
    /// (receive-only). Repeatable.
    #[arg(
        long = "participant",
        value_name = "ID=CLASS[:WEIGHT]",
        value_parser = parse_participant_spec
    )]
    participants: Vec<(ParticipantId, Participation)>,

    /// Demand: `id=rate` in Mbit/s. Repeatable; participants without a
    /// demand receive nothing.
    #[arg(
        long = "demand",
        value_name = "ID=RATE",
        value_parser = parse_demand_spec
    )]
    demands: Vec<(ParticipantId, Rate)>,
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let capacity = Capacity::try_new(args.capacity).map_err(|err| err.to_string())?;

    let participants: Vec<Participant> = args
        .participants
        .into_iter()
        .map(|(id, participation)| Participant::new(id, participation))
        .collect();
    let demand_of: BTreeMap<ParticipantId, u64> = args
        .demands
        .iter()
        .map(|(participant, rate)| (*participant, rate.value()))
        .collect();
    let demands: Vec<Demand> = args
        .demands
        .into_iter()
        .map(|(participant, rate)| Demand::new(participant, rate))
        .collect();

    let share = allocate(capacity, &participants, &demands).map_err(|err| err.to_string())?;
    print_report(capacity, &participants, &demand_of, &share);
    Ok(())
}

fn print_report(
    capacity: Capacity,
    participants: &[Participant],
    demand_of: &BTreeMap<ParticipantId, u64>,
    share: &FairShare,
) {
    println!("NO WARRANTY — SpiderNet sn-fair (AGPL-3.0, no liability)");
    println!();
    println!("pooled capacity: {} Mbit/s", capacity.value());
    println!();

    let headers = ["participant", "class", "weight", "demand", "allocation"];
    let mut rows = Vec::new();
    for participant in participants {
        let (class, weight) = match participant.participation() {
            Participation::Contributing { weight } => ("contributing", weight.value().to_string()),
            Participation::Leeching => ("leeching", "-".to_string()),
        };
        let demand = demand_of
            .get(&participant.id())
            .map_or_else(|| "0".to_string(), ToString::to_string);
        rows.push([
            participant.id().to_string(),
            class.to_string(),
            weight,
            demand,
            share.rate_of(participant.id()).to_string(),
        ]);
    }

    let mut widths = [0usize; 5];
    for (index, header) in headers.iter().enumerate() {
        widths[index] = header.len();
    }
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    print_row(&headers.map(ToString::to_string), &widths);
    for row in &rows {
        print_row(row, &widths);
    }

    println!();
    println!("total allocated: {}", share.total_allocated());
    println!("unused:          {}", share.unused());
}

fn print_row(cells: &[String], widths: &[usize]) {
    println!(
        "{:<w0$}  {:<w1$}  {:>w2$}  {:>w3$}  {:>w4$}",
        cells[0],
        cells[1],
        cells[2],
        cells[3],
        cells[4],
        w0 = widths[0],
        w1 = widths[1],
        w2 = widths[2],
        w3 = widths[3],
        w4 = widths[4],
    );
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(report) => {
            eprintln!("NO WARRANTY — SpiderNet sn-fair (AGPL-3.0, no liability)");
            eprintln!("error: {report}");
            ExitCode::FAILURE
        }
    }
}
