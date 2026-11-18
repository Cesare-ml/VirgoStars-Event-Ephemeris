/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Development-only two-source latitude audit. NEVER publishes or imports data.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};
use virgostars_events_core::BodyId;
use virgostars_events_generator::latitude_provider_compare::{
    compare_latitude_event_providers, compare_latitude_provider_samples, LatitudeComparisonOptions,
    LatitudeComparisonTolerance, LatitudeObservedCandidate, LatitudeSampleComparisonOptions,
    LatitudeSampleTolerance,
};
use virgostars_events_generator::provider_process::{
    LatitudeProviderIdentity, ProcessLatitudeProvider,
};
use virgostars_events_generator::SearchWindow;

const USAGE: &str = "Usage: vseph-compare-latitudes \
--mode samples|events|both --body NAME --start-us I64 --end-us I64 \
--step-us POSITIVE_I64 --precision-us POSITIVE_I64 \
--primary-program PATH --reference-program PATH \
--latitude-tolerance-deg NONNEGATIVE_F64 \
--speed-tolerance-deg-per-day NONNEGATIVE_F64 \
--event-time-tolerance-us NONNEGATIVE_I64 \
--peak-speed-tolerance-deg-per-day NONNEGATIVE_F64 \
--adaptive-depth 1..8 --max-probes 1..100000\n\
This experimental comparison never certifies scientific completeness or imports data.\n\
Exit 0: no observed disagreement; 1: observed disagreement (JSON still printed); \
2: invalid input/provider failure (no JSON).";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Samples,
    Events,
    Both,
}

impl Mode {
    fn samples(self) -> bool {
        matches!(self, Self::Samples | Self::Both)
    }

    fn events(self) -> bool {
        matches!(self, Self::Events | Self::Both)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Samples => "samples",
            Self::Events => "events",
            Self::Both => "both",
        }
    }
}

struct Request {
    mode: Mode,
    body: BodyId,
    window: SearchWindow,
    primary_program: PathBuf,
    reference_program: PathBuf,
    latitude_tolerance: f64,
    speed_tolerance: f64,
    event_time_tolerance: i64,
    peak_speed_tolerance: f64,
    adaptive_depth: u8,
    max_probes: usize,
}

fn parse_request(args: impl IntoIterator<Item = String>) -> Result<Request, String> {
    let mut values = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(option) = args.next() {
        if !option.starts_with("--") || values.contains_key(&option) {
            return Err(format!("invalid or repeated option: {option}"));
        }
        let value = args.next().ok_or(format!("missing value for {option}"))?;
        values.insert(option, value);
    }
    if values.len() != 14 {
        return Err("exactly fourteen required options are expected".into());
    }
    let required = |key: &str| -> Result<&str, String> {
        values
            .get(key)
            .map(String::as_str)
            .ok_or(format!("missing {key}"))
    };
    let mode = match required("--mode")? {
        "samples" => Mode::Samples,
        "events" => Mode::Events,
        "both" => Mode::Both,
        _ => return Err("--mode must be samples, events or both".into()),
    };
    let body = BodyId::try_from(required("--body")?)
        .map_err(|_| "invalid celestial body name".to_string())?;
    let integer = |key: &str| -> Result<i64, String> {
        required(key)?
            .parse()
            .map_err(|_| format!("{key} must be a signed 64-bit integer"))
    };
    let nonnegative_finite = |key: &str| -> Result<f64, String> {
        let value: f64 = required(key)?
            .parse()
            .map_err(|_| format!("{key} must be a finite nonnegative number"))?;
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{key} must be a finite nonnegative number"));
        }
        Ok(value)
    };
    let window = SearchWindow {
        start_us: integer("--start-us")?,
        end_us: integer("--end-us")?,
        step_us: integer("--step-us")?,
        precision_us: integer("--precision-us")?,
    };
    if window.start_us >= window.end_us
        || window.step_us <= 0
        || window.precision_us <= 0
        || window.precision_us > window.step_us
    {
        return Err("invalid half-open UTC window, step or precision".into());
    }
    let span = window.end_us as i128 - window.start_us as i128;
    let steps = (span + window.step_us as i128 - 1) / window.step_us as i128;
    if steps > 100_000 {
        return Err("UTC grid exceeds 100000 points".into());
    }
    let primary_program = PathBuf::from(required("--primary-program")?);
    let reference_program = PathBuf::from(required("--reference-program")?);
    if primary_program.as_os_str().is_empty() || reference_program.as_os_str().is_empty() {
        return Err("both provider paths must be nonempty".into());
    }
    let event_time_tolerance = integer("--event-time-tolerance-us")?;
    if event_time_tolerance < 0 {
        return Err("event-time tolerance must be nonnegative".into());
    }
    let adaptive_depth: u8 = required("--adaptive-depth")?
        .parse()
        .map_err(|_| "adaptive depth must be 1..8".to_string())?;
    if !(1..=8).contains(&adaptive_depth) {
        return Err("adaptive depth must be 1..8".into());
    }
    if mode.events()
        && (window.step_us as i128) / (1_i128 << (adaptive_depth as u32 + 2))
            < window.precision_us as i128
    {
        return Err("adaptive depth exceeds step/precision resolution".into());
    }
    let max_probes: usize = required("--max-probes")?
        .parse()
        .map_err(|_| "max probes must be 1..100000".to_string())?;
    if !(1..=100_000).contains(&max_probes) {
        return Err("max probes must be 1..100000".into());
    }
    Ok(Request {
        mode,
        body,
        window,
        primary_program,
        reference_program,
        latitude_tolerance: nonnegative_finite("--latitude-tolerance-deg")?,
        speed_tolerance: nonnegative_finite("--speed-tolerance-deg-per-day")?,
        event_time_tolerance,
        peak_speed_tolerance: nonnegative_finite("--peak-speed-tolerance-deg-per-day")?,
        adaptive_depth,
        max_probes,
    })
}

