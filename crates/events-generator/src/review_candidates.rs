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
use sha2::{Digest, Sha256};
use virgostars_event_ephemeris::{BodyId, EventFamily, TimestampMicros, VirgoStarsEventEphemeris};
use virgostars_events_generator::gaps::detect_sparse_spans;
use virgostars_events_generator::provider_process::{ProcessLongitudeProvider, ProviderIdentity};
use virgostars_events_generator::{LongitudeProvider, LongitudeSample};

const MAX_STAGING_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CANDIDATES: usize = 100_000;
const MAX_REFERENCE_CANDIDATES: usize = 10_000;
const MAX_REFERENCE_BYTES: u64 = 16 * 1024 * 1024;
const USAGE: &str = "Usage: vseph-review-candidates --archive P0 --sha256 INDEPENDENT_TRUSTED_DIGEST --staging GENERATED_JSON [--reference-provider-program EXEC] [--reference-events JSON --reference-events-sha256 TRUSTED_DIGEST --reference-tolerance-us I64]\nRead-only: authenticates source/structural provenance. Optional reference provider or SHA-bound event timestamps check candidate crossings; neither mode certifies astronomical accuracy or coverage.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    sha256: String,
    format: String,
    blocks_verified: usize,
    records_verified: u64,
}

#[derive(Deserialize, PartialEq, Eq)]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CadenceCheck {
    mode: String,
    step_us: i64,
    matched_candidates: usize,
    coverage_certified: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveRiskCheck {
    mode: String,
    max_depth: u8,
    midpoint_probes: usize,
    refined_segments: usize,
    leaf_segments: usize,
    matched_candidates: usize,
    coverage_certified: bool,
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
    cadence_check: Option<CadenceCheck>,
    adaptive_station_check: Option<AdaptiveRiskCheck>,
    adaptive_ingress_check: Option<AdaptiveRiskCheck>,
    adaptive_lunar_check: Option<AdaptiveRiskCheck>,
    provider: Option<ProviderIdentity>,
    sparse_spans: Vec<StagedSpan>,
    candidates: Vec<StagedCandidate>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct GoldenSpan {
    start_us: i64,
    end_us: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenEvent {
    gap_index: usize,
    at_us: i64,
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
struct GoldenReference {
    schema: String,
    source: String,
    version: String,
    family: String,
    body: String,
    requested_window: Window,
    sparse_spans: Vec<GoldenSpan>,
    events: Vec<GoldenEvent>,
}

struct ReviewOptions {
    archive: PathBuf,
    digest: String,
    staging: PathBuf,
    reference_provider: Option<PathBuf>,
    reference_events: Option<PathBuf>,
    reference_events_sha256: Option<String>,
    reference_tolerance_us: Option<i64>,
}

fn options(args: impl IntoIterator<Item = String>) -> Result<ReviewOptions, String> {
    let mut archive = None;
    let mut sha256 = None;
    let mut staging = None;
    let mut reference_provider = None;
    let mut reference_events = None;
    let mut reference_events_sha256 = None;
    let mut reference_tolerance_us = None;
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        let value = args.next().ok_or(format!("missing value for {key}"))?;
        let slot = match key.as_str() {
            "--archive" => &mut archive,
            "--sha256" => &mut sha256,
            "--staging" => &mut staging,
            "--reference-provider-program" => &mut reference_provider,
            "--reference-events" => &mut reference_events,
            "--reference-events-sha256" => &mut reference_events_sha256,
            "--reference-tolerance-us" => &mut reference_tolerance_us,
            _ => return Err(format!("unrecognized option {key}")),
        };
        if slot.replace(value).is_some() {
            return Err(format!("repeated option {key}"));
        }
    }
    if reference_events.is_some() != reference_events_sha256.is_some()
        || (reference_tolerance_us.is_some() && reference_events.is_none())
    {
        return Err("reference events and their trusted SHA-256 must be supplied together".into());
    }
    let reference_tolerance_us = reference_tolerance_us
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "invalid reference tolerance".to_string())
        })
        .transpose()?;
    if reference_tolerance_us.is_some_and(|value| value < 0) {
        return Err("reference tolerance must be nonnegative".into());
    }
    Ok(ReviewOptions {
        archive: PathBuf::from(archive.ok_or("missing --archive")?),
        digest: sha256.ok_or("missing --sha256")?,
        staging: PathBuf::from(staging.ok_or("missing --staging")?),
        reference_provider: reference_provider.map(PathBuf::from),
        reference_events: reference_events.map(PathBuf::from),
        reference_events_sha256,
        reference_tolerance_us,
    })
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
                || stage.cadence_check.is_some()
                || stage.adaptive_station_check.is_some()
                || stage.adaptive_ingress_check.is_some()
                || stage.adaptive_lunar_check.is_some()
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
            if let Some(cadence) = stage.cadence_check.as_ref() {
                if cadence.mode != "half_step"
                    || cadence.coverage_certified
                    || settings.step_us / 2 < settings.precision_us
                    || cadence.step_us != settings.step_us / 2
                    || cadence.matched_candidates != stage.candidates.len()
                    || stage.sparse_spans.is_empty()
                {
                    return Err("invalid self-reported half-step cadence metadata".into());
                }
            }
            if let Some(adaptive) = stage.adaptive_station_check.as_ref() {
                if family != EventFamily::Motion
                    || adaptive.mode != "risk_midpoint"
                    || adaptive.coverage_certified
                    || !(1..=8).contains(&adaptive.max_depth)
                    || (settings.step_us as i128) / (1_i128 << adaptive.max_depth)
                        < settings.precision_us as i128
                    || adaptive.midpoint_probes == 0
                    || adaptive.leaf_segments <= adaptive.refined_segments
                    || adaptive.midpoint_probes > settings.max_steps as usize
                    || adaptive.refined_segments > adaptive.midpoint_probes
                    || adaptive.matched_candidates != stage.candidates.len()
                    || stage.sparse_spans.is_empty()
                {
                    return Err("invalid self-reported adaptive station metadata".into());
                }
            }
            if let Some(adaptive) = stage.adaptive_ingress_check.as_ref() {
                if family != EventFamily::Ingress
                    || adaptive.mode != "zodiac_boundary_midpoint"
                    || adaptive.coverage_certified
                    || !(1..=8).contains(&adaptive.max_depth)
                    || (settings.step_us as i128) / (1_i128 << adaptive.max_depth)
                        < settings.precision_us as i128
                    || adaptive.midpoint_probes == 0
                    || adaptive.leaf_segments <= adaptive.refined_segments
                    || adaptive.midpoint_probes > settings.max_steps as usize
                    || adaptive.refined_segments > adaptive.midpoint_probes
                    || adaptive.matched_candidates != stage.candidates.len()
                    || stage.sparse_spans.is_empty()
                {
                    return Err("invalid self-reported adaptive ingress metadata".into());
                }
            }
            if let Some(adaptive) = stage.adaptive_lunar_check.as_ref() {
                if family != EventFamily::Synod
                    || stage.body != "Moon"
                    || adaptive.mode != "phase_angle_midpoint"
                    || adaptive.coverage_certified
                    || !(1..=8).contains(&adaptive.max_depth)
                    || (settings.step_us as i128) / (1_i128 << adaptive.max_depth)
                        < settings.precision_us as i128
                    || adaptive.midpoint_probes == 0
                    || adaptive.leaf_segments <= adaptive.refined_segments
                    || adaptive.midpoint_probes > settings.max_steps as usize
                    || adaptive.refined_segments > adaptive.midpoint_probes
                    || adaptive.matched_candidates != stage.candidates.len()
                    || stage.sparse_spans.is_empty()
                {
                    return Err("invalid self-reported adaptive lunar metadata".into());
                }
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

fn checked_sample<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    utc_us: i64,
) -> Result<LongitudeSample, String> {
    let sample = provider.sample(body, TimestampMicros::new(utc_us))?;
    if !sample.longitude_degrees.is_finite() || !sample.longitude_speed_degrees_per_day.is_finite()
    {
        return Err("nonfinite reference longitude or speed".into());
    }
    Ok(sample)
}

