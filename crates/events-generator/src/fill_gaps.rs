/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Offline review/staging of sparse intervals in an approved P0 snapshot.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use virgostars_event_ephemeris::{BodyId, EventFamily, MotionDirection, VirgoStarsEventEphemeris};
use virgostars_events_generator::gaps::detect_sparse_spans;
use virgostars_events_generator::lunar::lunar_phase_candidates;
use virgostars_events_generator::provider_process::ProcessLongitudeProvider;
use virgostars_events_generator::{ingress_candidates, station_candidates, SearchWindow};

const DEFAULT_CHUNK_US: i64 = 365 * 86_400_000_000;
const DEFAULT_MAX_STEPS: u64 = 100_000;
const HARD_MAX_STEPS: u64 = 1_000_000;
const USAGE: &str = "Usage: vseph-fill-gaps --mode plan|calculate --archive P0 --sha256 TRUSTED_DIGEST --family synods|motions|ingresses --body NAME --start-us I64 --end-us I64 --threshold-us I64 [--provider-program EXEC --step-us I64 --precision-us I64] [--chunk-us I64] [--max-steps U64] [--cadence-check half|none]\nOnly synods/Moon, motions, ingresses can be calculated; others are report-only. Optional half-step rescan catches some missed events, NOT scientific completeness. Never writes P0, original IDs or database.";

struct Options {
    mode: String,
    archive: PathBuf,
    digest: String,
    family: EventFamily,
    family_name: String,
    body: BodyId,
    start_us: i64,
    end_us: i64,
    threshold_us: i64,
    provider_program: Option<PathBuf>,
    step_us: Option<i64>,
    precision_us: Option<i64>,
    chunk_us: i64,
    max_steps: u64,
    verify_half_step: bool,
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut map = BTreeMap::new();
    let mut args = args.into_iter();
    const NAMES: &[&str] = &[
        "--mode",
        "--archive",
        "--sha256",
        "--family",
        "--body",
        "--start-us",
        "--end-us",
        "--threshold-us",
        "--provider-program",
        "--step-us",
        "--precision-us",
        "--chunk-us",
        "--max-steps",
        "--cadence-check",
    ];
    while let Some(key) = args.next() {
        if !NAMES.contains(&key.as_str()) || map.contains_key(&key) {
            return Err(format!("unknown or repeated option {key}"));
        }
        map.insert(
            key.clone(),
            args.next().ok_or(format!("missing {key} value"))?,
        );
    }
    let required = |key: &str| {
        map.get(key)
            .map(String::as_str)
            .ok_or(format!("missing {key}"))
    };
    let number = |key: &str| -> Result<i64, String> {
        required(key)?
            .parse::<i64>()
            .map_err(|_| format!("invalid signed 64-bit integer for {key}"))
    };
    let mode = required("--mode")?.to_string();
    if mode != "plan" && mode != "calculate" {
        return Err("--mode must be plan or calculate".into());
    }
    let family_name = required("--family")?.to_string();
    let family = match family_name.as_str() {
        "synods" => EventFamily::Synod,
        "motions" => EventFamily::Motion,
        "ingresses" => EventFamily::Ingress,
        _ => return Err("unsupported family; use synods, motions or ingresses".into()),
    };
    let body = BodyId::try_from(required("--body")?)
        .map_err(|_| "invalid celestial body name".to_string())?;
    let start_us = number("--start-us")?;
    let end_us = number("--end-us")?;
    let threshold_us = number("--threshold-us")?;
    if end_us <= start_us || threshold_us <= 0 {
        return Err("invalid range or threshold".into());
    }
    let chunk_us = map
        .get("--chunk-us")
        .map(|_| number("--chunk-us"))
        .transpose()?
        .unwrap_or(DEFAULT_CHUNK_US);
    if chunk_us <= 0 {
        return Err("--chunk-us must be positive".into());
    }
    let max_steps = map
        .get("--max-steps")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "invalid --max-steps".to_string())
        })
        .transpose()?
        .unwrap_or(DEFAULT_MAX_STEPS);
    if max_steps == 0 || max_steps > HARD_MAX_STEPS {
        return Err("--max-steps must be in 1..=1000000".into());
    }
    let verify_half_step = match map.get("--cadence-check").map(String::as_str) {
        None | Some("none") => false,
        Some("half") => true,
        Some(_) => return Err("--cadence-check must be half or none".into()),
    };
    let provider_program = map.get("--provider-program").map(PathBuf::from);
    let step_us = map
        .get("--step-us")
        .map(|_| number("--step-us"))
        .transpose()?;
    let precision_us = map
        .get("--precision-us")
        .map(|_| number("--precision-us"))
        .transpose()?;
    if mode == "calculate" {
        if provider_program.is_none()
            || step_us.map_or(true, |x| x <= 0)
            || precision_us.map_or(true, |x| x <= 0)
            || precision_us > step_us
            || chunk_us < step_us.unwrap_or(0)
        {
            return Err(
                "calculate requires a provider, valid step/precision and chunk >= step".into(),
            );
        }
        if family == EventFamily::Synod && body.as_str() != "Moon" {
            return Err("only Moon synod phase candidates are currently implemented".into());
        }
        if verify_half_step
            && step_us.expect("validated step") / 2 < precision_us.expect("validated precision")
        {
            return Err("half-step verification requires step_us / 2 >= precision_us".into());
        }
    } else if provider_program.is_some() || step_us.is_some() || precision_us.is_some() {
        return Err("plan does not invoke a provider; remove calculation options".into());
    } else if verify_half_step {
        return Err("half-step verification requires calculate mode".into());
    }
    Ok(Options {
        mode,
        archive: PathBuf::from(required("--archive")?),
        digest: required("--sha256")?.to_string(),
        family,
        family_name,
        body,
        start_us,
        end_us,
        threshold_us,
        provider_program,
        step_us,
        precision_us,
        chunk_us,
        max_steps,
        verify_half_step,
    })
}

