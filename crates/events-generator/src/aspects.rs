/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Candidate-only exact longitudinal aspects for two celestial bodies.
//!
//! Uses geocentric tropical ecliptic longitudes from one provider; never reads
//! or alters the owner dataset. A finite sampling grid can hide touching or
//! multiple crossings, even when endpoint speeds look safe. **No coverage is
//! certified** by an empty result. Independently validate real timestamps.

use virgostars_events_core::BodyId;

use crate::{
    checked_sample, midpoint, needs_refinement, next_time, GenerationError, LongitudeProvider,
    SearchWindow,
};

const MAX_RELATIVE_STEP_DEGREES: f64 = 30.0;
const MICROS_PER_DAY: f64 = 86_400_000_000.0;

/// Longitude separation measured in the ecliptic plane, not 3D sky distance.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ExactAspectKind {
    Conjunction,
    Sextile,
    Square,
    Trine,
}

impl ExactAspectKind {
    pub const fn degrees(self) -> u16 {
        match self {
            Self::Conjunction => 0,
            Self::Sextile => 60,
            Self::Square => 90,
            Self::Trine => 120,
        }
    }

    pub const fn all() -> [Self; 4] {
        [Self::Conjunction, Self::Sextile, Self::Square, Self::Trine]
    }
}

/// An exact-aspect **bracket**, never an authoritative astronomical timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactAspectCandidate {
    /// Canonical name order, so A/B and B/A are not emitted twice.
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub aspect: ExactAspectKind,
    /// `longitude(B) - longitude(A)` target: 0, 60/300, 90/270, 120/240.
    pub relative_target_degrees: u16,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
}

#[derive(Clone, Copy)]
struct PairSample {
    separation: f64,
    relative_speed: f64,
}

fn sample_pair<P: LongitudeProvider>(
    provider: &P,
    a: &BodyId,
    b: &BodyId,
    t: i64,
) -> Result<PairSample, GenerationError> {
    let a_sample = checked_sample(provider, a, t)?;
    let b_sample = checked_sample(provider, b, t)?;
    Ok(PairSample {
        separation: (b_sample.longitude_degrees - a_sample.longitude_degrees).rem_euclid(360.0),
        relative_speed: b_sample.longitude_speed_degrees_per_day
            - a_sample.longitude_speed_degrees_per_day,
    })
}

fn short_delta(a: f64, b: f64) -> f64 {
    let mut delta = (b - a).rem_euclid(360.0);
    if delta > 180.0 {
        delta -= 360.0;
    }
    delta
}

fn targets() -> [(ExactAspectKind, u16); 7] {
    [
        (ExactAspectKind::Conjunction, 0),
        (ExactAspectKind::Sextile, 60),
        (ExactAspectKind::Square, 90),
        (ExactAspectKind::Trine, 120),
        (ExactAspectKind::Trine, 240),
        (ExactAspectKind::Square, 270),
        (ExactAspectKind::Sextile, 300),
    ]
}

