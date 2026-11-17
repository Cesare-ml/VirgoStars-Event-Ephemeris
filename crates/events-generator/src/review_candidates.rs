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
use virgostars_event_ephemeris::{BodyId, EventFamily, TimestampMicros, VirgoStarsEventEphemeris};
use virgostars_events_generator::gaps::detect_sparse_spans;
use virgostars_events_generator::provider_process::{ProcessLongitudeProvider, ProviderIdentity};
use virgostars_events_generator::{LongitudeProvider, LongitudeSample};

const MAX_STAGING_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CANDIDATES: usize = 100_000;
const MAX_REFERENCE_CANDIDATES: usize = 10_000;
const USAGE: &str = "Usage: vseph-review-candidates --archive P0 --sha256 INDEPENDENT_TRUSTED_DIGEST --staging GENERATED_JSON [--reference-provider-program EXEC]\nRead-only: authenticates source/structural provenance. Optional separately identified provider checks bracket crossings; neither mode verifies astronomical accuracy or certifies coverage.";

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CadenceCheck {
    mode: String,
    step_us: i64,
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
    provider: Option<ProviderIdentity>,
    sparse_spans: Vec<StagedSpan>,
    candidates: Vec<StagedCandidate>,
}

fn options(
    args: impl IntoIterator<Item = String>,
) -> Result<(PathBuf, String, PathBuf, Option<PathBuf>), String> {
    let mut archive = None;
    let mut sha256 = None;
    let mut staging = None;
    let mut reference_provider = None;
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        let value = args.next().ok_or(format!("missing value for {key}"))?;
        let slot = match key.as_str() {
            "--archive" => &mut archive,
            "--sha256" => &mut sha256,
            "--staging" => &mut staging,
            "--reference-provider-program" => &mut reference_provider,
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
        reference_provider.map(PathBuf::from),
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
                || stage.cadence_check.is_some()
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

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let (archive_path, digest, staging_path, reference_program) = options(args)?;
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
    // Structural validation always precedes external process launch.
    let reference = reference_program
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
            "producer_claimed_half_step_check": staging.cadence_check.is_some(),
            "cadence_check_independently_recomputed": false,
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
