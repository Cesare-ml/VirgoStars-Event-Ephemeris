/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Candidate-only comparison of two independently supplied latitude providers.
//!
//! Two providers agreeing does not mean they are scientifically independent.
//! Even identical *empty* lists do not certify the absence of hidden events.
//! The caller must separately audit ecliptic frame, corrections, kernel,
//! time scale, licensing and independent astronomical golden timestamps.

use virgostars_events_core::{BodyId, TimestampMicros};

use crate::{
    latitude_crossings::{
        adaptive_ecliptic_plane_crossings_scan, adaptive_latitude_stations_scan,
        EclipticPlaneDirection, LatitudeMotionDirection,
    },
    latitude_extrema::{adaptive_latitude_speed_extrema_scan, LatitudeSpeedExtremumKind},
    next_time, GenerationError, LatitudeProvider, LatitudeSample, SearchWindow,
};

/// Matches only the same astronomical event family and observed direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum LatitudeCandidateKind {
    PlaneAscending,
    PlaneDescending,
    StationNorthward,
    StationSouthward,
    NorthwardSpeedMaximum,
    SouthwardSpeedMagnitudeMaximum,
}

impl LatitudeCandidateKind {
    fn is_speed_peak(self) -> bool {
        matches!(
            self,
            Self::NorthwardSpeedMaximum | Self::SouthwardSpeedMagnitudeMaximum
        )
    }
}

/// Generator-only event candidate; never an authoritative archived record.
#[derive(Clone, Debug, PartialEq)]
pub struct LatitudeObservedCandidate {
    pub body: BodyId,
    pub kind: LatitudeCandidateKind,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    /// Present only at a candidate signed latitude-speed maximum/minimum.
    pub signed_speed_degrees_per_day: Option<f64>,
}

