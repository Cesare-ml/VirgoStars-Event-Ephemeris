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

use virgostars_events_core::BodyId;

use crate::{
    latitude_crossings::{
        adaptive_ecliptic_plane_crossings_scan, adaptive_latitude_stations_scan,
        EclipticPlaneDirection, LatitudeMotionDirection,
    },
    latitude_extrema::{adaptive_latitude_speed_extrema_scan, LatitudeSpeedExtremumKind},
    GenerationError, LatitudeProvider, SearchWindow,
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
}
