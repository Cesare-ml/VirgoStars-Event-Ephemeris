/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Unverified local extrema of apparent geocentric tropical longitude speed.
//!
//! Searches only a caller-supplied provider for *candidate* direct maxima and
//! retrograde speed-magnitude maxima. This module never writes an archive or
//! certifies that unsampled peaks do not exist. Independently review real
//! timestamps, providers, time scales, kernels and temporal completeness.

use virgostars_events_core::BodyId;

use crate::{
    checked_sample, midpoint, needs_refinement, next_time, GenerationError, LongitudeProvider,
    SearchWindow,
};

/// `DirectMaximum` means a positive local maximum of signed dλ/dt.
/// `RetrogradeMagnitudeMaximum` means a negative local minimum of dλ/dt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SpeedExtremumKind {
    DirectMaximum,
    RetrogradeMagnitudeMaximum,
}

/// A candidate time bracket plus the signed speed sampled near its midpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedExtremumCandidate {
    pub body: BodyId,
    pub kind: SpeedExtremumKind,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    /// Apparent geocentric ecliptic longitude speed in degrees/day (not km/s).
    pub sampled_signed_speed_degrees_per_day: f64,
}

fn checked_speed<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    at_us: i64,
    remaining_probes: &mut usize,
) -> Result<f64, GenerationError> {
    if *remaining_probes == 0 {
        return Err(GenerationError::TooManySteps);
    }
    *remaining_probes -= 1;
    Ok(checked_sample(provider, body, at_us)?.longitude_speed_degrees_per_day)
}

fn measured_local_peak<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    first_us: i64,
    last_us: i64,
    precision_us: i64,
    kind: SpeedExtremumKind,
    remaining_probes: &mut usize,
) -> Result<SpeedExtremumCandidate, GenerationError> {
    let mut left = first_us;
    let mut right = last_us;
    // Integer rational fractions avoid precision loss with large i64 epochs.
    // This narrows a *presumed unimodal* sampled speed peak; it is not a
    // guarantee of unimodality between sample points.
    while needs_refinement(left, right, precision_us) {
        let width = right as i128 - left as i128;
        let first = (left as i128 + width * 382 / 1000) as i64;
        let second = (left as i128 + width * 618 / 1000) as i64;
        if first <= left || second >= right || first >= second {
            break;
        }
        let v1 = checked_speed(provider, body, first, remaining_probes)?;
        let v2 = checked_speed(provider, body, second, remaining_probes)?;
        let move_right = match kind {
            SpeedExtremumKind::DirectMaximum => v1 < v2,
            SpeedExtremumKind::RetrogradeMagnitudeMaximum => v1 > v2,
        };
        if move_right {
            left = first;
        } else {
            right = second;
        }
    }
    let speed = checked_speed(provider, body, midpoint(left, right), remaining_probes)?;
    if (kind == SpeedExtremumKind::DirectMaximum && speed <= 0.0)
        || (kind == SpeedExtremumKind::RetrogradeMagnitudeMaximum && speed >= 0.0)
    {
        return Err(GenerationError::AmbiguousSpeedExtremum);
    }
    Ok(SpeedExtremumCandidate {
        body: body.clone(),
        kind,
        bracket_start_us: left,
        bracket_end_us: right,
        sampled_signed_speed_degrees_per_day: speed,
    })
}

