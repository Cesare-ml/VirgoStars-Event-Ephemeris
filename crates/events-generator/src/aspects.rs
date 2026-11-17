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
}