/// Bracket potential *crossings* of 0°, ±60°, ±90°, ±120° between two bodies.
///
/// Input pair order is canonicalized. The time range is `[start_us, end_us)`;
/// a target hit at `end_us` is deliberately excluded. Each sample step must
/// have less than 30° relative displacement, including the speed-based bound;
/// coarse or ambiguous steps fail rather than claiming no events. Tangential
/// contacts without a sign change and multiple hidden crossings are outside
/// this first candidate detector and require independent, adaptive review.
pub fn exact_aspect_candidates<P: LongitudeProvider>(
    provider: &P,
    body_a: &BodyId,
    body_b: &BodyId,
    window: SearchWindow,
) -> Result<Vec<ExactAspectCandidate>, GenerationError> {
    window.validate()?;
    if body_a == body_b {
        return Err(GenerationError::InvalidBodyPair);
    }
    let (a, b) = if body_a.as_str() < body_b.as_str() {
        (body_a, body_b)
    } else {
        (body_b, body_a)
    };
    let mut found = Vec::new();
    let mut t = window.start_us;
    let mut before = sample_pair(provider, a, b, t)?;

    while t < window.end_us {
        let end = next_time(t, window);
        let after = sample_pair(provider, a, b, end)?;
        let delta = short_delta(before.separation, after.separation);
        let days = (end as i128 - t as i128) as f64 / MICROS_PER_DAY;
        let maximum_estimated_sweep =
            before.relative_speed.abs().max(after.relative_speed.abs()) * days;
        if delta.abs() >= MAX_RELATIVE_STEP_DEGREES
            || maximum_estimated_sweep >= MAX_RELATIVE_STEP_DEGREES
        {
            return Err(GenerationError::AmbiguousAspectStep);
        }

        if delta != 0.0 {
            for (aspect, target) in targets() {
                let forward_distance = (f64::from(target) - before.separation).rem_euclid(360.0);
                let backward_distance = (before.separation - f64::from(target)).rem_euclid(360.0);
                // Include hits at the segment start, exclude those at its end.
                let crossing = if delta > 0.0 {
                    forward_distance < delta
                } else {
                    backward_distance < -delta
                };
                if !crossing {
                    continue;
                }
                // A zero-speed exact hit at the segment boundary could be a
                // tangency, not a traversal. Do not fabricate a crossing;
                // an independent extrema/tangency detector is still needed.
                if (forward_distance == 0.0 || backward_distance == 0.0)
                    && before.relative_speed == 0.0
                {
                    continue;
                }
                let target_unwrapped = before.separation
                    + if delta > 0.0 {
                        forward_distance
                    } else {
                        -backward_distance
                    };
                let mut left = t;
                let mut right = end;
                while needs_refinement(left, right, window.precision_us) {
                    let mid = midpoint(left, right);
                    let midpoint_sample = sample_pair(provider, a, b, mid)?;
                    let middle_delta = short_delta(before.separation, midpoint_sample.separation);
                    if middle_delta.abs() >= MAX_RELATIVE_STEP_DEGREES {
                        return Err(GenerationError::AmbiguousAspectStep);
                    }
                    let middle_unwrapped = before.separation + middle_delta;
                    if (delta > 0.0 && middle_unwrapped < target_unwrapped)
                        || (delta < 0.0 && middle_unwrapped > target_unwrapped)
                    {
                        left = mid;
                    } else {
                        right = mid;
                    }
                }
                found.push(ExactAspectCandidate {
                    body_a: a.clone(),
                    body_b: b.clone(),
                    aspect,
                    relative_target_degrees: target,
                    bracket_start_us: left,
                    bracket_end_us: right,
                });
            }
        }
        t = end;
        before = after;
    }
    found.sort_by_key(|candidate| {
        (
            candidate.bracket_start_us,
            candidate.relative_target_degrees,
        )
    });
    Ok(found)
}

/// Diagnostic counters for a risk-triggered rescan. Neither the counters nor
/// the returned crossings establish complete astronomical coverage.
#[derive(Default, Debug)]
pub struct AdaptiveAspectReport {
    pub candidates: Vec<ExactAspectCandidate>,
    pub midpoint_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

struct AdaptiveAspectScan<'a, P> {
    provider: &'a P,
    body_a: &'a BodyId,
    body_b: &'a BodyId,
    precision_us: i64,
    remaining_probes: &'a mut usize,
    report: AdaptiveAspectReport,
}

