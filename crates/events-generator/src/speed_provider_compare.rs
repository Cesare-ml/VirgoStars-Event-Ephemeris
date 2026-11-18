/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Candidate-only comparison across two caller-supplied longitude providers.
//!
//! The caller must independently substantiate the providers' real source,
//! kernel, settings and licensing. Equal outputs (including two empty scans)
//! are NOT proof of scientific independence, event completeness or UTC coverage.

use virgostars_events_core::BodyId;

use crate::{
    speed_extrema::{adaptive_speed_extrema_scan, SpeedExtremumCandidate, SpeedExtremumKind},
    GenerationError, LongitudeProvider, SearchWindow,
};

/// Explicit comparison tolerances, not universal scientific accuracy claims.
#[derive(Clone, Copy, Debug)]
pub struct SpeedProviderComparisonTolerance {
    /// Maximum difference between candidate bracket midpoints, in UTC µs.
    pub time_us: i64,
    /// Maximum difference between sampled signed speeds, in degrees/day.
    pub signed_speed_degrees_per_day: f64,
}

/// Bounded primary/reference scan settings and numerical match thresholds.
#[derive(Clone, Copy, Debug)]
pub struct SpeedProviderComparisonOptions {
    pub window: SearchWindow,
    pub max_depth: u8,
    pub tolerance: SpeedProviderComparisonTolerance,
}

/// A one-to-one, same-body, same-kind, time-aligned candidate pair.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedProviderCandidatePair {
    pub primary: SpeedExtremumCandidate,
    pub reference: SpeedExtremumCandidate,
    /// Signed reference-minus-primary midpoint offset. i128 avoids overflow
    /// when comparing two valid but extremely distant i64 UTC timestamps.
    pub reference_offset_us: i128,
    /// Signed reference-minus-primary speed difference, in degrees/day.
    pub reference_speed_offset_degrees_per_day: f64,
    pub speed_within_tolerance: bool,
}

/// Diagnostic comparison of two independent *scans*, not certified coverage.
#[derive(Debug, Default)]
pub struct SpeedProviderComparisonReport {
    pub pairs: Vec<SpeedProviderCandidatePair>,
    pub unmatched_primary: Vec<SpeedExtremumCandidate>,
    pub unmatched_reference: Vec<SpeedExtremumCandidate>,
    pub primary_interior_probes: usize,
    pub reference_interior_probes: usize,
}

impl SpeedProviderComparisonReport {
    /// Disagreements among *observed candidates*. A `false` result, especially
    /// with two empty candidate lists, never certifies an interval as complete.
    pub fn has_observed_disagreements(&self) -> bool {
        !self.unmatched_primary.is_empty()
            || !self.unmatched_reference.is_empty()
            || self.pairs.iter().any(|pair| !pair.speed_within_tolerance)
    }
}

fn candidate_midpoint_us(candidate: &SpeedExtremumCandidate) -> i128 {
    candidate.bracket_start_us as i128
        + (candidate.bracket_end_us as i128 - candidate.bracket_start_us as i128) / 2
}

fn validate_unambiguous_matching(
    candidates: &[&SpeedExtremumCandidate],
    time_tolerance_us: i64,
) -> Result<(), GenerationError> {
    // More than one same-kind candidate within the matching neighborhood
    // makes greedy pairing arbitrary. Fail closed; ask for a smaller
    // tolerance or an independently reviewed event-identity oracle.
    for pair in candidates.windows(2) {
        if candidate_midpoint_us(pair[1]) - candidate_midpoint_us(pair[0])
            <= 2 * time_tolerance_us as i128
        {
            return Err(GenerationError::AmbiguousSpeedExtremum);
        }
    }
    Ok(())
}