fn normalized_sign(longitude: f64) -> u8 {
    (longitude.rem_euclid(360.0) / 30.0).floor() as u8
}

/// A separately supplied provider reports the declared crossing inside this
/// bracket. This does NOT prove that no other crossings were missed.
fn check_numerical_bracket<P: LongitudeProvider>(
    provider: &P,
    candidate: &StagedCandidate,
    body: &BodyId,
    family: EventFamily,
) -> Result<(), String> {
    let left = checked_sample(provider, body, candidate.bracket_start_us)?;
    let right = checked_sample(provider, body, candidate.bracket_end_us)?;
    let consistent = match family {
        EventFamily::Motion => match candidate.direction_after.as_deref() {
            Some("direct") => {
                left.longitude_speed_degrees_per_day < 0.0
                    && right.longitude_speed_degrees_per_day >= 0.0
            }
            Some("retrograde") => {
                left.longitude_speed_degrees_per_day >= 0.0
                    && right.longitude_speed_degrees_per_day < 0.0
            }
            _ => false,
        },
        EventFamily::Ingress => {
            let from = normalized_sign(left.longitude_degrees);
            let to = normalized_sign(right.longitude_degrees);
            let mut delta = (right.longitude_degrees - left.longitude_degrees).rem_euclid(360.0);
            if delta > 180.0 {
                delta -= 360.0;
            }
            let retrograde = delta < 0.0;
            delta.abs() < 30.0
                && delta != 0.0
                && Some(to) == candidate.sign_after_index
                && Some(retrograde) == candidate.retrograde
                && if retrograde {
                    to == (from + 11) % 12
                } else {
                    to == (from + 1) % 12
                }
        }
        EventFamily::Synod => {
            let sun = BodyId::try_from("Sun").expect("built-in Sun body");
            let left_sun = checked_sample(provider, &sun, candidate.bracket_start_us)?;
            let right_sun = checked_sample(provider, &sun, candidate.bracket_end_us)?;
            let advancing = left.longitude_speed_degrees_per_day
                > left_sun.longitude_speed_degrees_per_day
                && right.longitude_speed_degrees_per_day
                    > right_sun.longitude_speed_degrees_per_day;
            let before = (left.longitude_degrees - left_sun.longitude_degrees).rem_euclid(360.0);
            let after = (right.longitude_degrees - right_sun.longitude_degrees).rem_euclid(360.0);
            let progress = (after - before).rem_euclid(360.0);
            let target: f64 = match candidate.phase.as_deref() {
                Some("new_moon") => 0.0,
                Some("first_quarter") => 90.0,
                Some("full_moon") => 180.0,
                Some("last_quarter") => 270.0,
                _ => return Err("unsupported lunar phase".into()),
            };
            let offset = (target - before).rem_euclid(360.0);
            advancing && progress > 0.0 && progress < 30.0 && offset > 0.0 && offset <= progress
        }
    };
    if consistent {
        Ok(())
    } else {
        Err("candidate does not contain the declared crossing under the reference provider".into())
    }
}