impl<P: LongitudeProvider> AdaptiveAspectScan<'_, P> {
    fn segment(
        &mut self,
        start_us: i64,
        end_us: i64,
        before: PairSample,
        after: PairSample,
        depth_remaining: u8,
    ) -> Result<(), GenerationError> {
        if depth_remaining > 0 && needs_refinement(start_us, end_us, self.precision_us) {
            let mid_us = midpoint(start_us, end_us);
            if mid_us > start_us && mid_us < end_us {
                if *self.remaining_probes == 0 {
                    return Err(GenerationError::TooManySteps);
                }
                *self.remaining_probes -= 1;
                self.report.midpoint_probes += 1;
                let mid = sample_pair(self.provider, self.body_a, self.body_b, mid_us)?;

                let left_delta = short_delta(before.separation, mid.separation);
                let right_delta = short_delta(mid.separation, after.separation);
                let whole_delta = short_delta(before.separation, after.separation);
                let days = (end_us as i128 - start_us as i128) as f64 / MICROS_PER_DAY;
                let speed_bound = before
                    .relative_speed
                    .abs()
                    .max(after.relative_speed.abs())
                    .max(mid.relative_speed.abs())
                    * days;

                // An equal-looking pair of endpoints may hide a forward and
                // reverse crossing. A midpoint speed reversal, speed dip or
                // nonmonotonic separation triggers recursive inspection.
                let speed_reversal = (before.relative_speed < 0.0) != (mid.relative_speed < 0.0)
                    || (mid.relative_speed < 0.0) != (after.relative_speed < 0.0);
                let direction_reversal = (left_delta < 0.0) != (right_delta < 0.0);
                let midpoint_speed_dip = mid.relative_speed.abs() * 2.0
                    < before.relative_speed.abs().min(after.relative_speed.abs());
                let risky = speed_reversal
                    || direction_reversal
                    || midpoint_speed_dip
                    || left_delta.abs() >= MAX_RELATIVE_STEP_DEGREES
                    || right_delta.abs() >= MAX_RELATIVE_STEP_DEGREES
                    || whole_delta.abs() >= MAX_RELATIVE_STEP_DEGREES
                    || speed_bound >= MAX_RELATIVE_STEP_DEGREES;

                if risky {
                    self.report.refined_segments += 1;
                    self.segment(start_us, mid_us, before, mid, depth_remaining - 1)?;
                    self.segment(mid_us, end_us, mid, after, depth_remaining - 1)?;
                    return Ok(());
                }
            }
        }

        self.report.leaf_segments += 1;
        let span_us = end_us - start_us;
        let leaf_window = SearchWindow {
            start_us,
            end_us,
            step_us: span_us,
            precision_us: self.precision_us.min(span_us),
        };
        self.report.candidates.extend(exact_aspect_candidates(
            self.provider,
            self.body_a,
            self.body_b,
            leaf_window,
        )?);
        Ok(())
    }
}