/// Discover *possible* local maximum direct speeds and strongest retrograde
/// speeds from consecutive three-point samples.
///
/// A strict sign change of sampled speed **slopes** brackets a peak. Refinement
/// uses bounded unimodal optimization, counted against `remaining_probes`.
/// Adjacent equal speeds, a search interval with fewer than three time points,
/// and budget exhaustion fail closed. Multiple hidden extrema within one
/// coarse step may still be missed; an empty result is never a completeness
/// certificate. Time windows are half-open `[start_us, end_us)`.
pub fn speed_extremum_candidates<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    remaining_probes: &mut usize,
) -> Result<Vec<SpeedExtremumCandidate>, GenerationError> {
    window.validate()?;
    if *remaining_probes == 0 {
        return Err(GenerationError::TooManySteps);
    }
    let mut first_us = window.start_us;
    let mut first_speed = checked_speed(provider, body, first_us, remaining_probes)?;
    let mut center_us = next_time(first_us, window);
    if center_us >= window.end_us {
        return Err(GenerationError::InsufficientSpeedSamples);
    }
    let mut center_speed = checked_speed(provider, body, center_us, remaining_probes)?;
    let mut found = Vec::new();
    while center_us < window.end_us {
        let last_us = next_time(center_us, window);
        let last_speed = checked_speed(provider, body, last_us, remaining_probes)?;
        if first_speed == center_speed || center_speed == last_speed {
            return Err(GenerationError::AmbiguousSpeedExtremum);
        }
        let kind = if first_speed < center_speed && center_speed > last_speed && center_speed > 0.0
        {
            Some(SpeedExtremumKind::DirectMaximum)
        } else if first_speed > center_speed && center_speed < last_speed && center_speed < 0.0 {
            Some(SpeedExtremumKind::RetrogradeMagnitudeMaximum)
        } else {
            None
        };
        if let Some(kind) = kind {
            found.push(measured_local_peak(
                provider,
                body,
                first_us,
                last_us,
                window.precision_us,
                kind,
                remaining_probes,
            )?);
        }
        first_us = center_us;
        first_speed = center_speed;
        center_us = last_us;
        center_speed = last_speed;
    }
    found.sort_by_key(|candidate| candidate.bracket_start_us);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LongitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct QuadraticSpeed {
        retrograde: bool,
    }

    impl LongitudeProvider for QuadraticSpeed {
        fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let day = at.as_i64() as f64 / DAY as f64;
            let shifted = day - 2.0;
            let (longitude_degrees, longitude_speed_degrees_per_day) = if self.retrograde {
                (-5.0 * day + shifted.powi(3) / 3.0, -5.0 + shifted * shifted)
            } else {
                (5.0 * day - shifted.powi(3) / 3.0, 5.0 - shifted * shifted)
            };
            Ok(LongitudeSample {
                longitude_degrees,
                longitude_speed_degrees_per_day,
            })
        }
    }

    fn body() -> BodyId {
        BodyId::try_from("Mars").unwrap()
    }

    fn window(days: i64) -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: days * DAY,
            step_us: DAY,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn brackets_positive_direct_speed_peak() {
        let mut budget = 200;
        let candidates = speed_extremum_candidates(
            &QuadraticSpeed { retrograde: false },
            &body(),
            window(5),
            &mut budget,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, SpeedExtremumKind::DirectMaximum);
        assert!(candidates[0].bracket_start_us <= 2 * DAY);
        assert!(candidates[0].bracket_end_us >= 2 * DAY);
        assert!(candidates[0].bracket_end_us - candidates[0].bracket_start_us <= 1_000_000);
        assert!((candidates[0].sampled_signed_speed_degrees_per_day - 5.0).abs() < 1e-8);
    }

    #[test]
    fn brackets_largest_retrograde_speed_magnitude_as_signed_minimum() {
        let mut budget = 200;
        let candidates = speed_extremum_candidates(
            &QuadraticSpeed { retrograde: true },
            &body(),
            window(5),
            &mut budget,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].kind,
            SpeedExtremumKind::RetrogradeMagnitudeMaximum
        );
        assert!(candidates[0].bracket_start_us <= 2 * DAY);
        assert!(candidates[0].bracket_end_us >= 2 * DAY);
        assert!((candidates[0].sampled_signed_speed_degrees_per_day + 5.0).abs() < 1e-8);
    }

    struct MultipleMaxima;

    impl LongitudeProvider for MultipleMaxima {
        fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let day = at.as_i64() as f64 / DAY as f64;
            let angle = day * std::f64::consts::FRAC_PI_2;
            Ok(LongitudeSample {
                longitude_degrees: 2.0 * day - angle.cos() / std::f64::consts::FRAC_PI_2,
                longitude_speed_degrees_per_day: 2.0 + angle.sin(),
            })
        }
    }

    #[test]
    fn multiple_local_maxima_are_kept_in_chronological_order() {
        let mut budget = 400;
        let mut query = window(8);
        query.step_us = DAY / 2;
        let found =
            speed_extremum_candidates(&MultipleMaxima, &body(), query, &mut budget).unwrap();
        assert_eq!(found.len(), 2);
        for (candidate, expected) in found.iter().zip([DAY, 5 * DAY]) {
            assert_eq!(candidate.kind, SpeedExtremumKind::DirectMaximum);
            assert!(candidate.bracket_start_us <= expected);
            assert!(candidate.bracket_end_us >= expected);
        }
    }

    struct ConstantSpeed;

    impl LongitudeProvider for ConstantSpeed {
        fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            Ok(LongitudeSample {
                longitude_degrees: at.as_i64() as f64 / DAY as f64,
                longitude_speed_degrees_per_day: 1.0,
            })
        }
    }

    #[test]
    fn flat_plateau_and_insufficient_samples_fail_closed() {
        let mut budget = 100;
        assert_eq!(
            speed_extremum_candidates(&ConstantSpeed, &body(), window(3), &mut budget).unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum
        );
        let mut budget = 100;
        assert_eq!(
            speed_extremum_candidates(&ConstantSpeed, &body(), window(1), &mut budget).unwrap_err(),
            GenerationError::InsufficientSpeedSamples
        );
    }

    #[test]
    fn low_probe_budget_is_error_not_partial_event_list() {
        let mut budget = 3;
        assert_eq!(
            speed_extremum_candidates(
                &QuadraticSpeed { retrograde: false },
                &body(),
                window(5),
                &mut budget
            )
            .unwrap_err(),
            GenerationError::TooManySteps
        );
    }

    #[test]
    fn exclusive_end_does_not_report_unbracketed_peak() {
        let mut budget = 100;
        let found = speed_extremum_candidates(
            &QuadraticSpeed { retrograde: false },
            &body(),
            window(2),
            &mut budget,
        )
        .unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn non_finite_provider_speed_is_rejected() {
        struct BadProvider;
        impl LongitudeProvider for BadProvider {
            fn sample(&self, _: &BodyId, _: TimestampMicros) -> Result<LongitudeSample, String> {
                Ok(LongitudeSample {
                    longitude_degrees: 10.0,
                    longitude_speed_degrees_per_day: f64::NAN,
                })
            }
        }
        let mut budget = 20;
        assert_eq!(
            speed_extremum_candidates(&BadProvider, &body(), window(4), &mut budget).unwrap_err(),
            GenerationError::InvalidSample
        );
    }
}