/// Caller-approved numeric comparison tolerances, not astronomical accuracy.
#[derive(Clone, Copy, Debug)]
pub struct LatitudeComparisonTolerance {
    pub time_us: i64,
    pub signed_speed_degrees_per_day: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct LatitudeComparisonOptions {
    pub window: SearchWindow,
    pub max_depth: u8,
    pub tolerance: LatitudeComparisonTolerance,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatitudeComparisonProbeCounts {
    pub plane_crossings: usize,
    pub speed_stations: usize,
    pub speed_peaks: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LatitudeCandidatePair {
    pub primary: LatitudeObservedCandidate,
    pub reference: LatitudeObservedCandidate,
    /// Reference-minus-primary UTC bracket midpoint offset.
    pub reference_offset_us: i128,
    /// Available only for peak pairs; signed reference-minus-primary speed.
    pub signed_speed_offset_degrees_per_day: Option<f64>,
    /// `None` for plane crossings and stations (not speed-peak comparisons).
    pub speed_within_tolerance: Option<bool>,
}

#[derive(Debug, Default)]
pub struct LatitudeProviderComparisonReport {
    pub pairs: Vec<LatitudeCandidatePair>,
    pub unmatched_primary: Vec<LatitudeObservedCandidate>,
    pub unmatched_reference: Vec<LatitudeObservedCandidate>,
    pub primary_interior_probes: LatitudeComparisonProbeCounts,
    pub reference_interior_probes: LatitudeComparisonProbeCounts,
}

impl LatitudeProviderComparisonReport {
    /// Only observed candidate disagreements, NEVER a completeness verdict.
    pub fn has_observed_disagreements(&self) -> bool {
        !self.unmatched_primary.is_empty()
            || !self.unmatched_reference.is_empty()
            || self
                .pairs
                .iter()
                .any(|pair| pair.speed_within_tolerance == Some(false))
    }
}

fn midpoint_us(candidate: &LatitudeObservedCandidate) -> i128 {
    candidate.bracket_start_us as i128
        + (candidate.bracket_end_us as i128 - candidate.bracket_start_us as i128) / 2
}

fn reject_ambiguous_matches(
    candidates: &[&LatitudeObservedCandidate],
    time_tolerance_us: i64,
    kind: LatitudeCandidateKind,
) -> Result<(), GenerationError> {
    for pair in candidates.windows(2) {
        if midpoint_us(pair[1]) - midpoint_us(pair[0]) <= 2 * time_tolerance_us as i128 {
            return Err(if kind.is_speed_peak() {
                GenerationError::AmbiguousLatitudeSpeedExtremum
            } else {
                GenerationError::AmbiguousLatitudeCrossing
            });
        }
    }
    Ok(())
}

struct ObservedScan {
    candidates: Vec<LatitudeObservedCandidate>,
    probes: LatitudeComparisonProbeCounts,
}

fn scan_all_latitude_kinds<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    options: LatitudeComparisonOptions,
    remaining_probes: &mut usize,
) -> Result<ObservedScan, GenerationError> {
    let plane = adaptive_ecliptic_plane_crossings_scan(
        provider,
        body,
        options.window,
        options.max_depth,
        remaining_probes,
    )?;
    let stations = adaptive_latitude_stations_scan(
        provider,
        body,
        options.window,
        options.max_depth,
        remaining_probes,
    )?;
    let peaks = adaptive_latitude_speed_extrema_scan(
        provider,
        body,
        options.window,
        options.max_depth,
        remaining_probes,
    )?;

    let probes = LatitudeComparisonProbeCounts {
        plane_crossings: plane.interior_probes,
        speed_stations: stations.interior_probes,
        speed_peaks: peaks.interior_probes,
    };
    let mut candidates = Vec::with_capacity(
        plane.candidates.len() + stations.candidates.len() + peaks.candidates.len(),
    );
    candidates.extend(
        plane
            .candidates
            .into_iter()
            .map(|candidate| LatitudeObservedCandidate {
                body: candidate.body,
                kind: match candidate.direction {
                    EclipticPlaneDirection::Ascending => LatitudeCandidateKind::PlaneAscending,
                    EclipticPlaneDirection::Descending => LatitudeCandidateKind::PlaneDescending,
                },
                bracket_start_us: candidate.bracket_start_us,
                bracket_end_us: candidate.bracket_end_us,
                signed_speed_degrees_per_day: None,
            }),
    );
    candidates.extend(
        stations
            .candidates
            .into_iter()
            .map(|candidate| LatitudeObservedCandidate {
                body: candidate.body,
                kind: match candidate.direction_after {
                    LatitudeMotionDirection::Northward => LatitudeCandidateKind::StationNorthward,
                    LatitudeMotionDirection::Southward => LatitudeCandidateKind::StationSouthward,
                },
                bracket_start_us: candidate.bracket_start_us,
                bracket_end_us: candidate.bracket_end_us,
                signed_speed_degrees_per_day: None,
            }),
    );
    candidates.extend(
        peaks
            .candidates
            .into_iter()
            .map(|candidate| LatitudeObservedCandidate {
                body: candidate.body,
                kind: match candidate.kind {
                    LatitudeSpeedExtremumKind::NorthwardMaximum => {
                        LatitudeCandidateKind::NorthwardSpeedMaximum
                    }
                    LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum => {
                        LatitudeCandidateKind::SouthwardSpeedMagnitudeMaximum
                    }
                },
                bracket_start_us: candidate.bracket_start_us,
                bracket_end_us: candidate.bracket_end_us,
                signed_speed_degrees_per_day: Some(candidate.sampled_signed_speed_degrees_per_day),
            }),
    );
    candidates.sort_by_key(|candidate| {
        (
            candidate.kind,
            candidate.bracket_start_us,
            candidate.bracket_end_us,
        )
    });
    Ok(ObservedScan { candidates, probes })
}

/// Compare all six observed directional kinds from three latitudinal scanners.
///
/// The two providers receive **separate** budgets, shared across their three
/// event searches. Matching is one-to-one, same kind, chronological, within
/// `tolerance.time_us`; if same-kind events are too dense for unambiguous
/// pairing, fail closed. Reference-minus-primary differences in signed peak
/// speed are retained even if timestamps agree. Missing/extra candidates are
/// explicit. These results are not a source-independent astronomical oracle
/// and they never certify UTC coverage, including when both lists are empty.
pub fn compare_latitude_event_providers<A: LatitudeProvider, B: LatitudeProvider>(
    primary: &A,
    reference: &B,
    body: &BodyId,
    options: LatitudeComparisonOptions,
    primary_remaining_probes: &mut usize,
    reference_remaining_probes: &mut usize,
) -> Result<LatitudeProviderComparisonReport, GenerationError> {
    if options.tolerance.time_us < 0
        || !options.tolerance.signed_speed_degrees_per_day.is_finite()
        || options.tolerance.signed_speed_degrees_per_day < 0.0
    {
        return Err(GenerationError::InvalidWindow);
    }
    // Validate options before spending *either* provider's probe budget.
    options.window.validate()?;
    if !(1..=8).contains(&options.max_depth)
        || (options.window.step_us as i128) / (1_i128 << (options.max_depth as u32 + 2))
            < options.window.precision_us as i128
    {
        return Err(GenerationError::InvalidWindow);
    }

    let a = scan_all_latitude_kinds(primary, body, options, primary_remaining_probes)?;
    let b = scan_all_latitude_kinds(reference, body, options, reference_remaining_probes)?;
    let mut result = LatitudeProviderComparisonReport {
        primary_interior_probes: a.probes,
        reference_interior_probes: b.probes,
        ..LatitudeProviderComparisonReport::default()
    };

    for kind in [
        LatitudeCandidateKind::PlaneAscending,
        LatitudeCandidateKind::PlaneDescending,
        LatitudeCandidateKind::StationNorthward,
        LatitudeCandidateKind::StationSouthward,
        LatitudeCandidateKind::NorthwardSpeedMaximum,
        LatitudeCandidateKind::SouthwardSpeedMagnitudeMaximum,
    ] {
        let primary_candidates: Vec<_> = a.candidates.iter().filter(|c| c.kind == kind).collect();
        let reference_candidates: Vec<_> = b.candidates.iter().filter(|c| c.kind == kind).collect();
        reject_ambiguous_matches(&primary_candidates, options.tolerance.time_us, kind)?;
        reject_ambiguous_matches(&reference_candidates, options.tolerance.time_us, kind)?;
        let (mut i, mut j) = (0, 0);
        while i < primary_candidates.len() && j < reference_candidates.len() {
            let first = primary_candidates[i];
            let second = reference_candidates[j];
            let offset = midpoint_us(second) - midpoint_us(first);
            if offset.abs() <= options.tolerance.time_us as i128 {
                let signed_speed_offset_degrees_per_day = match (
                    first.signed_speed_degrees_per_day,
                    second.signed_speed_degrees_per_day,
                ) {
                    (Some(x), Some(y)) => {
                        let delta = y - x;
                        if !delta.is_finite() {
                            return Err(GenerationError::InvalidSample);
                        }
                        Some(delta)
                    }
                    (None, None) if !kind.is_speed_peak() => None,
                    _ => return Err(GenerationError::InvalidSample),
                };
                result.pairs.push(LatitudeCandidatePair {
                    primary: first.clone(),
                    reference: second.clone(),
                    reference_offset_us: offset,
                    speed_within_tolerance: signed_speed_offset_degrees_per_day
                        .map(|delta| delta.abs() <= options.tolerance.signed_speed_degrees_per_day),
                    signed_speed_offset_degrees_per_day,
                });
                i += 1;
                j += 1;
            } else if offset > 0 {
                result.unmatched_primary.push(first.clone());
                i += 1;
            } else {
                result.unmatched_reference.push(second.clone());
                j += 1;
            }
        }
        result.unmatched_primary.extend(
            primary_candidates[i..]
                .iter()
                .map(|candidate| (*candidate).clone()),
        );
        result.unmatched_reference.extend(
            reference_candidates[j..]
                .iter()
                .map(|candidate| (*candidate).clone()),
        );
    }
    result.pairs.sort_by_key(|pair| {
        (
            pair.primary.bracket_start_us,
            pair.primary.kind,
            pair.primary.bracket_end_us,
        )
    });
    result
        .unmatched_primary
        .sort_by_key(|candidate| (candidate.bracket_start_us, candidate.kind));
    result
        .unmatched_reference
        .sort_by_key(|candidate| (candidate.bracket_start_us, candidate.kind));
    Ok(result)
}

/// Numeric tolerances for same-instant latitude and signed latitude speed.
/// These are explicitly chosen by a scientific reviewer, never inferred.
#[derive(Clone, Copy, Debug)]
pub struct LatitudeSampleTolerance {
    pub latitude_degrees: f64,
    pub signed_speed_degrees_per_day: f64,
}

/// One identical UTC sampling schedule is used for both latitude providers.
#[derive(Clone, Copy, Debug)]
pub struct LatitudeSampleComparisonOptions {
    pub window: SearchWindow,
    pub tolerance: LatitudeSampleTolerance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatitudeSampleDisagreement {
    pub at_us: i64,
    /// Signed reference minus primary beta, degrees.
    pub latitude_offset_degrees: f64,
    /// Signed reference minus primary d(beta)/dt, degrees/day.
    pub signed_speed_offset_degrees_per_day: f64,
    pub latitude_within_tolerance: bool,
    pub speed_within_tolerance: bool,
}

/// Sample residuals can reveal differences even if both event scans are empty.
/// This report still never determines astronomical accuracy or UTC coverage.
#[derive(Debug, Default)]
pub struct LatitudeSampleComparisonReport {
    pub samples_checked: usize,
    /// Stored only for at least one out-of-tolerance coordinate/speed.
    pub disagreements: Vec<LatitudeSampleDisagreement>,
    pub maximum_absolute_latitude_offset_degrees: f64,
    pub maximum_absolute_speed_offset_degrees_per_day: f64,
    pub maximum_latitude_offset_at_us: Option<i64>,
    pub maximum_speed_offset_at_us: Option<i64>,
}

impl LatitudeSampleComparisonReport {
    /// Only a verdict for the **sampled** instants and supplied tolerances.
    pub fn has_sample_disagreements(&self) -> bool {
        !self.disagreements.is_empty()
    }
}

fn checked_latitude_sample<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    at_us: i64,
    remaining_probes: &mut usize,
) -> Result<LatitudeSample, GenerationError> {
    if *remaining_probes == 0 {
        return Err(GenerationError::TooManySteps);
    }
    *remaining_probes -= 1;
    let sample = provider
        .sample_latitude(body, TimestampMicros::new(at_us))
        .map_err(GenerationError::Provider)?;
    if !sample.latitude_degrees.is_finite()
        || sample.latitude_degrees.abs() > 90.0
        || !sample.latitude_speed_degrees_per_day.is_finite()
    {
        return Err(GenerationError::InvalidSample);
    }
    Ok(sample)
}

/// Check actual coordinate/speed residuals on an identical, bounded UTC grid.
///
/// Probes occur at `[start, end)` in increments of `step_us` (never sampling
/// the exclusive end). Unlike candidate matching, this detects systematic
/// beta/speed offsets even with two empty candidate lists. Separate budgets
/// apply to both providers, and no partial report is returned on error.
/// An empty disagreement list can never certify unsampled instants, frame
/// equivalence, provider independence or scientific coverage.
pub fn compare_latitude_provider_samples<A: LatitudeProvider, B: LatitudeProvider>(
    primary: &A,
    reference: &B,
    body: &BodyId,
    options: LatitudeSampleComparisonOptions,
    primary_remaining_probes: &mut usize,
    reference_remaining_probes: &mut usize,
) -> Result<LatitudeSampleComparisonReport, GenerationError> {
    options.window.validate()?;
    if !options.tolerance.latitude_degrees.is_finite()
        || options.tolerance.latitude_degrees < 0.0
        || !options.tolerance.signed_speed_degrees_per_day.is_finite()
        || options.tolerance.signed_speed_degrees_per_day < 0.0
    {
        return Err(GenerationError::InvalidWindow);
    }
    let mut report = LatitudeSampleComparisonReport::default();
    let mut at_us = options.window.start_us;
    while at_us < options.window.end_us {
        // Budget feasibility is shared for the pair before either call.
        if *primary_remaining_probes == 0 || *reference_remaining_probes == 0 {
            return Err(GenerationError::TooManySteps);
        }
        let a = checked_latitude_sample(primary, body, at_us, primary_remaining_probes)?;
        let b = checked_latitude_sample(reference, body, at_us, reference_remaining_probes)?;
        let lat_offset = b.latitude_degrees - a.latitude_degrees;
        let speed_offset = b.latitude_speed_degrees_per_day - a.latitude_speed_degrees_per_day;
        if !lat_offset.is_finite() || !speed_offset.is_finite() {
            return Err(GenerationError::InvalidSample);
        }
        let abs_lat = lat_offset.abs();
        let abs_speed = speed_offset.abs();
        if report.maximum_latitude_offset_at_us.is_none()
            || abs_lat > report.maximum_absolute_latitude_offset_degrees
        {
            report.maximum_absolute_latitude_offset_degrees = abs_lat;
            report.maximum_latitude_offset_at_us = Some(at_us);
        }
        if report.maximum_speed_offset_at_us.is_none()
            || abs_speed > report.maximum_absolute_speed_offset_degrees_per_day
        {
            report.maximum_absolute_speed_offset_degrees_per_day = abs_speed;
            report.maximum_speed_offset_at_us = Some(at_us);
        }
        let latitude_within_tolerance = abs_lat <= options.tolerance.latitude_degrees;
        let speed_within_tolerance = abs_speed <= options.tolerance.signed_speed_degrees_per_day;
        if !latitude_within_tolerance || !speed_within_tolerance {
            report.disagreements.push(LatitudeSampleDisagreement {
                at_us,
                latitude_offset_degrees: lat_offset,
                signed_speed_offset_degrees_per_day: speed_offset,
                latitude_within_tolerance,
                speed_within_tolerance,
            });
        }
        report.samples_checked += 1;
        at_us = next_time(at_us, options.window);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LatitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct Periodic {
        shift_days: f64,
        amplitude: f64,
        frequency: f64,
    }

    impl LatitudeProvider for Periodic {
        fn sample_latitude(
            &self,
            _: &BodyId,
            at: TimestampMicros,
        ) -> Result<LatitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            let phase =
                2.0 * std::f64::consts::PI * self.frequency * (days + 0.1 - self.shift_days);
            let angular_speed = 2.0 * std::f64::consts::PI * self.frequency;
            Ok(LatitudeSample {
                latitude_degrees: self.amplitude * phase.cos(),
                latitude_speed_degrees_per_day: -self.amplitude * angular_speed * phase.sin(),
            })
        }
    }

    struct Monotone;
    impl LatitudeProvider for Monotone {
        fn sample_latitude(
            &self,
            _: &BodyId,
            at: TimestampMicros,
        ) -> Result<LatitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LatitudeSample {
                latitude_degrees: 0.2 + 0.02 * days + 0.0001 * days * days,
                latitude_speed_degrees_per_day: 0.02 + 0.0002 * days,
            })
        }
    }