/// Midpoint-rescan potentially risky aspect intervals, with a strict shared
/// probe budget and bounded recursion depth (1..=8). A phase excursion between
/// samples can still be missed when all three samples look quiet. In
/// particular, exact tangential contacts remain unimplemented and a zero
/// candidate count never implies audited absence.
pub fn adaptive_exact_aspect_scan<P: LongitudeProvider>(
    provider: &P,
    body_a: &BodyId,
    body_b: &BodyId,
    window: SearchWindow,
    max_depth: u8,
    remaining_probes: &mut usize,
) -> Result<AdaptiveAspectReport, GenerationError> {
    window.validate()?;
    if body_a == body_b {
        return Err(GenerationError::InvalidBodyPair);
    }
    if max_depth == 0 || max_depth > 8 || *remaining_probes == 0 {
        return Err(GenerationError::InvalidWindow);
    }
    if (window.step_us as i128) / (1_i128 << max_depth) < window.precision_us as i128 {
        return Err(GenerationError::InvalidWindow);
    }

    let (body_a, body_b) = if body_a.as_str() < body_b.as_str() {
        (body_a, body_b)
    } else {
        (body_b, body_a)
    };
    let mut scan = AdaptiveAspectScan {
        provider,
        body_a,
        body_b,
        precision_us: window.precision_us,
        remaining_probes,
        report: AdaptiveAspectReport::default(),
    };
    let mut start_us = window.start_us;
    let mut before = sample_pair(provider, body_a, body_b, start_us)?;
    while start_us < window.end_us {
        let end_us = next_time(start_us, window);
        let after = sample_pair(provider, body_a, body_b, end_us)?;
        scan.segment(start_us, end_us, before, after, max_depth)?;
        start_us = end_us;
        before = after;
    }
    scan.report.candidates.sort_by_key(|candidate| {
        (
            candidate.bracket_start_us,
            candidate.relative_target_degrees,
        )
    });
    Ok(scan.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LongitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct LinearPair {
        start: f64,
        rate_per_day: f64,
    }

    impl LongitudeProvider for LinearPair {
        fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            if body.as_str() == "Mars" {
                return Ok(LongitudeSample {
                    longitude_degrees: 0.0,
                    longitude_speed_degrees_per_day: 0.0,
                });
            }
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: self.start + self.rate_per_day * days,
                longitude_speed_degrees_per_day: self.rate_per_day,
            })
        }
    }

    fn body(name: &str) -> BodyId {
        BodyId::try_from(name).unwrap()
    }

    fn window(days: i64) -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: days * DAY,
            step_us: DAY / 2,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn finds_sextile_square_and_trine_in_canonical_pair_order() {
        let provider = LinearPair {
            start: 1.0,
            rate_per_day: 20.0,
        };
        let forward =
            exact_aspect_candidates(&provider, &body("Mars"), &body("Venus"), window(8)).unwrap();
        let reverse =
            exact_aspect_candidates(&provider, &body("Venus"), &body("Mars"), window(8)).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), 3);
        assert_eq!(
            forward.iter().map(|item| item.aspect).collect::<Vec<_>>(),
            [
                ExactAspectKind::Sextile,
                ExactAspectKind::Square,
                ExactAspectKind::Trine
            ]
        );
        for (item, expected_days) in forward.iter().zip([2.95, 4.45, 5.95]) {
            let expected = (expected_days * DAY as f64) as i64;
            assert!(item.bracket_start_us <= expected);
            assert!(item.bracket_end_us >= expected);
            assert!(item.bracket_end_us - item.bracket_start_us <= 1_000_000);
            assert_eq!(item.body_a.as_str(), "Mars");
            assert_eq!(item.body_b.as_str(), "Venus");
        }
    }

    #[test]
    fn wraps_360_and_includes_start_not_end() {
        let start = LinearPair {
            start: 350.0,
            rate_per_day: 20.0,
        };
        let found =
            exact_aspect_candidates(&start, &body("Mars"), &body("Venus"), window(1)).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].aspect, ExactAspectKind::Conjunction);
        assert!(found[0].bracket_start_us <= DAY / 2);
        assert!(found[0].bracket_end_us >= DAY / 2);

        let at_start = LinearPair {
            start: 0.0,
            rate_per_day: 20.0,
        };
        let initial =
            exact_aspect_candidates(&at_start, &body("Mars"), &body("Venus"), window(1)).unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].bracket_start_us, 0);

        let at_end = LinearPair {
            start: 40.0,
            rate_per_day: 20.0,
        };
        let excluded =
            exact_aspect_candidates(&at_end, &body("Mars"), &body("Venus"), window(1)).unwrap();
        assert!(
            excluded.is_empty(),
            "a sextile at exclusive end must not be included"
        );
    }

    #[test]
    fn detects_retrograde_conjunction_and_opposite_orientations() {
        let retro = LinearPair {
            start: 10.0,
            rate_per_day: -20.0,
        };
        let found =
            exact_aspect_candidates(&retro, &body("Mars"), &body("Venus"), window(1)).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].relative_target_degrees, 0);
        assert_eq!(found[0].aspect, ExactAspectKind::Conjunction);

        let forward = LinearPair {
            start: 250.0,
            rate_per_day: 20.0,
        };
        let other =
            exact_aspect_candidates(&forward, &body("Mars"), &body("Venus"), window(4)).unwrap();
        assert_eq!(
            other
                .iter()
                .map(|item| (item.aspect, item.relative_target_degrees))
                .collect::<Vec<_>>(),
            [
                (ExactAspectKind::Square, 270),
                (ExactAspectKind::Sextile, 300)
            ]
        );
    }

    #[test]
    fn rejects_same_body_and_coarse_relative_steps() {
        let coarse = LinearPair {
            start: 5.0,
            rate_per_day: 60.0,
        };
        assert_eq!(
            exact_aspect_candidates(&coarse, &body("Mars"), &body("Mars"), window(1)).unwrap_err(),
            GenerationError::InvalidBodyPair
        );
        assert_eq!(
            exact_aspect_candidates(&coarse, &body("Mars"), &body("Venus"), window(1)).unwrap_err(),
            GenerationError::AmbiguousAspectStep
        );
    }

    #[test]
    fn invalid_provider_samples_fail_closed() {
        let provider = LinearPair {
            start: f64::NAN,
            rate_per_day: 1.0,
        };
        assert_eq!(
            exact_aspect_candidates(&provider, &body("Mars"), &body("Venus"), window(1))
                .unwrap_err(),
            GenerationError::InvalidSample
        );
    }

    #[test]
    fn non_crossing_tangency_does_not_claim_completeness() {
        struct Touch;
        impl LongitudeProvider for Touch {
            fn sample(
                &self,
                body: &BodyId,
                at: TimestampMicros,
            ) -> Result<LongitudeSample, String> {
                if body.as_str() == "Mars" {
                    return Ok(LongitudeSample {
                        longitude_degrees: 0.0,
                        longitude_speed_degrees_per_day: 0.0,
                    });
                }
                let day = at.as_i64() as f64 / DAY as f64;
                Ok(LongitudeSample {
                    longitude_degrees: 60.0 + (day - 1.0).powi(2),
                    longitude_speed_degrees_per_day: 2.0 * (day - 1.0),
                })
            }
        }
        // Touches 60° without changing side. A crossing-only detector is
        // deliberately insufficient to certify absence of exact contacts.
        assert!(
            exact_aspect_candidates(&Touch, &body("Mars"), &body("Venus"), window(2))
                .unwrap()
                .is_empty()
        );
    }

    struct DoubleSextile;

    impl LongitudeProvider for DoubleSextile {
        fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            if body.as_str() == "Mars" {
                return Ok(LongitudeSample {
                    longitude_degrees: 0.0,
                    longitude_speed_degrees_per_day: 0.0,
                });
            }
            let day = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: 61.0 - 4.0 * day + 2.0 * day * day,
                longitude_speed_degrees_per_day: -4.0 + 4.0 * day,
            })
        }
    }

    fn double_crossing_window() -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: 2 * DAY,
            step_us: 2 * DAY,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn adaptive_rescan_finds_two_crossings_hidden_by_equal_endpoints() {
        let a = body("Mars");
        let b = body("Venus");
        assert!(
            exact_aspect_candidates(&DoubleSextile, &a, &b, double_crossing_window())
                .unwrap()
                .is_empty(),
            "initial coarse samples hide both sextile crossings"
        );
        let mut budget = 100;
        let report = adaptive_exact_aspect_scan(
            &DoubleSextile,
            &a,
            &b,
            double_crossing_window(),
            6,
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert!(report.refined_segments > 0);
        assert!(report.midpoint_probes > 0);
        for (candidate, expected_day) in report
            .candidates
            .iter()
            .zip([1.0 - 0.5_f64.sqrt(), 1.0 + 0.5_f64.sqrt()])
        {
            assert_eq!(candidate.aspect, ExactAspectKind::Sextile);
            assert_eq!(candidate.relative_target_degrees, 60);
            let expected = (expected_day * DAY as f64) as i64;
            assert!(candidate.bracket_start_us <= expected);
            assert!(candidate.bracket_end_us >= expected);
        }
    }

    #[test]
    fn adaptive_rescan_fails_closed_when_probe_budget_exhausted() {
        let mut budget = 1;
        let result = adaptive_exact_aspect_scan(
            &DoubleSextile,
            &body("Venus"),
            &body("Mars"),
            double_crossing_window(),
            6,
            &mut budget,
        );
        assert_eq!(result.unwrap_err(), GenerationError::TooManySteps);
    }

    #[test]
    fn adaptive_rescan_rejects_invalid_depth_without_provider_queries() {
        let mut budget = 10;
        assert_eq!(
            adaptive_exact_aspect_scan(
                &DoubleSextile,
                &body("Mars"),
                &body("Venus"),
                double_crossing_window(),
                0,
                &mut budget,
            )
            .unwrap_err(),
            GenerationError::InvalidWindow
        );
        assert_eq!(budget, 10);
    }
}