fn check_matching_conventions(
    a: &LatitudeProviderIdentity,
    b: &LatitudeProviderIdentity,
) -> Result<(), String> {
    if a.frame != b.frame
        || a.ecliptic_plane != b.ecliptic_plane
        || a.position_corrections != b.position_corrections
        || a.dynamical_time_scale != b.dynamical_time_scale
        || a.input_time_scale != b.input_time_scale
        || a.speed_definition != b.speed_definition
    {
        return Err("latitude provider reference planes/time scales/corrections differ".into());
    }
    Ok(())
}

fn candidate_json(candidate: &LatitudeObservedCandidate) -> Value {
    json!({
        "body": candidate.body.as_str(),
        "kind": format!("{:?}", candidate.kind),
        "bracket_start_us": candidate.bracket_start_us.to_string(),
        "bracket_end_us": candidate.bracket_end_us.to_string(),
        "signed_speed_degrees_per_day": candidate.signed_speed_degrees_per_day,
    })
}

/// Returns whether any sampled disagreement was observed. On provider errors,
/// returns without printing even a partially computed JSON document.
fn run(request: Request) -> Result<bool, String> {
    let primary = ProcessLatitudeProvider::spawn(&request.primary_program)?;
    let reference = ProcessLatitudeProvider::spawn(&request.reference_program)?;
    check_matching_conventions(primary.identity(), reference.identity())?;

    let mut disagreement = false;
    let samples = if request.mode.samples() {
        let (mut a_budget, mut b_budget) = (request.max_probes, request.max_probes);
        let report = compare_latitude_provider_samples(
            &primary,
            &reference,
            &request.body,
            LatitudeSampleComparisonOptions {
                window: request.window,
                tolerance: LatitudeSampleTolerance {
                    latitude_degrees: request.latitude_tolerance,
                    signed_speed_degrees_per_day: request.speed_tolerance,
                },
            },
            &mut a_budget,
            &mut b_budget,
        )
        .map_err(|error| error.to_string())?;
        disagreement |= report.has_sample_disagreements();
        Some(json!({
            "samples_checked": report.samples_checked,
            "primary_probes_used": request.max_probes - a_budget,
            "reference_probes_used": request.max_probes - b_budget,
            "disagreements": report.disagreements.iter().map(|entry| json!({
                "at_us": entry.at_us.to_string(),
                "latitude_offset_degrees": entry.latitude_offset_degrees,
                "speed_offset_degrees_per_day": entry.signed_speed_offset_degrees_per_day,
                "latitude_within_tolerance": entry.latitude_within_tolerance,
                "speed_within_tolerance": entry.speed_within_tolerance,
            })).collect::<Vec<_>>(),
            "maximum_absolute_latitude_offset_degrees": report.maximum_absolute_latitude_offset_degrees,
            "maximum_absolute_speed_offset_degrees_per_day": report.maximum_absolute_speed_offset_degrees_per_day,
            "maximum_latitude_offset_at_us": report.maximum_latitude_offset_at_us.map(|us| us.to_string()),
            "maximum_speed_offset_at_us": report.maximum_speed_offset_at_us.map(|us| us.to_string()),
        }))
    } else {
        None
    };

    let events = if request.mode.events() {
        let (mut a_budget, mut b_budget) = (request.max_probes, request.max_probes);
        let report = compare_latitude_event_providers(
            &primary,
            &reference,
            &request.body,
            LatitudeComparisonOptions {
                window: request.window,
                max_depth: request.adaptive_depth,
                tolerance: LatitudeComparisonTolerance {
                    time_us: request.event_time_tolerance,
                    signed_speed_degrees_per_day: request.peak_speed_tolerance,
                },
            },
            &mut a_budget,
            &mut b_budget,
        )
        .map_err(|error| error.to_string())?;
        disagreement |= report.has_observed_disagreements();
        Some(json!({
            "primary_probes_used": request.max_probes - a_budget,
            "reference_probes_used": request.max_probes - b_budget,
            "pairs": report.pairs.iter().map(|pair| json!({
                "primary": candidate_json(&pair.primary),
                "reference": candidate_json(&pair.reference),
                "reference_offset_us": pair.reference_offset_us.to_string(),
                "signed_speed_offset_degrees_per_day": pair.signed_speed_offset_degrees_per_day,
                "speed_within_tolerance": pair.speed_within_tolerance,
            })).collect::<Vec<_>>(),
            "unmatched_primary": report.unmatched_primary.iter().map(candidate_json).collect::<Vec<_>>(),
            "unmatched_reference": report.unmatched_reference.iter().map(candidate_json).collect::<Vec<_>>(),
            "primary_interior_probes": {
                "plane_crossings": report.primary_interior_probes.plane_crossings,
                "speed_stations": report.primary_interior_probes.speed_stations,
                "speed_peaks": report.primary_interior_probes.speed_peaks,
            },
            "reference_interior_probes": {
                "plane_crossings": report.reference_interior_probes.plane_crossings,
                "speed_stations": report.reference_interior_probes.speed_stations,
                "speed_peaks": report.reference_interior_probes.speed_peaks,
            },
        }))
    } else {
        None
    };

    let output = json!({
        "kind": "experimental_latitude_provider_comparison",
        "body": request.body.as_str(),
        "mode": request.mode.name(),
        "window": {
            "start_us": request.window.start_us.to_string(),
            "end_us": request.window.end_us.to_string(),
            "step_us": request.window.step_us.to_string(),
            "precision_us": request.window.precision_us.to_string(),
        },
        "tolerances": {
            "latitude_degrees": request.latitude_tolerance,
            "speed_degrees_per_day": request.speed_tolerance,
            "event_time_us": request.event_time_tolerance.to_string(),
            "peak_speed_degrees_per_day": request.peak_speed_tolerance,
        },
        "primary_provider": primary.identity(),
        "reference_provider": reference.identity(),
        "identities_equal": primary.identity() == reference.identity(),
        "samples": samples,
        "events": events,
        "observed_disagreements": disagreement,
        "provider_independence_verified": false,
        "astronomical_accuracy_verified": false,
        "coverage_certified": false,
        "safe_to_import": false,
    });
    let rendered = serde_json::to_string_pretty(&output)
        .map_err(|error| format!("cannot encode latitude audit: {error}"))?;
    println!("{rendered}");
    Ok(disagreement)
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return;
    }
    match parse_request(args).and_then(run) {
        Ok(false) => {}
        Ok(true) => {
            eprintln!("observed latitude differences exceed configured tolerances");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Vec<String> {
        [
            "--mode",
            "samples",
            "--body",
            "Mars",
            "--start-us",
            "0",
            "--end-us",
            "2000000",
            "--step-us",
            "1000000",
            "--precision-us",
            "1000000",
            "--primary-program",
            "first",
            "--reference-program",
            "second",
            "--latitude-tolerance-deg",
            "0.001",
            "--speed-tolerance-deg-per-day",
            "0.001",
            "--event-time-tolerance-us",
            "1000",
            "--peak-speed-tolerance-deg-per-day",
            "0.01",
            "--adaptive-depth",
            "1",
            "--max-probes",
            "100",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
    }

    #[test]
    fn parses_explicit_window_and_all_required_tolerances() {
        let request = parse_request(options()).unwrap();
        assert_eq!(request.mode, Mode::Samples);
        assert_eq!(request.window.end_us, 2_000_000);
        assert_eq!(request.max_probes, 100);
    }

    #[test]
    fn rejects_unbounded_invalid_or_ambiguous_options_before_spawning() {
        let base = options();
        for (key, value) in [
            ("--start-us", "2000000"),
            ("--step-us", "0"),
            ("--precision-us", "2000000"),
            ("--latitude-tolerance-deg", "NaN"),
            ("--speed-tolerance-deg-per-day", "inf"),
            ("--event-time-tolerance-us", "-1"),
            ("--adaptive-depth", "0"),
            ("--max-probes", "0"),
            ("--max-probes", "100001"),
            ("--mode", "unknown"),
            ("--reference-program", ""),
        ] {
            let mut modified = base.clone();
            let index = modified.iter().position(|arg| arg == key).unwrap();
            modified[index + 1] = value.to_string();
            assert!(parse_request(modified).is_err(), "{key}={value}");
        }
        let mut duplicate = base.clone();
        duplicate.extend(["--mode".into(), "both".into()]);
        assert!(parse_request(duplicate).is_err());
        let mut unknown = base;
        unknown[0] = "--typo".into();
        assert!(parse_request(unknown).is_err());
    }
}