    fn provider(shift_days: f64, amplitude: f64) -> Periodic {
        Periodic {
            shift_days,
            amplitude,
            frequency: 1.0,
        }
    }

    fn body() -> BodyId {
        BodyId::try_from("Mars").unwrap()
    }

    fn options() -> LatitudeComparisonOptions {
        LatitudeComparisonOptions {
            window: SearchWindow {
                start_us: 0,
                end_us: 2 * DAY,
                step_us: DAY,
                precision_us: 1_000_000,
            },
            max_depth: 1,
            tolerance: LatitudeComparisonTolerance {
                time_us: 600_000_000,
                signed_speed_degrees_per_day: 0.1,
            },
        }
    }

    fn compare(
        primary: &impl LatitudeProvider,
        reference: &impl LatitudeProvider,
        opts: LatitudeComparisonOptions,
    ) -> LatitudeProviderComparisonReport {
        let (mut a, mut b) = (2_000, 2_000);
        compare_latitude_event_providers(primary, reference, &body(), opts, &mut a, &mut b).unwrap()
    }

    #[test]
    fn twelve_directional_events_match_across_slightly_shifted_providers() {
        let report = compare(&provider(0.0, 1.0), &provider(0.005, 1.0), options());
        assert_eq!(report.pairs.len(), 12);
        assert!(report.unmatched_primary.is_empty());
        assert!(report.unmatched_reference.is_empty());
        assert_eq!(report.primary_interior_probes.plane_crossings, 6);
        assert_eq!(report.reference_interior_probes.speed_stations, 6);
        assert_eq!(report.primary_interior_probes.speed_peaks, 6);
        assert!(!report.has_observed_disagreements());
        for pair in &report.pairs {
            assert!((pair.reference_offset_us - 432_000_000).abs() <= 1_000_000);
            assert_eq!(pair.primary.kind, pair.reference.kind);
        }
        assert_eq!(
            report
                .pairs
                .iter()
                .filter(|p| p.signed_speed_offset_degrees_per_day.is_some())
                .count(),
            4
        );
    }

