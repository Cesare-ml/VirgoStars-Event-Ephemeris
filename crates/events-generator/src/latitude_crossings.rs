/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Experimental geocentric ecliptic-latitude plane crossings and speed stations.
//!
//! These generator-only algorithms find *candidates* for sign-changing roots
//! of latitude beta and signed latitude speed d(beta)/dt respectively. No
//! astronomical accuracy, missed-crossing completeness or UTC coverage is
//! certified by a successful (including empty) scan. A single coarse step
//! can conceal two roots; independently verify real planetary timestamps.

use virgostars_events_core::BodyId;

use crate::{
    midpoint, needs_refinement, next_time, GenerationError, LatitudeProvider, LatitudeSample,
    SearchWindow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EclipticPlaneDirection {
    /// From negative (south) to positive (north) ecliptic latitude.
    Ascending,
    /// From positive (north) to negative (south) ecliptic latitude.
    Descending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatitudeMotionDirection {
    Northward,
    Southward,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EclipticPlaneCrossingCandidate {
    pub body: BodyId,
    /// Inclusive time bracket containing the observed zero, not a certified UTC instant.
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    pub direction: EclipticPlaneDirection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatitudeStationCandidate {
    pub body: BodyId,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    /// The direction *after* beta speed crosses zero. Stop and restart share
    /// this same mathematical event; no later timestamp is fabricated.
    pub direction_after: LatitudeMotionDirection,
}

#[derive(Clone, Copy)]
enum RootOf {
    Latitude,
    LatitudeSpeed,
}

impl RootOf {
    fn scalar(self, sample: LatitudeSample) -> f64 {
        match self {
            Self::Latitude => sample.latitude_degrees,
            Self::LatitudeSpeed => sample.latitude_speed_degrees_per_day,
        }
    }
}

#[derive(Clone, Copy)]
struct SignedZeroBracket {
    start_us: i64,
    end_us: i64,
    positive_after: bool,
}

fn checked_value<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    at_us: i64,
    root: RootOf,
    remaining_probes: &mut usize,
) -> Result<f64, GenerationError> {
    if *remaining_probes == 0 {
        return Err(GenerationError::TooManySteps);
    }
    *remaining_probes -= 1;
    let sample = provider
        .sample_latitude(body, virgostars_events_core::TimestampMicros::new(at_us))
        .map_err(GenerationError::Provider)?;
    if !sample.latitude_degrees.is_finite()
        || !sample.latitude_speed_degrees_per_day.is_finite()
        || sample.latitude_degrees.abs() > 90.0
    {
        return Err(GenerationError::InvalidSample);
    }
    Ok(root.scalar(sample))
}

/// Validated signed ecliptic-latitude speed, sharing the caller's probe budget.
/// Kept in one place so station/plane-crossing and speed-peak searches reject
/// the same malformed or physically impossible latitude samples.
pub(crate) fn checked_latitude_speed<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    at_us: i64,
    remaining_probes: &mut usize,
) -> Result<f64, GenerationError> {
    checked_value(
        provider,
        body,
        at_us,
        RootOf::LatitudeSpeed,
        remaining_probes,
    )
}

fn refine_opposite_signs<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    bracket: (i64, i64),
    sign_before: bool,
    root: RootOf,
    precision_us: i64,
    remaining_probes: &mut usize,
) -> Result<(i64, i64), GenerationError> {
    let (mut left, mut right) = bracket;
    while needs_refinement(left, right, precision_us) {
        let mid = midpoint(left, right);
        if mid <= left || mid >= right {
            break;
        }
        let value = checked_value(provider, body, mid, root, remaining_probes)?;
        if value == 0.0 {
            // Exact synthetic or provider-reported zero; a singleton bracket
            // is valid, but its numerical/scientific accuracy is not proven.
            return Ok((mid, mid));
        }
        if value.is_sign_positive() == sign_before {
            left = mid;
        } else {
            right = mid;
        }
    }
    Ok((left, right))
}

fn signed_zero_candidates<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    root: RootOf,
    remaining_probes: &mut usize,
) -> Result<Vec<SignedZeroBracket>, GenerationError> {
    window.validate()?;
    let mut result = Vec::new();
    let mut t = window.start_us;
    let mut before = checked_value(provider, body, t, root, remaining_probes)?;
    if before == 0.0 {
        // Without a left-hand sample, a zero at the inclusive window start
        // could be a tangent, a crossing or a finite zero plateau.
        return Err(GenerationError::AmbiguousLatitudeCrossing);
    }
    let mut pending_zero: Option<(i64, bool)> = None;

    while t < window.end_us {
        let end = next_time(t, window);
        let after = checked_value(provider, body, end, root, remaining_probes)?;
        if before == 0.0 && after == 0.0 {
            // Two consecutive sampled zeros cannot establish a unique event.
            return Err(GenerationError::AmbiguousLatitudeCrossing);
        }
        if after == 0.0 {
            if end < window.end_us {
                pending_zero = Some((end, before.is_sign_positive()));
            }
            // A zero exactly at the exclusive end is intentionally omitted.
        } else if before == 0.0 {
            let Some((zero_at, positive_before)) = pending_zero.take() else {
                return Err(GenerationError::AmbiguousLatitudeCrossing);
            };
            if positive_before != after.is_sign_positive() {
                result.push(SignedZeroBracket {
                    start_us: zero_at,
                    end_us: zero_at,
                    positive_after: after.is_sign_positive(),
                });
            }
            // An exact tangent with the same sign on both sides is not a crossing.
        } else if before.is_sign_positive() != after.is_sign_positive() {
            let (left, right) = refine_opposite_signs(
                provider,
                body,
                (t, end),
                before.is_sign_positive(),
                root,
                window.precision_us,
                remaining_probes,
            )?;
            result.push(SignedZeroBracket {
                start_us: left,
                end_us: right,
                positive_after: after.is_sign_positive(),
            });
        }
        t = end;
        before = after;
    }
    Ok(result)
}

