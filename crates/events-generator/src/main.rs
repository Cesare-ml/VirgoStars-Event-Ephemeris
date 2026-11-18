/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */

use std::path::PathBuf;

use serde_json::json;
use virgostars_events_core::{BodyId, MotionDirection};
use virgostars_events_generator::provider_process::ProcessLongitudeProvider;
use virgostars_events_generator::{ingress_candidates, station_candidates, SearchWindow};

const USAGE: &str = "Usage: vseph-generate-candidates --mode stations|ingresses --body NAME --start-us I64 --end-us I64 --step-us I64 --precision-us I64 --provider-program PATH\nOnly prints unverified staging candidates as JSON; no dataset/DB changes.";

struct Request {
    mode: String,
    body: BodyId,
    window: SearchWindow,
    provider: PathBuf,
}

fn parse_request(args: impl IntoIterator<Item = String>) -> Result<Request, String> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(option) = args.next() {
        if !option.starts_with("--") || values.contains_key(&option) {
            return Err(format!("invalid or repeated argument: {option}"));
        }
        let value = args.next().ok_or(format!("missing value for {option}"))?;
        values.insert(option, value);
    }
    if values.len() != 7 {
        return Err("exactly seven required options are expected".into());
    }
    let required = |key: &str| {
        values
            .get(key)
            .map(String::as_str)
            .ok_or(format!("missing {key}"))
    };
    let mode = required("--mode")?.to_string();
    if mode != "stations" && mode != "ingresses" {
        return Err("--mode must be stations or ingresses".into());
    }
    let body = BodyId::try_from(required("--body")?)
        .map_err(|_| "invalid celestial body name".to_string())?;
    let integer = |key: &str| -> Result<i64, String> {
        required(key)?
            .parse()
            .map_err(|_| format!("{key} must be signed 64-bit integer"))
    };
    let window = SearchWindow {
        start_us: integer("--start-us")?,
        end_us: integer("--end-us")?,
        step_us: integer("--step-us")?,
        precision_us: integer("--precision-us")?,
    };
    let provider = PathBuf::from(required("--provider-program")?);
    if provider.as_os_str().is_empty() {
        return Err("provider executable path is empty".into());
    }
    Ok(Request {
        mode,
        body,
        window,
        provider,
    })
}

fn run() -> Result<(), String> {
    let options: Vec<_> = std::env::args().skip(1).collect();
    if options == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let request = parse_request(options)?;
    let provider = ProcessLongitudeProvider::spawn(&request.provider)?;
    let mut records = Vec::new();
    match request.mode.as_str() {
        "stations" => {
            for candidate in station_candidates(&provider, &request.body, request.window)
                .map_err(|error| error.to_string())?
            {
                records.push(json!({
                    "body": candidate.body.as_str(),
                    "bracket_start_us": candidate.bracket_start_us,
                    "bracket_end_us": candidate.bracket_end_us,
                    "direction_after": match candidate.direction_after {
                        MotionDirection::Direct => "direct",
                        MotionDirection::Retrograde => "retrograde",
                    }
                }));
            }
        }
        "ingresses" => {
            for candidate in ingress_candidates(&provider, &request.body, request.window)
                .map_err(|error| error.to_string())?
            {
                records.push(json!({
                    "body": candidate.body.as_str(),
                    "bracket_start_us": candidate.bracket_start_us,
                    "bracket_end_us": candidate.bracket_end_us,
                    "sign_after_index": candidate.sign_after.index(),
                    "retrograde": candidate.retrograde
                }));
            }
        }
        _ => unreachable!("validated mode"),
    }
    // Atomic stdout: never emit a partial staging artifact on a late provider error.
    let output = json!({
        "kind": "unverified_candidate_brackets",
        "coverage_certified": false,
        "provider": provider.identity(),
        "mode": request.mode,
        "window": {
            "start_us": request.window.start_us,
            "end_us": request.window.end_us,
            "step_us": request.window.step_us,
            "precision_us": request.window.precision_us
        },
        "candidates": records
    });
    println!("{output}");
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
    fn repeated_and_missing_options_fail_closed() {
        let valid = vec![
            "--mode",
            "stations",
            "--body",
            "Moon",
            "--start-us",
            "0",
            "--end-us",
            "1000",
            "--step-us",
            "50",
            "--precision-us",
            "1",
            "--provider-program",
            "path",
        ];
        assert!(parse_request(valid.iter().map(|s| s.to_string())).is_ok());
        let mut repeated = valid.clone();
        repeated.extend(["--mode", "ingresses"]);
        assert!(parse_request(repeated.iter().map(|s| s.to_string())).is_err());
        let mut missing = valid.clone();
        missing.truncate(missing.len() - 2);
        assert!(parse_request(missing.iter().map(|s| s.to_string())).is_err());
    }
}