    #[test]
    fn matching_times_still_reveal_disagreement_of_signed_peak_speeds() {
        let report = compare(&provider(0.0, 1.0), &provider(0.0, 1.2), options());
        assert_eq!(report.pairs.len(), 12);
        assert_eq!(
            report
                .pairs
                .iter()
                .filter(|p| p.speed_within_tolerance == Some(false))
                .count(),
            4
        );
        assert_eq!(
            report
                .pairs
                .iter()
                .filter(|p| p.speed_within_tolerance.is_none())
                .count(),
            8
        );
        assert!(report.has_observed_disagreements());
        assert!(report
            .pairs
            .iter()
            .filter_map(|p| p.signed_speed_offset_degrees_per_day)
            .any(|d| d > 1.0));
        assert!(report
            .pairs
            .iter()
            .filter_map(|p| p.signed_speed_offset_degrees_per_day)
            .any(|d| d < -1.0));
    }

    #[test]
    fn reversed_latitude_directions_are_not_silently_matched() {
        let report = compare(&provider(0.0, 1.0), &provider(0.0, -1.0), options());
        assert!(report.pairs.is_empty());
        assert_eq!(report.unmatched_primary.len(), 12);
        assert_eq!(report.unmatched_reference.len(), 12);
        assert!(report.has_observed_disagreements());
    }