fn check_reference_provider(
    stage: &Staging,
    program: &std::path::Path,
) -> Result<ProviderIdentity, String> {
    if stage.mode != "calculate" || stage.candidates.is_empty() {
        return Err("reference checks require a calculated staging file with candidates".into());
    }
    if stage.candidates.len() > MAX_REFERENCE_CANDIDATES {
        return Err(format!(
            "reference checks limited to {MAX_REFERENCE_CANDIDATES} candidates; split the generation window"
        ));
    }
    let provider = ProcessLongitudeProvider::spawn(program)?;
    let identity = provider.identity().clone();
    if stage.provider.as_ref() == Some(&identity) {
        return Err(
            "reference provider declares the SAME identity/settings as the generator".into(),
        );
    }
    let family = match stage.family.as_str() {
        "synods" => EventFamily::Synod,
        "motions" => EventFamily::Motion,
        "ingresses" => EventFamily::Ingress,
        _ => return Err("unsupported reference family".into()),
    };
    let body = BodyId::try_from(stage.body.as_str()).map_err(|_| "invalid body".to_string())?;
    for (index, candidate) in stage.candidates.iter().enumerate() {
        check_numerical_bracket(&provider, candidate, &body, family).map_err(|error| {
            format!("reference numerical check failed for candidate #{index}: {error}")
        })?;
    }
    Ok(identity)
}

