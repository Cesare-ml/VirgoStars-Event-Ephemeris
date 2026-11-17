/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Independent, read-only structural/provenance audit of gap-calculation staging.
//! A passing review is NEVER an astronomical verification or approval to import.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::json;
use virgostars_event_ephemeris::{BodyId, EventFamily, VirgoStarsEventEphemeris};
use virgostars_events_generator::gaps::detect_sparse_spans;
use virgostars_events_generator::provider_process::ProviderIdentity;

const MAX_STAGING_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CANDIDATES: usize = 100_000;
const USAGE: &str = "Usage: vseph-review-candidates --archive P0 --sha256 INDEPENDENT_TRUSTED_DIGEST --staging GENERATED_JSON\nRead-only: checks provenance, exact original sparse spans and candidate bracket structure. Does NOT verify astronomical events or certify coverage.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    sha256: String,
    format: String,
    blocks_verified: usize,
    records_verified: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Window {
    start_us: i64,
    end_us: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CalculationSettings {
    step_us: i64,
    precision_us: i64,
    chunk_us: i64,
    max_steps: u64,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
struct StagedSpan {
    kind: String,
    start_us: i64,
    end_us: i64,
    previous_original_source_id: Option<u64>,
    next_original_source_id: Option<u64>,
    previous_recorded_us: Option<i64>,
    next_recorded_us: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedCandidate {
    gap_index: usize,
    family: String,
    body: String,
    bracket_start_us: i64,
    bracket_end_us: i64,
    #[serde(default)]
    direction_after: Option<String>,
    #[serde(default)]
    sign_after_index: Option<u8>,
    #[serde(default)]
    retrograde: Option<bool>,
    #[serde(default)]
    phase: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Staging {
    kind: String,
    coverage_certified: bool,
    generation_complete: bool,
    dataset: Dataset,
    mode: String,
    family: String,
    body: String,
    requested_window: Window,
    sparse_threshold_us: i64,
    calculation_settings: Option<CalculationSettings>,
    provider: Option<ProviderIdentity>,
    sparse_spans: Vec<StagedSpan>,
    candidates: Vec<StagedCandidate>,
}

fn options(args: impl IntoIterator<Item = String>) -> Result<(PathBuf, String, PathBuf), String> {
    let mut archive = None;
    let mut sha256 = None;
    let mut staging = None;
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        let value = args.next().ok_or(format!("missing value for {key}"))?;
        let slot = match key.as_str() {
            "--archive" => &mut archive,
            "--sha256" => &mut sha256,
            "--staging" => &mut staging,
            _ => return Err(format!("unrecognized option {key}")),
        };
        if slot.replace(value).is_some() {
            return Err(format!("repeated option {key}"));
        }
    }
    Ok((
        PathBuf::from(archive.ok_or("missing --archive")?),
        sha256.ok_or("missing --sha256")?,
        PathBuf::from(staging.ok_or("missing --staging")?),
    ))
}

fn check_staging(
    stage: &Staging,
    archive: &VirgoStarsEventEphemeris,
) -> Result<(usize, usize), String> {
    if stage.kind != "unverified_sparse_interval_staging"
        || stage.coverage_certified
        || stage.generation_complete
    {
        return Err("staging kind or unverified status is invalid".into());
    }
    if stage.dataset.sha256 != archive.verified_sha256()
        || stage.dataset.format != "VSEPH-P0-EXPERIMENTAL"
        || stage.dataset.blocks_verified != archive.audit().blocks
        || stage.dataset.records_verified != archive.audit().records
    {
        return Err(
            "staging dataset identity/audit differs from the independently trusted archive".into(),
        );
    }

    let family = match stage.family.as_str() {
        "synods" => EventFamily::Synod,
        "motions" => EventFamily::Motion,
        "ingresses" => EventFamily::Ingress,
        _ => return Err("unsupported staged family".into()),
    };
    let body =
        BodyId::try_from(stage.body.as_str()).map_err(|_| "invalid staged body".to_string())?;
    let expected = detect_sparse_spans(
        archive.raw_archive(),
        family,
        &body,
        stage.requested_window.start_us,
        stage.requested_window.end_us,
        stage.sparse_threshold_us,
    )
    .map_err(|error| error.to_string())?;
    if expected.len() != stage.sparse_spans.len() {
        return Err("staged sparse-span count differs from the authenticated archive".into());
    }
    for (index, (actual, source)) in stage.sparse_spans.iter().zip(expected.iter()).enumerate() {
        let expected = StagedSpan {
            kind: source.kind.code().into(),
            start_us: source.start_us,
            end_us: source.end_us,
            previous_original_source_id: source.previous_original_source_id,
            next_original_source_id: source.next_original_source_id,
            previous_recorded_us: source.previous_recorded_us,
            next_recorded_us: source.next_recorded_us,
        };
        if actual != &expected {
            return Err(format!(
                "staged sparse span #{index} does not match original record anchors"
            ));
        }
    }

    if stage.candidates.len() > MAX_CANDIDATES {
        return Err("candidate review count exceeds hard limit".into());
    }
    let precision = match stage.mode.as_str() {
        "plan" => {
            if stage.provider.is_some()
                || stage.calculation_settings.is_some()
                || !stage.candidates.is_empty()
            {
                return Err("plan mode cannot contain provider, settings or candidates".into());
            }
            None
        }
        "calculate" => {
            if stage.family == "synods" && stage.body != "Moon" {
                return Err("non-Moon synods are not implemented".into());
            }
            let settings = stage
                .calculation_settings
                .as_ref()
                .ok_or("missing calculation settings")?;
            if settings.step_us <= 0
                || settings.precision_us <= 0
                || settings.precision_us > settings.step_us
                || settings.chunk_us < settings.step_us
                || settings.max_steps == 0
                || settings.max_steps > 1_000_000
            {
                return Err("invalid calculation settings".into());
            }
            if !stage.sparse_spans.is_empty() {
                let identity = stage
                    .provider
                    .as_ref()
                    .ok_or("missing calculation provider identity")?;
                if identity.protocol != "virgostars.longitude.v2"
                    || identity.frame != "geocentric-tropical-ecliptic-degrees"
                    || identity.input_time_scale != "utc-posix-microseconds"
                    || identity.provider.trim().is_empty()
                    || identity.version.trim().is_empty()
                    || identity.flags.trim().is_empty()
                {
                    return Err("invalid ephemeris provider identity".into());
                }
            } else if stage.provider.is_some() {
                return Err("provider reported despite no calculated sparse spans".into());
            }
            Some(settings.precision_us)
        }
        _ => return Err("unknown staging mode".into()),
    };

    let mut keys = BTreeSet::new();
    let mut last_key = None;
    let mut touching_original = 0;
    for (index, candidate) in stage.candidates.iter().enumerate() {
        if candidate.family != stage.family || candidate.body != stage.body {
            return Err(format!(
                "candidate #{index} family/body does not match staging"
            ));
        }
        let span = stage
            .sparse_spans
            .get(candidate.gap_index)
            .ok_or(format!("candidate #{index} has invalid gap index"))?;
        let width = candidate.bracket_end_us as i128 - candidate.bracket_start_us as i128;
        if candidate.bracket_start_us < span.start_us
            || candidate.bracket_end_us > span.end_us
            || width <= 0
            || width > precision.unwrap_or(0) as i128
        {
            return Err(format!(
                "candidate #{index} bracket exceeds its source span or declared precision"
            ));
        }
        let fields_valid = match family {
            EventFamily::Synod => {
                matches!(
                    candidate.phase.as_deref(),
                    Some("new_moon" | "first_quarter" | "full_moon" | "last_quarter")
                ) && candidate.direction_after.is_none()
                    && candidate.sign_after_index.is_none()
                    && candidate.retrograde.is_none()
            }
            EventFamily::Motion => {
                matches!(
                    candidate.direction_after.as_deref(),
                    Some("direct" | "retrograde")
                ) && candidate.phase.is_none()
                    && candidate.sign_after_index.is_none()
                    && candidate.retrograde.is_none()
            }
            EventFamily::Ingress => {
                candidate.sign_after_index.is_some_and(|sign| sign < 12)
                    && candidate.retrograde.is_some()
                    && candidate.phase.is_none()
                    && candidate.direction_after.is_none()
            }
        };
        if !fields_valid {
            return Err(format!(
                "candidate #{index} has invalid family-specific fields"
            ));
        }
        let key = (
            candidate.gap_index,
            candidate.bracket_start_us,
            candidate.bracket_end_us,
        );
        if last_key.is_some_and(|previous| key < previous) || !keys.insert(key) {
            return Err(format!(
                "candidate #{index} is out of order or duplicates a bracket"
            ));
        }
        last_key = Some(key);
        if archive
            .next(family, &stage.body, candidate.bracket_start_us)
            .map_err(|error| error.to_string())?
            .is_some_and(|record| record.at().as_i64() <= candidate.bracket_end_us)
        {
            // A bracket can touch a recorded endpoint. This is a review signal,
            // not permission to overwrite the source event or assign an ID.
            touching_original += 1;
        }
    }
    Ok((stage.candidates.len(), touching_original))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let (archive_path, digest, staging_path) = options(args)?;
    let archive = VirgoStarsEventEphemeris::open_verified(&archive_path, &digest)
        .map_err(|error| error.to_string())?;
    let meta =
        fs::metadata(&staging_path).map_err(|error| format!("cannot stat staging: {error}"))?;
    if !meta.is_file() || meta.len() > MAX_STAGING_BYTES {
        return Err("staging must be a regular file <= 32 MiB".into());
    }
    let raw = fs::read(&staging_path).map_err(|error| format!("cannot read staging: {error}"))?;
    if raw.len() as u64 > MAX_STAGING_BYTES {
        return Err("staging changed or exceeded byte limit during read".into());
    }
    let staging: Staging = serde_json::from_slice(&raw)
        .map_err(|error| format!("invalid staging JSON schema: {error}"))?;
    let (count, touching) = check_staging(&staging, &archive)?;
    println!(
        "{}",
        json!({
            "kind": "structural_candidate_review",
            "staging_structure_verified": true,
            "source_archive_authenticated": true,
            "source_archive_sha256": archive.verified_sha256(),
            "original_sparse_spans_recomputed": staging.sparse_spans.len(),
            "candidate_brackets_checked": count,
            "brackets_touching_recorded_anchors": touching,
            "coverage_certified": false,
            "astronomical_accuracy_verified": false,
            "safe_to_import": false,
            "next_step": "independent scientific comparison, source semantics and owner approval required"
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
