/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Developer-only numerical crosschecks for unverified speed-extrema candidates.
//!
//! The longitude-derived central speed and the signed speed-slope test use
//! different numerical quantities but the **same** supplied provider. They
//! are NOT independent astronomical ephemerides or completeness evidence.

use crate::{
    checked_sample, speed_extrema::SpeedExtremumCandidate, GenerationError, LongitudeProvider,
};

const US_PER_DAY: f64 = 86_400_000_000.0;

/// Caller-chosen window and tolerances; no universal astronomical thresholds.
#[derive(Clone, Copy, Debug)]
pub struct SpeedCrosscheckOptions {
    /// Inclusive UTC/POSIX microsecond start of an independently chosen review window.
    pub start_us: i64,
    /// Exclusive UTC/POSIX microsecond end of the review window.
    pub end_us: i64,
    /// Symmetric time distance from the candidate bracket midpoint to each probe.
    pub half_span_us: i64,
    /// Tolerated mismatch between provider speed, longitude secant and candidate speed.
    pub speed_tolerance_degrees_per_day: f64,
    /// Slope-magnitude threshold below which acceleration evidence is inconclusive.
    pub acceleration_noise_floor_degrees_per_day_squared: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedCrosscheckStatus {
    /// Both accelerations point toward the candidate and speed values agree.
    SupportsCandidate,
    /// Strong opposite-slope evidence or inconsistent numerical speed values.
    ContradictsCandidate,
    /// No contradiction was established, but the acceleration signal is weak.
    Inconclusive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedCrosscheckReport {
    pub status: SpeedCrosscheckStatus,
    pub center_us: i64,
    /// Finite difference d(signed longitude speed)/dt immediately before candidate.
    pub left_acceleration_degrees_per_day_squared: f64,
    /// Finite difference d(signed longitude speed)/dt immediately after candidate.
    pub right_acceleration_degrees_per_day_squared: f64,
    /// Central time difference of *longitude*, reduced to the shortest arc.
    pub longitude_secant_speed_degrees_per_day: f64,
    pub provider_center_speed_degrees_per_day: f64,
    /// Absolute discrepancy: longitude finite-difference speed vs provider dλ/dt.
    pub speed_consistency_residual_degrees_per_day: f64,
    /// Absolute discrepancy: stored candidate speed vs newly sampled center speed.
    pub candidate_speed_residual_degrees_per_day: f64,
}

/// Crosscheck one precomputed speed-extremum candidate against a signed-speed
/// acceleration pattern and independently differentiated longitude samples.
///
/// Three samples are taken at `center_us ± half_span_us` and `center_us`.
/// The acceleration slopes should be positive then negative for a direct
/// speed maximum, or negative then positive for a retrograde speed minimum.
/// A central longitude secant additionally checks that the provider's speed
/// is consistent with its *own* positions. This does **not** establish truth
/// against an independently sourced ephemeris, guarantee a unique maximum,
/// or certify an interval with no omitted events.
///
/// Invalid boundaries, a speed ambiguity at ±180° longitude travel, nonfinite
/// inputs and exhausted budgets fail closed. A `SupportsCandidate` result
/// means **numerical consistency only** and cannot be used to approve a P0
/// record or change historical UTC coverage status.
pub fn crosscheck_speed_extremum<P: LongitudeProvider>(
    provider: &P,
    candidate: &SpeedExtremumCandidate,
    options: SpeedCrosscheckOptions,
    remaining_probes: &mut usize,
) -> Result<SpeedCrosscheckReport, GenerationError> {
    if options.end_us <= options.start_us
        || options.half_span_us <= 0
        || !options.speed_tolerance_degrees_per_day.is_finite()
        || options.speed_tolerance_degrees_per_day < 0.0
        || !options
            .acceleration_noise_floor_degrees_per_day_squared
            .is_finite()
        || options.acceleration_noise_floor_degrees_per_day_squared < 0.0
        || candidate.bracket_start_us > candidate.bracket_end_us
        || candidate.bracket_start_us < options.start_us
        || candidate.bracket_end_us >= options.end_us
        || !candidate.sampled_signed_speed_degrees_per_day.is_finite()
    {
        return Err(GenerationError::InvalidWindow);
    }
    let center = (candidate.bracket_start_us as i128
        + (candidate.bracket_end_us as i128 - candidate.bracket_start_us as i128) / 2)
        as i64;
    let left = center as i128 - options.half_span_us as i128;
    let right = center as i128 + options.half_span_us as i128;
    if left < options.start_us as i128
        || right >= options.end_us as i128
        || left >= candidate.bracket_start_us as i128
        || right <= candidate.bracket_end_us as i128
    {
        return Err(GenerationError::InvalidWindow);
    }

    let mut probe = |at_us: i64| {
        if *remaining_probes == 0 {
            return Err(GenerationError::TooManySteps);
        }
        *remaining_probes -= 1;
        checked_sample(provider, &candidate.body, at_us)
    };
    let before = probe(left as i64)?;
    let middle = probe(center)?;
    let after = probe(right as i64)?;
    let elapsed_days = options.half_span_us as f64 / US_PER_DAY;
    let signed_speed = middle.longitude_speed_degrees_per_day;
    // If apparent longitude could traverse >= 180° over the full symmetric
    // probe span, shortest-arc unwrapping is ambiguous. Restrict the numerical
    // crosscheck rather than silently interpreting multiple possible turns.
    let speed_bound = before
        .longitude_speed_degrees_per_day
        .abs()
        .max(signed_speed.abs())
        .max(after.longitude_speed_degrees_per_day.abs());
    if speed_bound * (2.0 * elapsed_days) >= 180.0 {
        return Err(GenerationError::AmbiguousSpeedExtremum);
    }

    let raw_longitude_delta = after.longitude_degrees - before.longitude_degrees;
    let wrapped_delta = (raw_longitude_delta + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped_delta == -180.0 {
        return Err(GenerationError::AmbiguousSpeedExtremum);
    }
    let longitude_secant_speed = wrapped_delta / (2.0 * elapsed_days);
    let left_acceleration = (signed_speed - before.longitude_speed_degrees_per_day) / elapsed_days;
    let right_acceleration = (after.longitude_speed_degrees_per_day - signed_speed) / elapsed_days;
    let speed_residual = (longitude_secant_speed - signed_speed).abs();
    let candidate_residual = (signed_speed - candidate.sampled_signed_speed_degrees_per_day).abs();
    if !longitude_secant_speed.is_finite()
        || !left_acceleration.is_finite()
        || !right_acceleration.is_finite()
        || !speed_residual.is_finite()
        || !candidate_residual.is_finite()
    {
        return Err(GenerationError::InvalidSample);
    }

    use crate::speed_extrema::SpeedExtremumKind;
    let floor = options.acceleration_noise_floor_degrees_per_day_squared;
    let (supported, contradicted, valid_signed_speed) = match candidate.kind {
        SpeedExtremumKind::DirectMaximum => (
            left_acceleration > floor && right_acceleration < -floor,
            left_acceleration < -floor || right_acceleration > floor,
            signed_speed > 0.0,
        ),
        SpeedExtremumKind::RetrogradeMagnitudeMaximum => (
            left_acceleration < -floor && right_acceleration > floor,
            left_acceleration > floor || right_acceleration < -floor,
            signed_speed < 0.0,
        ),
    };
    let status = if !valid_signed_speed
        || speed_residual > options.speed_tolerance_degrees_per_day
        || candidate_residual > options.speed_tolerance_degrees_per_day
        || contradicted
    {
        SpeedCrosscheckStatus::ContradictsCandidate
    } else if supported {
        SpeedCrosscheckStatus::SupportsCandidate
    } else {
        SpeedCrosscheckStatus::Inconclusive
    };

    Ok(SpeedCrosscheckReport {
        status,
        center_us: center,
        left_acceleration_degrees_per_day_squared: left_acceleration,
        right_acceleration_degrees_per_day_squared: right_acceleration,
        longitude_secant_speed_degrees_per_day: longitude_secant_speed,
        provider_center_speed_degrees_per_day: signed_speed,
        speed_consistency_residual_degrees_per_day: speed_residual,
        candidate_speed_residual_degrees_per_day: candidate_residual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{speed_extrema::SpeedExtremumKind, LongitudeSample};
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct ParabolicSpeed {
        retrograde: bool,
        dishonest_longitude: bool,
        wrap_longitude: bool,
    }

    impl LongitudeProvider for ParabolicSpeed {
        fn sample(
            &self,
            _: &virgostars_events_core::BodyId,
            at: TimestampMicros,
        ) -> Result<LongitudeSample, String> {
            let day = at.as_i64() as f64 / DAY as f64;
            let shifted = day - 2.0;
            let sign = if self.retrograde { -1.0 } else { 1.0 };
            let longitude = sign * (5.0 * day - shifted.powi(3) / 3.0)
                + if self.wrap_longitude { 10.0 } else { 0.0 };
            Ok(LongitudeSample {
                longitude_degrees: if self.dishonest_longitude {
                    0.0
                } else if self.wrap_longitude {
                    longitude.rem_euclid(360.0)
                } else {
                    longitude
                },
                longitude_speed_degrees_per_day: sign * (5.0 - shifted * shifted),
            })
        }
    }

    fn candidate(kind: SpeedExtremumKind, speed: f64) -> SpeedExtremumCandidate {
        SpeedExtremumCandidate {
            body: virgostars_events_core::BodyId::try_from("Mars").unwrap(),
            kind,
            bracket_start_us: 2 * DAY - 250_000,
            bracket_end_us: 2 * DAY + 250_000,
            sampled_signed_speed_degrees_per_day: speed,
        }
    }

    fn options() -> SpeedCrosscheckOptions {
        SpeedCrosscheckOptions {
            start_us: 0,
            end_us: 5 * DAY,
            half_span_us: DAY / 8,
            speed_tolerance_degrees_per_day: 0.01,
            acceleration_noise_floor_degrees_per_day_squared: 0.0001,
        }
    }

    #[test]
    fn supports_direct_maximum_with_consistent_speed_and_acceleration() {
        let mut budget = 3;
        let report = crosscheck_speed_extremum(
            &ParabolicSpeed {
                retrograde: false,
                dishonest_longitude: false,
                wrap_longitude: false,
            },
            &candidate(SpeedExtremumKind::DirectMaximum, 5.0),
            options(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget, 0);
        assert_eq!(report.status, SpeedCrosscheckStatus::SupportsCandidate);
        assert!(report.left_acceleration_degrees_per_day_squared > 0.0);
        assert!(report.right_acceleration_degrees_per_day_squared < 0.0);
        assert!(report.speed_consistency_residual_degrees_per_day < 0.01);
    }

    #[test]
    fn supports_retrograde_speed_magnitude_and_wrapped_longitudes() {
        for wrap in [false, true] {
            let mut budget = 3;
            let report = crosscheck_speed_extremum(
                &ParabolicSpeed {
                    retrograde: true,
                    dishonest_longitude: false,
                    wrap_longitude: wrap,
                },
                &candidate(SpeedExtremumKind::RetrogradeMagnitudeMaximum, -5.0),
                options(),
                &mut budget,
            )
            .unwrap();
            assert_eq!(report.status, SpeedCrosscheckStatus::SupportsCandidate);
            assert!(report.left_acceleration_degrees_per_day_squared < 0.0);
            assert!(report.right_acceleration_degrees_per_day_squared > 0.0);
        }
    }

    #[test]
    fn generator_candidate_can_be_checked_without_archive_or_database() {
        use crate::{speed_extrema::speed_extremum_candidates, SearchWindow};
        let provider = ParabolicSpeed {
            retrograde: false,
            dishonest_longitude: false,
            wrap_longitude: false,
        };
        let mut budget = 250;
        let proposed = speed_extremum_candidates(
            &provider,
            &virgostars_events_core::BodyId::try_from("Mars").unwrap(),
            SearchWindow {
                start_us: 0,
                end_us: 5 * DAY,
                step_us: DAY,
                precision_us: 1_000_000,
            },
            &mut budget,
        )
        .unwrap();
        assert_eq!(proposed.len(), 1);
        let report =
            crosscheck_speed_extremum(&provider, &proposed[0], options(), &mut budget).unwrap();
        assert_eq!(report.status, SpeedCrosscheckStatus::SupportsCandidate);
    }

    #[test]
    fn detects_provider_longitude_speed_inconsistency_or_stale_candidate() {
        for (dishonest, recorded_speed) in [(true, 5.0), (false, 4.0)] {
            let mut budget = 3;
            let report = crosscheck_speed_extremum(
                &ParabolicSpeed {
                    retrograde: false,
                    dishonest_longitude: dishonest,
                    wrap_longitude: false,
                },
                &candidate(SpeedExtremumKind::DirectMaximum, recorded_speed),
                options(),
                &mut budget,
            )
            .unwrap();
            assert_eq!(report.status, SpeedCrosscheckStatus::ContradictsCandidate);
        }
    }

    #[test]
    fn incorrect_extremum_kind_is_contradicted() {
        let mut budget = 3;
        let report = crosscheck_speed_extremum(
            &ParabolicSpeed {
                retrograde: false,
                dishonest_longitude: false,
                wrap_longitude: false,
            },
            &candidate(SpeedExtremumKind::RetrogradeMagnitudeMaximum, 5.0),
            options(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.status, SpeedCrosscheckStatus::ContradictsCandidate);
    }

    #[test]
    fn flat_acceleration_is_inconclusive_not_supported() {
        struct Constant;
        impl LongitudeProvider for Constant {
            fn sample(
                &self,
                _: &virgostars_events_core::BodyId,
                at: TimestampMicros,
            ) -> Result<LongitudeSample, String> {
                let day = at.as_i64() as f64 / DAY as f64;
                Ok(LongitudeSample {
                    longitude_degrees: day,
                    longitude_speed_degrees_per_day: 1.0,
                })
            }
        }
        let mut budget = 3;
        let report = crosscheck_speed_extremum(
            &Constant,
            &candidate(SpeedExtremumKind::DirectMaximum, 1.0),
            options(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.status, SpeedCrosscheckStatus::Inconclusive);
    }

    #[test]
    fn monotone_speed_contradicts_fabricated_maximum() {
        struct RisingSpeed;
        impl LongitudeProvider for RisingSpeed {
            fn sample(
                &self,
                _: &virgostars_events_core::BodyId,
                at: TimestampMicros,
            ) -> Result<LongitudeSample, String> {
                let day = at.as_i64() as f64 / DAY as f64;
                Ok(LongitudeSample {
                    longitude_degrees: 3.0 * day + day * day / 2.0,
                    longitude_speed_degrees_per_day: 3.0 + day,
                })
            }
        }
        let mut budget = 3;
        let report = crosscheck_speed_extremum(
            &RisingSpeed,
            &candidate(SpeedExtremumKind::DirectMaximum, 5.0),
            options(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.status, SpeedCrosscheckStatus::ContradictsCandidate);
        assert!(report.right_acceleration_degrees_per_day_squared > 0.0);
    }

    #[test]
    fn unresolvable_longitude_turn_and_nonfinite_sample_fail_closed() {
        struct Fast;
        impl LongitudeProvider for Fast {
            fn sample(
                &self,
                _: &virgostars_events_core::BodyId,
                at: TimestampMicros,
            ) -> Result<LongitudeSample, String> {
                let day = at.as_i64() as f64 / DAY as f64;
                Ok(LongitudeSample {
                    longitude_degrees: 200.0 * day,
                    longitude_speed_degrees_per_day: 200.0,
                })
            }
        }
        let mut budget = 3;
        let mut wide = options();
        wide.half_span_us = DAY / 2;
        assert_eq!(
            crosscheck_speed_extremum(
                &Fast,
                &candidate(SpeedExtremumKind::DirectMaximum, 200.0),
                wide,
                &mut budget,
            )
            .unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum
        );

        struct NonFinite;
        impl LongitudeProvider for NonFinite {
            fn sample(
                &self,
                _: &virgostars_events_core::BodyId,
                _: TimestampMicros,
            ) -> Result<LongitudeSample, String> {
                Ok(LongitudeSample {
                    longitude_degrees: f64::INFINITY,
                    longitude_speed_degrees_per_day: 5.0,
                })
            }
        }
        let mut budget = 3;
        assert_eq!(
            crosscheck_speed_extremum(
                &NonFinite,
                &candidate(SpeedExtremumKind::DirectMaximum, 5.0),
                options(),
                &mut budget,
            )
            .unwrap_err(),
            GenerationError::InvalidSample
        );
    }

    #[test]
    fn invalid_edges_tolerance_and_budget_fail_closed() {
        let provider = ParabolicSpeed {
            retrograde: false,
            dishonest_longitude: false,
            wrap_longitude: false,
        };
        let test_candidate = candidate(SpeedExtremumKind::DirectMaximum, 5.0);
        let mut invalid = options();
        invalid.half_span_us = 100;
        let mut budget = 10;
        assert_eq!(
            crosscheck_speed_extremum(&provider, &test_candidate, invalid, &mut budget)
                .unwrap_err(),
            GenerationError::InvalidWindow
        );
        invalid = options();
        invalid.speed_tolerance_degrees_per_day = f64::NAN;
        assert_eq!(
            crosscheck_speed_extremum(&provider, &test_candidate, invalid, &mut budget)
                .unwrap_err(),
            GenerationError::InvalidWindow
        );
        invalid = options();
        invalid.end_us = 2 * DAY;
        assert_eq!(
            crosscheck_speed_extremum(&provider, &test_candidate, invalid, &mut budget)
                .unwrap_err(),
            GenerationError::InvalidWindow
        );
        let mut budget = 2;
        assert_eq!(
            crosscheck_speed_extremum(&provider, &test_candidate, options(), &mut budget)
                .unwrap_err(),
            GenerationError::TooManySteps
        );
    }
}