fn generate_chunk(
    provider: &ProcessLongitudeProvider,
    family: EventFamily,
    body: &BodyId,
    gap_index: usize,
    window: SearchWindow,
) -> Result<Vec<Value>, String> {
    match family {
        EventFamily::Motion => station_candidates(provider, body, window)
            .map_err(|error| error.to_string())
            .map(|found| {
                found
                    .into_iter()
                    .map(|candidate| {
                        json!({
                            "gap_index": gap_index,
                            "family": "motions",
                            "body": candidate.body.as_str(),
                            "bracket_start_us": candidate.bracket_start_us,
                            "bracket_end_us": candidate.bracket_end_us,
                            "direction_after": match candidate.direction_after {
                                MotionDirection::Direct => "direct",
                                MotionDirection::Retrograde => "retrograde",
                            },
                        })
                    })
                    .collect()
            }),
        EventFamily::Ingress => ingress_candidates(provider, body, window)
            .map_err(|error| error.to_string())
            .map(|found| {
                found
                    .into_iter()
                    .map(|candidate| {
                        json!({
                            "gap_index": gap_index,
                            "family": "ingresses",
                            "body": candidate.body.as_str(),
                            "bracket_start_us": candidate.bracket_start_us,
                            "bracket_end_us": candidate.bracket_end_us,
                            "sign_after_index": candidate.sign_after.index(),
                            "retrograde": candidate.retrograde,
                        })
                    })
                    .collect()
            }),
        EventFamily::Synod => lunar_phase_candidates(provider, window)
            .map_err(|error| error.to_string())
            .map(|found| {
                found
                    .into_iter()
                    .map(|candidate| {
                        json!({
                            "gap_index": gap_index,
                            "family": "synods",
                            "body": "Moon",
                            "phase": candidate.phase.code(),
                            "bracket_start_us": candidate.bracket_start_us,
                            "bracket_end_us": candidate.bracket_end_us,
                        })
                    })
                    .collect()
            }),
    }
}