/// Locate *candidate* crossings of the configured geocentric ecliptic plane.
///
/// Ascending means beta negative -> positive, descending means the reverse.
/// A sampled zero is accepted only when the signs on either side differ;
/// tangencies are ignored, zero plateaus and zero at the start fail closed,
/// and a zero at the exclusive end is ignored. The scan is not exhaustive:
/// two unsampled roots in one step can cancel. The shared provider budget
/// covers the initial grid and subsequent bracket refinement.
pub fn ecliptic_plane_crossing_candidates<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    remaining_probes: &mut usize,
) -> Result<Vec<EclipticPlaneCrossingCandidate>, GenerationError> {
    signed_zero_candidates(provider, body, window, RootOf::Latitude, remaining_probes).map(
        |roots| {
            roots
                .into_iter()
                .map(|root| EclipticPlaneCrossingCandidate {
                    body: body.clone(),
                    bracket_start_us: root.start_us,
                    bracket_end_us: root.end_us,
                    direction: if root.positive_after {
                        EclipticPlaneDirection::Ascending
                    } else {
                        EclipticPlaneDirection::Descending
                    },
                })
                .collect()
        },
    )
}

/// Locate *candidate* zeros of signed latitude speed with reversal.
///
/// The same timestamp is both the mathematical stop and the resumption in
/// the `direction_after` direction; no artificial restart is created. An
/// optional perceptual stationary *duration* is not defined by this API.
pub fn latitude_station_candidates<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    remaining_probes: &mut usize,
) -> Result<Vec<LatitudeStationCandidate>, GenerationError> {
    signed_zero_candidates(
        provider,
        body,
        window,
        RootOf::LatitudeSpeed,
        remaining_probes,
    )
    .map(|roots| {
        roots
            .into_iter()
            .map(|root| LatitudeStationCandidate {
                body: body.clone(),
                bracket_start_us: root.start_us,
                bracket_end_us: root.end_us,
                direction_after: if root.positive_after {
                    LatitudeMotionDirection::Northward
                } else {
                    LatitudeMotionDirection::Southward
                },
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    fn body() -> BodyId {
        BodyId::try_from("Mars").unwrap()
    }

    fn window(start: i64, end: i64, step: i64) -> SearchWindow {
        SearchWindow {
            start_us: start,
            end_us: end,
            step_us: step,
            precision_us: 1_000_000,
        }
    }

    struct Model<F>(F);
    impl<F: Fn(f64) -> (f64, f64)> LatitudeProvider for Model<F> {
        fn sample_latitude(
            &self,
            _body: &BodyId,
            at: TimestampMicros,
        ) -> Result<LatitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            let (latitude, speed) = (self.0)(days);
            Ok(LatitudeSample {
                latitude_degrees: latitude,
                latitude_speed_degrees_per_day: speed,
            })
        }
    }

    #[test]
    fn exact_sampled_ascending_and_descending_plane_crossings() {
        for (slope, expected) in [
            (1.0, EclipticPlaneDirection::Ascending),
            (-1.0, EclipticPlaneDirection::Descending),
        ] {
            let model = Model(move |d: f64| (slope * (d - 1.0), slope));
            let mut budget = 20;
            let events = ecliptic_plane_crossing_candidates(
                &model,
                &body(),
                window(0, 3 * DAY, DAY),
                &mut budget,
            )
            .unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].direction, expected);
            assert_eq!(
                (events[0].bracket_start_us, events[0].bracket_end_us),
                (DAY, DAY)
            );
        }
    }

    #[test]
    fn sampled_speed_zero_classifies_both_station_directions() {
        for (sign, expected) in [
            (1.0, LatitudeMotionDirection::Southward),
            (-1.0, LatitudeMotionDirection::Northward),
        ] {
            let model = Model(move |d: f64| (sign * -(d - 1.0).powi(2), sign * -2.0 * (d - 1.0)));
            let mut budget = 20;
            let stations =
                latitude_station_candidates(&model, &body(), window(0, 3 * DAY, DAY), &mut budget)
                    .unwrap();
            assert_eq!(stations.len(), 1);
            assert_eq!(stations[0].direction_after, expected);
            assert_eq!(
                (stations[0].bracket_start_us, stations[0].bracket_end_us),
                (DAY, DAY)
            );
        }
    }

    #[test]
    fn nonsampled_root_is_refined_inside_one_second_bracket() {
        let model = Model(|d: f64| (d - 0.65, 1.0));
        let mut budget = 100;
        let events = ecliptic_plane_crossing_candidates(
            &model,
            &body(),
            window(0, 2 * DAY, DAY),
            &mut budget,
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let root = DAY * 65 / 100;
        assert!(events[0].bracket_start_us <= root && root <= events[0].bracket_end_us);
        assert!(events[0].bracket_end_us - events[0].bracket_start_us <= 1_000_000);
    }

    #[test]
    fn tangency_at_sample_is_not_an_ecliptic_plane_crossing() {
        let model = Model(|d: f64| ((d - 1.0).powi(2), 2.0 * (d - 1.0)));
        let mut budget = 20;
        let crossings = ecliptic_plane_crossing_candidates(
            &model,
            &body(),
            window(0, 3 * DAY, DAY),
            &mut budget,
        )
        .unwrap();
        assert!(crossings.is_empty());
        let mut budget = 20;
        let station =
            latitude_station_candidates(&model, &body(), window(0, 3 * DAY, DAY), &mut budget)
                .unwrap();
        assert_eq!(station.len(), 1);
        assert_eq!(
            station[0].direction_after,
            LatitudeMotionDirection::Northward
        );
    }

    #[test]
    fn zero_latitude_speed_without_sign_reversal_is_not_a_station() {
        let model = Model(|d: f64| ((d - 1.0).powi(3) / 3.0, (d - 1.0).powi(2)));
        let mut budget = 20;
        let stations =
            latitude_station_candidates(&model, &body(), window(0, 3 * DAY, DAY), &mut budget)
                .unwrap();
        assert!(stations.is_empty());
    }

    #[test]
    fn exact_exclusive_end_zero_is_not_reported() {
        let model = Model(|d: f64| (d - 2.0, 1.0));
        let mut budget = 20;
        let result = ecliptic_plane_crossing_candidates(
            &model,
            &body(),
            window(0, 2 * DAY, DAY),
            &mut budget,
        )
        .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn zero_at_inclusive_start_requires_pre_start_direction_context() {
        let model = Model(|d: f64| (d, 1.0));
        let mut budget = 20;
        assert_eq!(
            ecliptic_plane_crossing_candidates(
                &model,
                &body(),
                window(0, 2 * DAY, DAY),
                &mut budget,
            ),
            Err(GenerationError::AmbiguousLatitudeCrossing)
        );
    }

    #[test]
    fn zero_plateau_fails_instead_of_inventing_a_crossing() {
        let model = Model(|d: f64| {
            (
                if (1.0..=2.0).contains(&d) {
                    0.0
                } else {
                    d - 1.0
                },
                1.0,
            )
        });
        let mut budget = 20;
        assert_eq!(
            ecliptic_plane_crossing_candidates(
                &model,
                &body(),
                window(0, 4 * DAY, DAY),
                &mut budget,
            ),
            Err(GenerationError::AmbiguousLatitudeCrossing)
        );
    }

    #[test]
    fn exhausted_probes_fail_closed() {
        let model = Model(|d: f64| (d - 0.63, 1.0));
        let mut budget = 2;
        assert_eq!(
            ecliptic_plane_crossing_candidates(
                &model,
                &body(),
                window(0, 2 * DAY, DAY),
                &mut budget,
            ),
            Err(GenerationError::TooManySteps)
        );
    }

    #[test]
    fn invalid_position_or_speed_is_refused_even_for_other_quantity() {
        for (latitude, speed) in [(f64::NAN, 1.0), (0.1, f64::INFINITY), (91.0, 1.0)] {
            let model = Model(move |_d: f64| (latitude, speed));
            let mut budget = 10;
            assert_eq!(
                ecliptic_plane_crossing_candidates(
                    &model,
                    &body(),
                    window(0, DAY, DAY),
                    &mut budget,
                ),
                Err(GenerationError::InvalidSample)
            );
        }
    }

    #[test]
    fn same_sign_coarse_endpoints_can_hide_two_crossings() {
        let model = Model(|d: f64| ((d - 0.25) * (d - 0.75), 2.0 * d - 1.0));
        let mut budget = 20;
        let roots =
            ecliptic_plane_crossing_candidates(&model, &body(), window(0, DAY, DAY), &mut budget)
                .unwrap();
        // This is an explicit limitation, NOT evidence that the interval is complete.
        assert!(roots.is_empty());
    }
}