    #[test]
    fn time_offsets_outside_tolerance_are_reported_as_unmatched() {
        let report = compare(&provider(0.0, 1.0), &provider(0.04, 1.0), options());
        assert!(report.pairs.is_empty());
        assert_eq!(report.unmatched_primary.len(), 12);
        assert_eq!(report.unmatched_reference.len(), 12);
    }

    #[test]
    fn two_empty_candidate_lists_are_not_coverage_evidence() {
        let report = compare(&Monotone, &Monotone, options());
        assert!(report.pairs.is_empty());
        assert!(report.unmatched_primary.is_empty());
        assert!(report.unmatched_reference.is_empty());
        assert!(!report.has_observed_disagreements());
        // There is intentionally no `complete`/`covered` field in the report.
    }

    #[test]
    fn rejects_ambiguous_nearby_same_kind_events() {
        let dense = Periodic {
            frequency: 2.0,
            ..provider(0.0, 1.0)
        };
        let mut opts = options();
        opts.tolerance.time_us = 3 * DAY / 10;
        let (mut a, mut b) = (2_000, 2_000);
        assert_eq!(
            compare_latitude_event_providers(&dense, &dense, &body(), opts, &mut a, &mut b,)
                .unwrap_err(),
            GenerationError::AmbiguousLatitudeCrossing
        );
    }