fn compare_half_step(coarse: &[Value], fine: &[Value], precision_us: i64) -> Result<(), String> {
    if coarse.len() != fine.len() {
        return Err(format!(
            "half-step rescan found {} candidates, initial scan found {}; possible missed crossings",
            fine.len(),
            coarse.len()
        ));
    }
    for (index, (original, checked)) in coarse.iter().zip(fine).enumerate() {
        for field in [
            "gap_index",
            "family",
            "body",
            "phase",
            "direction_after",
            "sign_after_index",
            "retrograde",
        ] {
            if original.get(field) != checked.get(field) {
                return Err(format!(
                    "half-step candidate #{index} changed event identity at {field}"
                ));
            }
        }
        let midpoint = |event: &Value| -> Result<i128, String> {
            let start = event["bracket_start_us"]
                .as_i64()
                .ok_or("invalid first bracket endpoint")?;
            let end = event["bracket_end_us"]
                .as_i64()
                .ok_or("invalid second bracket endpoint")?;
            Ok((start as i128 + end as i128) / 2)
        };
        if (midpoint(original)? - midpoint(checked)?).abs() > precision_us as i128 {
            return Err(format!(
                "half-step candidate #{index} moved by more than the requested precision"
            ));
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let archive = VirgoStarsEventEphemeris::open_verified(&options.archive, &options.digest)
        .map_err(|error| error.to_string())?;
    let spans = detect_sparse_spans(
        archive.raw_archive(),
        options.family,
        &options.body,
        options.start_us,
        options.end_us,
        options.threshold_us,
    )
    .map_err(|error| error.to_string())?;

    let gaps: Vec<Value> = spans
        .iter()
        .map(|span| {
            json!({
                "kind": span.kind.code(),
                "start_us": span.start_us,
                "end_us": span.end_us,
                "previous_original_source_id": span.previous_original_source_id,
                "next_original_source_id": span.next_original_source_id,
                "previous_recorded_us": span.previous_recorded_us,
                "next_recorded_us": span.next_recorded_us,
            })
        })
        .collect();
    let mut candidates: Vec<Value> = Vec::new();
    let mut provider_identity = Value::Null;
    if options.mode == "calculate" && !spans.is_empty() {
        let step = options.step_us.expect("validated step");
        let precision = options.precision_us.expect("validated precision");
        let mut chunks = Vec::new();
        let mut total_steps: i128 = 0;
        for (gap_index, gap) in spans.iter().enumerate() {
            let mut start = gap.start_us;
            while start < gap.end_us {
                let end = (start as i128 + options.chunk_us as i128).min(gap.end_us as i128) as i64;
                let span_len = end as i128 - start as i128;
                total_steps += (span_len + step as i128 - 1) / step as i128;
                if options.verify_half_step {
                    let half = (step / 2) as i128;
                    total_steps += (span_len + half - 1) / half;
                }
                if total_steps > options.max_steps as i128 {
                    return Err(format!(
                        "calculation needs {total_steps} position steps: split the window or raise --max-steps (max 1000000)"
                    ));
                }
                chunks.push((
                    gap_index,
                    SearchWindow {
                        start_us: start,
                        end_us: end,
                        step_us: step,
                        precision_us: precision,
                    },
                ));
                start = end;
            }
        }
        // Open the separately licensed provider only *after* budget checks.
        let provider = ProcessLongitudeProvider::spawn(
            options
                .provider_program
                .as_ref()
                .expect("validated provider"),
        )?;
        provider_identity = serde_json::to_value(provider.identity())
            .map_err(|error| format!("cannot serialize provider identity: {error}"))?;
        for (gap_index, window) in chunks {
            let found =
                generate_chunk(&provider, options.family, &options.body, gap_index, window)?;
            if options.verify_half_step {
                let fine = generate_chunk(
                    &provider,
                    options.family,
                    &options.body,
                    gap_index,
                    SearchWindow {
                        step_us: step / 2,
                        ..window
                    },
                )?;
                compare_half_step(&found, &fine, precision)?;
            }
            candidates.extend(found);
        }
    }
    // Atomic single output: no partly calculated output after provider errors.
    println!(
        "{}",
        json!({
            "kind": "unverified_sparse_interval_staging",
            "coverage_certified": false,
            "generation_complete": false,
            "dataset": {
                "sha256": archive.verified_sha256(),
                "format": "VSEPH-P0-EXPERIMENTAL",
                "blocks_verified": archive.audit().blocks,
                "records_verified": archive.audit().records,
            },
            "mode": options.mode,
            "family": options.family_name,
            "body": options.body.as_str(),
            "requested_window": {"start_us": options.start_us, "end_us": options.end_us},
            "sparse_threshold_us": options.threshold_us,
            "calculation_settings": if options.mode == "calculate" {
                json!({
                    "step_us": options.step_us.expect("validated step"),
                    "precision_us": options.precision_us.expect("validated precision"),
                    "chunk_us": options.chunk_us,
                    "max_steps": options.max_steps,
                })
            } else { Value::Null },
            "provider": provider_identity,
            "cadence_check": if options.verify_half_step && !spans.is_empty() {
                json!({
                    "mode": "half_step",
                    "step_us": options.step_us.expect("validated step") / 2,
                    "matched_candidates": candidates.len(),
                    "coverage_certified": false,
                })
            } else { Value::Null },
            "sparse_spans": gaps,
            "candidates": candidates,
        })
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}\n{USAGE}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_family_modes_and_step_budgets() {
        let options = vec![
            "--mode",
            "plan",
            "--archive",
            "local.p0",
            "--sha256",
            "sha",
            "--family",
            "synods",
            "--body",
            "Moon",
            "--start-us",
            "0",
            "--end-us",
            "1000",
            "--threshold-us",
            "100",
        ];
        assert!(parse_options(options.iter().map(|s| s.to_string())).is_ok());
        let mut bad = options.clone();
        bad.extend(["--provider-program", "node"]);
        assert!(parse_options(bad.iter().map(|s| s.to_string())).is_err());
        let mut calculate = options.clone();
        calculate[1] = "calculate";
        calculate.extend([
            "--provider-program",
            "mock",
            "--step-us",
            "100",
            "--precision-us",
            "1",
        ]);
        assert!(parse_options(calculate.iter().map(|s| s.to_string())).is_ok());
        calculate[7] = "Mars";
        assert!(parse_options(calculate.iter().map(|s| s.to_string())).is_err());
    }
}