/// Compare a complete per-span reference event list against all staged brackets.
/// The supplied SHA authenticates the reference file bytes but cannot establish
/// the scientific accuracy or completeness of its author.
fn check_reference_events(
    stage: &Staging,
    path: &std::path::Path,
    trusted_digest: &str,
    tolerance_us: i64,
) -> Result<usize, String> {
    if stage.mode != "calculate" {
        return Err("reference events require calculated staging".into());
    }
    let precision = stage
        .calculation_settings
        .as_ref()
        .ok_or("missing calculation precision")?
        .precision_us;
    if tolerance_us < 0 || tolerance_us > precision {
        return Err(
            "reference timestamp tolerance must be between zero and bracket precision".into(),
        );
    }
    if trusted_digest.len() != 64
        || !trusted_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("reference events SHA-256 must contain 64 lowercase hex digits".into());
    }
    let meta =
        fs::metadata(path).map_err(|error| format!("cannot stat reference events: {error}"))?;
    if !meta.is_file() || meta.len() > MAX_REFERENCE_BYTES {
        return Err("reference events must be a regular file <= 16 MiB".into());
    }
    let raw = fs::read(path).map_err(|error| format!("cannot read reference events: {error}"))?;
    if raw.len() as u64 > MAX_REFERENCE_BYTES {
        return Err("reference events exceeded byte limit during read".into());
    }
    let actual_digest = format!("{:x}", Sha256::digest(&raw));
    if actual_digest != trusted_digest {
        return Err("reference events do not match independently trusted SHA-256".into());
    }
    let golden: GoldenReference = serde_json::from_slice(&raw)
        .map_err(|error| format!("invalid reference events schema: {error}"))?;
    if golden.schema != "vseph-reference-events-v1"
        || golden.source.trim().is_empty()
        || golden.version.trim().is_empty()
        || golden.family != stage.family
        || golden.body != stage.body
        || golden.requested_window != stage.requested_window
    {
        return Err("reference event identity, provenance or requested window differs".into());
    }
    if golden.sparse_spans.len() != stage.sparse_spans.len()
        || !golden
            .sparse_spans
            .iter()
            .zip(&stage.sparse_spans)
            .all(|(golden, staged)| {
                golden.start_us == staged.start_us && golden.end_us == staged.end_us
            })
    {
        return Err(
            "reference events do not declare all independently derived sparse spans".into(),
        );
    }
    if golden.events.len() > MAX_REFERENCE_CANDIDATES {
        return Err("reference events exceed comparison hard limit".into());
    }
    if golden.events.len() != stage.candidates.len() {
        return Err("reference/staging event count mismatch (missing or extra event)".into());
    }
    let mut previous = None;
    for (index, (reference, staged)) in golden.events.iter().zip(&stage.candidates).enumerate() {
        let span = stage
            .sparse_spans
            .get(reference.gap_index)
            .ok_or(format!("reference event #{index} has invalid gap index"))?;
        let key = (reference.gap_index, reference.at_us);
        if previous.is_some_and(|last| key <= last)
            || reference.at_us < span.start_us
            || reference.at_us > span.end_us
        {
            return Err(format!(
                "reference event #{index} is unordered, duplicated or outside its sparse span"
            ));
        }
        previous = Some(key);
        if reference.gap_index != staged.gap_index
            || reference.phase != staged.phase
            || reference.direction_after != staged.direction_after
            || reference.sign_after_index != staged.sign_after_index
            || reference.retrograde != staged.retrograde
        {
            return Err(format!(
                "reference event #{index} identity differs from staged candidate"
            ));
        }
        let at = reference.at_us as i128;
        let tolerance = tolerance_us as i128;
        if at < staged.bracket_start_us as i128 - tolerance
            || at > staged.bracket_end_us as i128 + tolerance
        {
            return Err(format!(
                "reference event #{index} UTC timestamp is outside bracket tolerance"
            ));
        }
    }
    Ok(golden.events.len())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let opts = options(args)?;
    let archive = VirgoStarsEventEphemeris::open_verified(&opts.archive, &opts.digest)
        .map_err(|error| error.to_string())?;
    let meta =
        fs::metadata(&opts.staging).map_err(|error| format!("cannot stat staging: {error}"))?;
    if !meta.is_file() || meta.len() > MAX_STAGING_BYTES {
        return Err("staging must be a regular file <= 32 MiB".into());
    }
    let raw = fs::read(&opts.staging).map_err(|error| format!("cannot read staging: {error}"))?;
    if raw.len() as u64 > MAX_STAGING_BYTES {
        return Err("staging changed or exceeded byte limit during read".into());
    }
    let staging: Staging = serde_json::from_slice(&raw)
        .map_err(|error| format!("invalid staging JSON schema: {error}"))?;
    let (count, touching) = check_staging(&staging, &archive)?;
    // Structural validation always precedes external process launch.
    let golden_checked = opts
        .reference_events
        .as_deref()
        .map(|path| {
            check_reference_events(
                &staging,
                path,
                opts.reference_events_sha256
                    .as_deref()
                    .expect("validated pair"),
                opts.reference_tolerance_us.unwrap_or(0),
            )
        })
        .transpose()?;
    let reference = opts
        .reference_provider
        .as_deref()
        .map(|program| check_reference_provider(&staging, program))
        .transpose()?;
    let crosschecked = reference.is_some();
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
            "reference_provider": reference,
            "numerical_crossings_checked": if crosschecked { count } else { 0 },
            "reference_crossings_consistent": crosschecked,
            "reference_independence_scientifically_established": false,
            "reference_event_digest_authenticated": golden_checked.is_some(),
            "reference_event_timestamps_matched": golden_checked.unwrap_or(0),
            "reference_events_correspondence_consistent": golden_checked.is_some(),
            "reference_events_scientifically_independent": false,
            "producer_claimed_half_step_check": staging.cadence_check.is_some(),
            "cadence_check_independently_recomputed": false,
            "producer_claimed_adaptive_station_check": staging.adaptive_station_check.is_some(),
            "adaptive_station_check_independently_recomputed": false,
            "producer_claimed_adaptive_ingress_check": staging.adaptive_ingress_check.is_some(),
            "adaptive_ingress_check_independently_recomputed": false,
            "producer_claimed_adaptive_lunar_check": staging.adaptive_lunar_check.is_some(),
            "adaptive_lunar_check_independently_recomputed": false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_events_options_require_an_external_digest_and_valid_tolerance() {
        let original = [
            "--archive",
            "approved.p0",
            "--sha256",
            "0",
            "--staging",
            "staged.json",
        ];
        let mut incomplete: Vec<_> = original.iter().map(|s| s.to_string()).collect();
        incomplete.extend(["--reference-events".into(), "golden.json".into()]);
        assert!(options(incomplete).is_err());

        let mut complete: Vec<_> = original.iter().map(|s| s.to_string()).collect();
        complete.extend([
            "--reference-events".into(),
            "golden.json".into(),
            "--reference-events-sha256".into(),
            "0".repeat(64),
            "--reference-tolerance-us".into(),
            "100".into(),
        ]);
        assert_eq!(
            options(complete.clone()).unwrap().reference_tolerance_us,
            Some(100)
        );
        complete.extend(["--reference-tolerance-us".into(), "200".into()]);
        assert!(options(complete).is_err());
    }

    struct LinearReference {
        retrograde: bool,
    }

    impl LongitudeProvider for LinearReference {
        fn sample(&self, _body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let time = at.as_i64() as f64;
            Ok(LongitudeSample {
                longitude_degrees: if self.retrograde {
                    0.2 - time * 0.2
                } else {
                    29.8 + time * 0.2
                },
                longitude_speed_degrees_per_day: if self.retrograde {
                    1.0 - time
                } else {
                    time - 1.0
                },
            })
        }
    }

    fn candidate() -> StagedCandidate {
        StagedCandidate {
            gap_index: 0,
            family: "motions".into(),
            body: "Mars".into(),
            bracket_start_us: 0,
            bracket_end_us: 2,
            direction_after: Some("direct".into()),
            sign_after_index: None,
            retrograde: None,
            phase: None,
        }
    }

    #[test]
    fn rejects_wrong_station_direction_and_recognizes_retrograde_station() {
        let body = BodyId::try_from("Mars").unwrap();
        let mut bracket = candidate();
        assert!(check_numerical_bracket(
            &LinearReference { retrograde: false },
            &bracket,
            &body,
            EventFamily::Motion
        )
        .is_ok());
        assert!(check_numerical_bracket(
            &LinearReference { retrograde: true },
            &bracket,
            &body,
            EventFamily::Motion
        )
        .is_err());
        bracket.direction_after = Some("retrograde".into());
        assert!(check_numerical_bracket(
            &LinearReference { retrograde: true },
            &bracket,
            &body,
            EventFamily::Motion
        )
        .is_ok());
    }

    #[test]
    fn checks_backward_pisces_reentry_across_zero_longitude() {
        let body = BodyId::try_from("Mars").unwrap();
        let mut bracket = candidate();
        bracket.family = "ingresses".into();
        bracket.direction_after = None;
        bracket.sign_after_index = Some(11);
        bracket.retrograde = Some(true);
        assert!(check_numerical_bracket(
            &LinearReference { retrograde: true },
            &bracket,
            &body,
            EventFamily::Ingress
        )
        .is_ok());
        bracket.retrograde = Some(false);
        assert!(check_numerical_bracket(
            &LinearReference { retrograde: true },
            &bracket,
            &body,
            EventFamily::Ingress
        )
        .is_err());
    }
}