/// Rescan an identical half-open UTC window with both providers and reconcile
/// observed direct maxima and retrograde-magnitude maxima chronologically.
///
/// Each provider gets its **own** probe budget so one cannot silently starve
/// the other; exhausted budgets fail without returning a partial report.
/// Candidates are paired once by kind and midpoint time within `tolerance`.
/// Same-kind events closer than twice the time tolerance cannot be matched
/// unambiguously and fail closed. Speed disagreements are preserved as pairs
/// with `speed_within_tolerance=false`; missing/extra observed peaks remain in
/// separate lists. No inference about unsampled peaks or full historical
/// coverage follows from a consistent or empty report.
pub fn compare_speed_extrema_providers<A: LongitudeProvider, B: LongitudeProvider>(
    primary: &A,
    reference: &B,
    body: &BodyId,
    options: SpeedProviderComparisonOptions,
    primary_remaining_probes: &mut usize,
    reference_remaining_probes: &mut usize,
) -> Result<SpeedProviderComparisonReport, GenerationError> {
    let SpeedProviderComparisonOptions {
        window,
        max_depth,
        tolerance,
    } = options;
    if tolerance.time_us < 0
        || !tolerance.signed_speed_degrees_per_day.is_finite()
        || tolerance.signed_speed_degrees_per_day < 0.0
    {
        return Err(GenerationError::InvalidWindow);
    }
    let a =
        adaptive_speed_extrema_scan(primary, body, window, max_depth, primary_remaining_probes)?;
    let b = adaptive_speed_extrema_scan(
        reference,
        body,
        window,
        max_depth,
        reference_remaining_probes,
    )?;
    let mut report = SpeedProviderComparisonReport {
        primary_interior_probes: a.interior_probes,
        reference_interior_probes: b.interior_probes,
        ..SpeedProviderComparisonReport::default()
    };

    for kind in [
        SpeedExtremumKind::DirectMaximum,
        SpeedExtremumKind::RetrogradeMagnitudeMaximum,
    ] {
        let primary_candidates: Vec<_> = a
            .candidates
            .iter()
            .filter(|candidate| candidate.kind == kind)
            .collect();
        let reference_candidates: Vec<_> = b
            .candidates
            .iter()
            .filter(|candidate| candidate.kind == kind)
            .collect();
        validate_unambiguous_matching(&primary_candidates, tolerance.time_us)?;
        validate_unambiguous_matching(&reference_candidates, tolerance.time_us)?;
        let mut i = 0;
        let mut j = 0;
        while i < primary_candidates.len() && j < reference_candidates.len() {
            let first = primary_candidates[i];
            let second = reference_candidates[j];
            let offset = candidate_midpoint_us(second) - candidate_midpoint_us(first);
            if offset.abs() <= tolerance.time_us as i128 {
                let speed_offset = second.sampled_signed_speed_degrees_per_day
                    - first.sampled_signed_speed_degrees_per_day;
                if !speed_offset.is_finite() {
                    return Err(GenerationError::InvalidSample);
                }
                report.pairs.push(SpeedProviderCandidatePair {
                    primary: first.clone(),
                    reference: second.clone(),
                    reference_offset_us: offset,
                    reference_speed_offset_degrees_per_day: speed_offset,
                    speed_within_tolerance: speed_offset.abs()
                        <= tolerance.signed_speed_degrees_per_day,
                });
                i += 1;
                j += 1;
            } else if offset > 0 {
                report.unmatched_primary.push(first.clone());
                i += 1;
            } else {
                report.unmatched_reference.push(second.clone());
                j += 1;
            }
        }
        report.unmatched_primary.extend(
            primary_candidates[i..]
                .iter()
                .map(|candidate| (*candidate).clone()),
        );
        report.unmatched_reference.extend(
            reference_candidates[j..]
                .iter()
                .map(|candidate| (*candidate).clone()),
        );
    }

    report.pairs.sort_by_key(|pair| {
        (
            pair.primary.bracket_start_us,
            pair.primary.kind,
            pair.primary.bracket_end_us,
        )
    });
    report
        .unmatched_primary
        .sort_by_key(|candidate| (candidate.bracket_start_us, candidate.kind));
    report
        .unmatched_reference
        .sort_by_key(|candidate| (candidate.bracket_start_us, candidate.kind));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LongitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct ParabolicProvider {
        peak_shift_us: i64,
        signed_speed_bias: f64,
        retrograde: bool,
    }

    impl LongitudeProvider for ParabolicProvider {
        fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let day = at.as_i64() as f64 / DAY as f64;
            let center = 2.0 + self.peak_shift_us as f64 / DAY as f64;
            let sign = if self.retrograde { -1.0 } else { 1.0 };
            let speed = (5.0 + self.signed_speed_bias) - (day - center).powi(2);
            Ok(LongitudeSample {
                longitude_degrees: sign
                    * ((5.0 + self.signed_speed_bias) * day - (day - center).powi(3) / 3.0),
                longitude_speed_degrees_per_day: sign * speed,
            })
        }
    }

    struct LinearProvider;
    impl LongitudeProvider for LinearProvider {
        fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let day = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: 3.0 * day + day * day / 2.0,
                longitude_speed_degrees_per_day: 3.0 + day,
            })
        }
    }

    fn body() -> BodyId {
        BodyId::try_from("Mars").unwrap()
    }

    fn window() -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: 5 * DAY,
            step_us: DAY,
            precision_us: 1_000_000,
        }
    }

    fn tolerance() -> SpeedProviderComparisonTolerance {
        SpeedProviderComparisonTolerance {
            time_us: 3_600_000_000,
            signed_speed_degrees_per_day: 0.1,
        }
    }

    fn options(max_depth: u8) -> SpeedProviderComparisonOptions {
        SpeedProviderComparisonOptions {
            window: window(),
            max_depth,
            tolerance: tolerance(),
        }
    }

    fn provider(shift_us: i64, bias: f64, retrograde: bool) -> ParabolicProvider {
        ParabolicProvider {
            peak_shift_us: shift_us,
            signed_speed_bias: bias,
            retrograde,
        }
    }

    #[test]
    fn matches_nearby_direct_peaks_with_bounded_timestamp_offset() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &provider(0, 0.0, false),
            &provider(1_800_000_000, 0.0, false),
            &body(),
            options(3),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(comparison.pairs.len(), 1);
        assert!(comparison.pairs[0].speed_within_tolerance);
        assert!((comparison.pairs[0].reference_offset_us - 1_800_000_000).abs() < 1_000_000);
        assert!(!comparison.has_observed_disagreements());
        assert!(comparison.primary_interior_probes > 0);
        assert!(comparison.reference_interior_probes > 0);
    }

    #[test]
    fn keeps_unmatched_events_when_reference_peak_is_outside_time_tolerance() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &provider(0, 0.0, false),
            &provider(6 * 3_600_000_000, 0.0, false),
            &body(),
            options(2),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert!(comparison.pairs.is_empty());
        assert_eq!(comparison.unmatched_primary.len(), 1);
        assert_eq!(comparison.unmatched_reference.len(), 1);
        assert!(comparison.has_observed_disagreements());
    }

    #[test]
    fn keeps_paired_but_inconsistent_speed_values_visible() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &provider(0, 0.0, true),
            &provider(0, 0.5, true),
            &body(),
            options(2),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(comparison.pairs.len(), 1);
        assert_eq!(
            comparison.pairs[0].primary.kind,
            SpeedExtremumKind::RetrogradeMagnitudeMaximum
        );
        assert!(!comparison.pairs[0].speed_within_tolerance);
        assert!(comparison.pairs[0].reference_speed_offset_degrees_per_day < -0.4);
        assert!(comparison.has_observed_disagreements());
    }

    #[test]
    fn reports_missing_reference_peak_instead_of_silent_consistency() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &provider(0, 0.0, false),
            &LinearProvider,
            &body(),
            options(3),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert_eq!(comparison.unmatched_primary.len(), 1);
        assert!(comparison.unmatched_reference.is_empty());
        assert!(comparison.has_observed_disagreements());
    }

    #[test]
    fn reports_extra_reference_peak_when_primary_has_none() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &LinearProvider,
            &provider(0, 0.0, true),
            &body(),
            options(2),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert!(comparison.unmatched_primary.is_empty());
        assert_eq!(comparison.unmatched_reference.len(), 1);
        assert!(comparison.pairs.is_empty());
        assert!(comparison.has_observed_disagreements());
    }

    #[test]
    fn refuses_ambiguous_matching_when_same_kind_events_are_too_close() {
        struct DensePeaks;
        impl LongitudeProvider for DensePeaks {
            fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
                let day = at.as_i64() as f64 / DAY as f64;
                let angle = 2.0 * std::f64::consts::PI * day;
                Ok(LongitudeSample {
                    longitude_degrees: 3.5 * day
                        - (2.0 * angle).sin() / (8.0 * std::f64::consts::PI),
                    longitude_speed_degrees_per_day: 3.0 + angle.sin().powi(2),
                })
            }
        }
        let mut a = 2_000;
        let mut b = 2_000;
        let mut settings = options(1);
        settings.window.end_us = 2 * DAY;
        settings.tolerance.time_us = 3 * DAY / 10;
        assert_eq!(
            compare_speed_extrema_providers(
                &DensePeaks,
                &DensePeaks,
                &body(),
                settings,
                &mut a,
                &mut b,
            )
            .unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum
        );
    }

    #[test]
    fn two_empty_observed_lists_are_not_coverage_evidence() {
        let mut a = 1_000;
        let mut b = 1_000;
        let comparison = compare_speed_extrema_providers(
            &LinearProvider,
            &LinearProvider,
            &body(),
            options(2),
            &mut a,
            &mut b,
        )
        .unwrap();
        assert!(comparison.pairs.is_empty());
        assert!(comparison.unmatched_primary.is_empty());
        assert!(comparison.unmatched_reference.is_empty());
        assert!(!comparison.has_observed_disagreements());
        // The API has deliberately no 'complete', 'certified' or 'covered'
        // status; only observed-event differences can be reported here.
    }

    #[test]
    fn invalid_tolerances_and_exhausted_reference_budget_fail_closed() {
        let mut a = 1_000;
        let mut b = 1_000;
        let mut invalid = tolerance();
        invalid.time_us = -1;
        assert_eq!(
            compare_speed_extrema_providers(
                &LinearProvider,
                &LinearProvider,
                &body(),
                SpeedProviderComparisonOptions {
                    tolerance: invalid,
                    ..options(2)
                },
                &mut a,
                &mut b,
            )
            .unwrap_err(),
            GenerationError::InvalidWindow
        );
        invalid = tolerance();
        invalid.signed_speed_degrees_per_day = f64::NAN;
        assert_eq!(
            compare_speed_extrema_providers(
                &LinearProvider,
                &LinearProvider,
                &body(),
                SpeedProviderComparisonOptions {
                    tolerance: invalid,
                    ..options(2)
                },
                &mut a,
                &mut b,
            )
            .unwrap_err(),
            GenerationError::InvalidWindow
        );
        let mut b = 2;
        assert_eq!(
            compare_speed_extrema_providers(
                &LinearProvider,
                &LinearProvider,
                &body(),
                options(2),
                &mut a,
                &mut b,
            )
            .unwrap_err(),
            GenerationError::TooManySteps
        );
    }
}