    #[test]
    fn invalid_tolerances_and_depth_fail_before_probing() {
        let mut opts = options();
        opts.tolerance.time_us = -1;
        let (mut a, mut b) = (2_000, 2_000);
        assert_eq!(
            compare_latitude_event_providers(&Monotone, &Monotone, &body(), opts, &mut a, &mut b)
                .unwrap_err(),
            GenerationError::InvalidWindow
        );
        assert_eq!((a, b), (2_000, 2_000));
        opts = options();
        for invalid_speed in [f64::NAN, f64::INFINITY, -0.1] {
            opts.tolerance.signed_speed_degrees_per_day = invalid_speed;
            let (mut a, mut b) = (2_000, 2_000);
            assert_eq!(
                compare_latitude_event_providers(
                    &Monotone,
                    &Monotone,
                    &body(),
                    opts,
                    &mut a,
                    &mut b
                )
                .unwrap_err(),
                GenerationError::InvalidWindow
            );
            assert_eq!((a, b), (2_000, 2_000));
        }
        opts = options();
        for depth in [0, 9] {
            opts.max_depth = depth;
            let (mut a, mut b) = (2_000, 2_000);
            assert_eq!(
                compare_latitude_event_providers(
                    &Monotone,
                    &Monotone,
                    &body(),
                    opts,
                    &mut a,
                    &mut b
                )
                .unwrap_err(),
                GenerationError::InvalidWindow
            );
            assert_eq!((a, b), (2_000, 2_000));
        }
    }

    #[test]
    fn a_starved_reference_budget_never_returns_partial_parity() {
        let (mut a, mut b) = (2_000, 2);
        assert_eq!(
            compare_latitude_event_providers(
                &provider(0.0, 1.0),
                &provider(0.0, 1.0),
                &body(),
                options(),
                &mut a,
                &mut b
            )
            .unwrap_err(),
            GenerationError::TooManySteps
        );
        assert_eq!(b, 0);
        assert!(a < 2_000);
    }

    struct SampleOffset {
        latitude_bias: f64,
        speed_bias: f64,
    }

    impl LatitudeProvider for SampleOffset {
        fn sample_latitude(
            &self,
            _: &BodyId,
            at: TimestampMicros,
        ) -> Result<LatitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LatitudeSample {
                latitude_degrees: 0.2 + 0.02 * days + self.latitude_bias,
                latitude_speed_degrees_per_day: 0.02 + self.speed_bias,
            })
        }
    }

    fn sample_options() -> LatitudeSampleComparisonOptions {
        LatitudeSampleComparisonOptions {
            window: SearchWindow {
                start_us: 0,
                end_us: 2 * DAY,
                step_us: DAY / 4,
                precision_us: 1_000_000,
            },
            tolerance: LatitudeSampleTolerance {
                latitude_degrees: 0.01,
                signed_speed_degrees_per_day: 0.001,
            },
        }
    }

    #[test]
    fn matching_latitude_samples_have_zero_residual_and_shared_utc_grid() {
        let (mut a, mut b) = (10, 10);
        let report = compare_latitude_provider_samples(
            &provider(0.0, 1.0),
            &provider(0.0, 1.0),
            &body(),
            sample_options(),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(report.samples_checked, 8);
        assert!(!report.has_sample_disagreements());
        assert_eq!(report.maximum_absolute_latitude_offset_degrees, 0.0);
        assert_eq!(report.maximum_absolute_speed_offset_degrees_per_day, 0.0);
        assert_eq!((a, b), (2, 2));
    }

    #[test]
    fn systematic_frame_and_speed_biases_show_up_even_without_events() {
        let (mut a, mut b) = (20, 20);
        let report = compare_latitude_provider_samples(
            &SampleOffset {
                latitude_bias: 0.0,
                speed_bias: 0.0,
            },
            &SampleOffset {
                latitude_bias: 0.025,
                speed_bias: -0.004,
            },
            &body(),
            sample_options(),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(report.samples_checked, 8);
        assert_eq!(report.disagreements.len(), 8);
        assert!(report.has_sample_disagreements());
        assert!((report.maximum_absolute_latitude_offset_degrees - 0.025).abs() < 1e-10);
        assert!((report.maximum_absolute_speed_offset_degrees_per_day - 0.004).abs() < 1e-10);
        assert!(report.disagreements.iter().all(|d| {
            !d.latitude_within_tolerance
                && !d.speed_within_tolerance
                && d.latitude_offset_degrees > 0.0
                && d.signed_speed_offset_degrees_per_day < 0.0
        }));
    }

    #[test]
    fn coordinate_and_speed_tolerances_are_independently_applied() {
        let mut opts = sample_options();
        opts.tolerance.latitude_degrees = 0.03;
        let (mut a, mut b) = (20, 20);
        let report = compare_latitude_provider_samples(
            &SampleOffset {
                latitude_bias: 0.0,
                speed_bias: 0.0,
            },
            &SampleOffset {
                latitude_bias: 0.02,
                speed_bias: 0.004,
            },
            &body(),
            opts,
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(report.disagreements.len(), 8);
        assert!(report
            .disagreements
            .iter()
            .all(|d| d.latitude_within_tolerance && !d.speed_within_tolerance));
    }

    #[test]
    fn grid_never_samples_the_exclusive_end_even_if_it_disagrees() {
        struct EndSpike;
        impl LatitudeProvider for EndSpike {
            fn sample_latitude(
                &self,
                _: &BodyId,
                at: TimestampMicros,
            ) -> Result<LatitudeSample, String> {
                Ok(LatitudeSample {
                    latitude_degrees: if at.as_i64() == DAY { 1.0 } else { 0.0 },
                    latitude_speed_degrees_per_day: 0.0,
                })
            }
        }
        struct Zero;
        impl LatitudeProvider for Zero {
            fn sample_latitude(
                &self,
                _: &BodyId,
                _: TimestampMicros,
            ) -> Result<LatitudeSample, String> {
                Ok(LatitudeSample {
                    latitude_degrees: 0.0,
                    latitude_speed_degrees_per_day: 0.0,
                })
            }
        }
        let mut opts = sample_options();
        opts.window.end_us = DAY;
        let (mut a, mut b) = (10, 10);
        let report =
            compare_latitude_provider_samples(&EndSpike, &Zero, &body(), opts, &mut a, &mut b)
                .unwrap();
        assert_eq!(report.samples_checked, 4);
        assert!(report.disagreements.is_empty());
        assert_eq!((a, b), (6, 6));
    }

    #[test]
    fn bad_sample_options_reject_before_any_provider_calls() {
        for bad in [f64::NAN, f64::INFINITY, -0.01] {
            let mut opts = sample_options();
            opts.tolerance.latitude_degrees = bad;
            let (mut a, mut b) = (10, 10);
            assert_eq!(
                compare_latitude_provider_samples(
                    &Monotone,
                    &Monotone,
                    &body(),
                    opts,
                    &mut a,
                    &mut b
                )
                .unwrap_err(),
                GenerationError::InvalidWindow
            );
            assert_eq!((a, b), (10, 10));
            opts = sample_options();
            opts.tolerance.signed_speed_degrees_per_day = bad;
            assert_eq!(
                compare_latitude_provider_samples(
                    &Monotone,
                    &Monotone,
                    &body(),
                    opts,
                    &mut a,
                    &mut b
                )
                .unwrap_err(),
                GenerationError::InvalidWindow
            );
            assert_eq!((a, b), (10, 10));
        }
    }

    #[test]
    fn sample_comparison_budget_exhaustion_never_returns_partial_report() {
        let (mut a, mut b) = (10, 2);
        assert_eq!(
            compare_latitude_provider_samples(
                &Monotone,
                &Monotone,
                &body(),
                sample_options(),
                &mut a,
                &mut b
            )
            .unwrap_err(),
            GenerationError::TooManySteps
        );
        assert_eq!((a, b), (8, 0));
    }

    #[test]
    fn provider_errors_invalid_samples_and_overflowed_residuals_fail_closed() {
        struct InvalidSampleProvider(f64, f64);
        impl LatitudeProvider for InvalidSampleProvider {
            fn sample_latitude(
                &self,
                _: &BodyId,
                _: TimestampMicros,
            ) -> Result<LatitudeSample, String> {
                Ok(LatitudeSample {
                    latitude_degrees: self.0,
                    latitude_speed_degrees_per_day: self.1,
                })
            }
        }
        for invalid in [
            InvalidSampleProvider(f64::NAN, 0.0),
            InvalidSampleProvider(90.1, 0.0),
            InvalidSampleProvider(0.0, f64::INFINITY),
        ] {
            let (mut a, mut b) = (10, 10);
            assert_eq!(
                compare_latitude_provider_samples(
                    &Monotone,
                    &invalid,
                    &body(),
                    sample_options(),
                    &mut a,
                    &mut b
                )
                .unwrap_err(),
                GenerationError::InvalidSample
            );
        }
        let (mut a, mut b) = (10, 10);
        assert_eq!(
            compare_latitude_provider_samples(
                &InvalidSampleProvider(0.0, f64::MAX),
                &InvalidSampleProvider(0.0, -f64::MAX),
                &body(),
                sample_options(),
                &mut a,
                &mut b
            )
            .unwrap_err(),
            GenerationError::InvalidSample
        );
        struct Fails;
        impl LatitudeProvider for Fails {
            fn sample_latitude(
                &self,
                _: &BodyId,
                _: TimestampMicros,
            ) -> Result<LatitudeSample, String> {
                Err("synthetic provider failure".to_string())
            }
        }
        let (mut a, mut b) = (10, 10);
        assert_eq!(
            compare_latitude_provider_samples(
                &Monotone,
                &Fails,
                &body(),
                sample_options(),
                &mut a,
                &mut b
            )
            .unwrap_err(),
            GenerationError::Provider("synthetic provider failure".to_string())
        );
    }
}
