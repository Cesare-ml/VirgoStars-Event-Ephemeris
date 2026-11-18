/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Experimental, provider-neutral event *candidate* discovery.
//!
//! This crate never reads/writes the owner archive, SQL tables or gaps. It
//! scans finite longitude/speed (and, separately, ecliptic-latitude/speed)
//! samples supplied by the caller and brackets candidate stations, zodiac
//! ingresses and ecliptic-plane crossings. It is NOT a complete astronomy
//! generator: unsampled multiple crossings can be missed, a candidate is not
//! a verified source record, and source IDs must never be invented here.
//!
//! Swiss Ephemeris can be used only through an independent, license-reviewed
//! development adapter. A future VirgoStars-Ephemeris adapter implements the
//! same trait without changing the reader or its mobile/desktop consumers.

use std::error::Error;
use std::fmt;
use virgostars_events_core::{BodyId, MotionDirection, TimestampMicros, ZodiacSign};

pub mod adaptive;
pub mod gaps;
pub mod latitude_crossings;
pub mod lunar;
pub mod provider_process;
pub mod speed_crosscheck;
pub mod speed_extrema;
pub mod speed_provider_compare;

const MAX_SCAN_STEPS: i128 = 100_000;

/// Geocentric tropical ecliptic longitude and instantaneous longitude speed.
/// Provider implementations must document ephemeris, time scale and settings.
#[derive(Clone, Copy, Debug)]
pub struct LongitudeSample {
    pub longitude_degrees: f64,
    pub longitude_speed_degrees_per_day: f64,
}

/// Replaceable *development-time* position source, never part of the reader.
pub trait LongitudeProvider {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String>;
}

/// Signed geocentric ecliptic latitude (north positive) and its time derivative.
///
/// The implementing development-only provider must state whether its plane is
/// mean/true and of date/fixed epoch, apparent corrections, time scales,
/// ephemeris/kernel/version and license. These values must not be inferred from
/// longitude. This trait is never used by the offline P0 consumer.
#[derive(Clone, Copy, Debug)]
pub struct LatitudeSample {
    pub latitude_degrees: f64,
    pub latitude_speed_degrees_per_day: f64,
}

pub trait LatitudeProvider {
    fn sample_latitude(&self, body: &BodyId, at: TimestampMicros)
        -> Result<LatitudeSample, String>;
}

/// UTC/POSIX microsecond interval, fixed scan stride and target bracket size.
#[derive(Clone, Copy, Debug)]
pub struct SearchWindow {
    pub start_us: i64,
    pub end_us: i64,
    pub step_us: i64,
    pub precision_us: i64,
}

impl SearchWindow {
    pub(crate) fn validate(self) -> Result<(), GenerationError> {
        if self.end_us <= self.start_us
            || self.step_us <= 0
            || self.precision_us <= 0
            || self.precision_us > self.step_us
        {
            return Err(GenerationError::InvalidWindow);
        }
        let span = self.end_us as i128 - self.start_us as i128;
        let steps = (span + self.step_us as i128 - 1) / self.step_us as i128;
        if steps > MAX_SCAN_STEPS {
            return Err(GenerationError::TooManySteps);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GenerationError {
    InvalidWindow,
    TooManySteps,
    InvalidSample,
    AmbiguousIngressStep,
    AmbiguousLunarStep,
    AmbiguousSpeedExtremum,
    AmbiguousLatitudeCrossing,
    InsufficientSpeedSamples,
    Provider(String),
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWindow => f.write_str("invalid UTC generation window or precision"),
            Self::TooManySteps => f.write_str("generation window exceeds sample budget"),
            Self::InvalidSample => f.write_str("non-finite ephemeris position or speed"),
            Self::AmbiguousIngressStep => f.write_str("ambiguous ingress; reduce scan step"),
            Self::AmbiguousLunarStep => f.write_str("ambiguous lunar phase; reduce scan step"),
            Self::AmbiguousSpeedExtremum => {
                f.write_str("ambiguous speed extremum; reduce scan step or investigate plateau")
            }
            Self::AmbiguousLatitudeCrossing => f.write_str(
                "ambiguous latitude/latitude-speed zero; insufficient sign context or plateau",
            ),
            Self::InsufficientSpeedSamples => {
                f.write_str("speed extrema search needs at least three sample times")
            }
            Self::Provider(message) => write!(f, "ephemeris provider error: {message}"),
        }
    }
}

impl Error for GenerationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationCandidate {
    pub body: BodyId,
    /// True crossing lies within this inclusive interval, *if* the provider
    /// is continuous and the bracket contains a single zero crossing.
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    pub direction_after: MotionDirection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngressCandidate {
    pub body: BodyId,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    pub sign_after: ZodiacSign,
    pub retrograde: bool,
}

pub(crate) fn checked_sample<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    at_us: i64,
) -> Result<LongitudeSample, GenerationError> {
    let sample = provider
        .sample(body, TimestampMicros::new(at_us))
        .map_err(GenerationError::Provider)?;
    if !sample.longitude_degrees.is_finite() || !sample.longitude_speed_degrees_per_day.is_finite()
    {
        return Err(GenerationError::InvalidSample);
    }
    Ok(sample)
}

pub(crate) fn next_time(t: i64, window: SearchWindow) -> i64 {
    (t as i128 + window.step_us as i128).min(window.end_us as i128) as i64
}

pub(crate) fn midpoint(left: i64, right: i64) -> i64 {
    (left as i128 + (right as i128 - left as i128) / 2) as i64
}

pub(crate) fn needs_refinement(left: i64, right: i64, precision_us: i64) -> bool {
    right as i128 - left as i128 > precision_us as i128
}

/// Detect candidate longitude-speed sign changes (d/r) from sampled brackets.
/// A coarse step can miss two stations; use independently verified cadence
/// and compare with historical source records before accepting coverage.
pub fn station_candidates<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
) -> Result<Vec<StationCandidate>, GenerationError> {
    window.validate()?;
    let mut result = Vec::new();
    let mut t = window.start_us;
    let mut before = checked_sample(provider, body, t)?;
    while t < window.end_us {
        let end = next_time(t, window);
        let after = checked_sample(provider, body, end)?;
        let direction = match (
            before.longitude_speed_degrees_per_day,
            after.longitude_speed_degrees_per_day,
        ) {
            (a, b) if a >= 0.0 && b < 0.0 => Some(MotionDirection::Retrograde),
            (a, b) if a < 0.0 && b >= 0.0 => Some(MotionDirection::Direct),
            _ => None,
        };
        if let Some(direction_after) = direction {
            let mut left = t;
            let mut right = end;
            while needs_refinement(left, right, window.precision_us) {
                let mid = midpoint(left, right);
                let middle = checked_sample(provider, body, mid)?;
                let still_before = match direction_after {
                    MotionDirection::Direct => middle.longitude_speed_degrees_per_day < 0.0,
                    MotionDirection::Retrograde => middle.longitude_speed_degrees_per_day >= 0.0,
                };
                if still_before {
                    left = mid;
                } else {
                    right = mid;
                }
            }
            result.push(StationCandidate {
                body: body.clone(),
                bracket_start_us: left,
                bracket_end_us: right,
                direction_after,
            });
        }
        t = end;
        before = after;
    }
    Ok(result)
}

pub(crate) fn zodiac_sign(longitude: f64) -> u8 {
    (longitude.rem_euclid(360.0) / 30.0).floor() as u8
}

/// Detect possible ingress/re-entry between adjacent samples. Adjacent sign
/// changes are bracketed; larger angular jumps fail closed as ambiguous.
pub fn ingress_candidates<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
) -> Result<Vec<IngressCandidate>, GenerationError> {
    window.validate()?;
    let mut result = Vec::new();
    let mut t = window.start_us;
    let mut before = checked_sample(provider, body, t)?;
    while t < window.end_us {
        let end = next_time(t, window);
        let after = checked_sample(provider, body, end)?;
        let from = zodiac_sign(before.longitude_degrees);
        let to = zodiac_sign(after.longitude_degrees);
        let mut delta = (after.longitude_degrees - before.longitude_degrees).rem_euclid(360.0);
        if delta > 180.0 {
            delta -= 360.0;
        }
        if delta.abs() >= 30.0 {
            return Err(GenerationError::AmbiguousIngressStep);
        }
        if from != to {
            let forward = delta > 0.0;
            let expected = if forward {
                (from + 1) % 12
            } else {
                (from + 11) % 12
            };
            if delta == 0.0 || to != expected {
                return Err(GenerationError::AmbiguousIngressStep);
            }
            let mut left = t;
            let mut right = end;
            while needs_refinement(left, right, window.precision_us) {
                let mid = midpoint(left, right);
                let middle = checked_sample(provider, body, mid)?;
                if zodiac_sign(middle.longitude_degrees) == from {
                    left = mid;
                } else {
                    right = mid;
                }
            }
            result.push(IngressCandidate {
                body: body.clone(),
                bracket_start_us: left,
                bracket_end_us: right,
                sign_after: ZodiacSign::try_from(to).expect("validated 0..11 sign"),
                retrograde: !forward,
            });
        }
        t = end;
        before = after;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const DAY: i64 = 86_400_000_000;

    struct Linear {
        start: f64,
        rate: f64,
    }
    impl LongitudeProvider for Linear {
        fn sample(&self, _body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: self.start + days * self.rate,
                longitude_speed_degrees_per_day: self.rate,
            })
        }
    }

    struct Turn;
    impl LongitudeProvider for Turn {
        fn sample(&self, _body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: 20.0 + (days - 1.0).powi(2),
                longitude_speed_degrees_per_day: 2.0 * (days - 1.0),
            })
        }
    }

    fn body() -> BodyId {
        BodyId::try_from("Mercury").unwrap()
    }
    fn window() -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: 2 * DAY,
            step_us: DAY,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn station_is_bounded_by_true_synthetic_zero() {
        let stations = station_candidates(&Turn, &body(), window()).unwrap();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].direction_after, MotionDirection::Direct);
        assert!(stations[0].bracket_start_us <= DAY);
        assert!(stations[0].bracket_end_us >= DAY);
        assert!(stations[0].bracket_end_us - stations[0].bracket_start_us <= 1_000_000);
    }

    #[test]
    fn forward_ingress_and_retrograde_reentry() {
        let forward = ingress_candidates(
            &Linear {
                start: 29.0,
                rate: 2.0,
            },
            &body(),
            window(),
        )
        .unwrap();
        assert_eq!(forward.len(), 1);
        assert_eq!(forward[0].sign_after.index(), 1);
        assert!(!forward[0].retrograde);
        assert!(forward[0].bracket_start_us <= DAY / 2);
        assert!(forward[0].bracket_end_us >= DAY / 2);

        let retro = ingress_candidates(
            &Linear {
                start: 0.5,
                rate: -1.0,
            },
            &body(),
            window(),
        )
        .unwrap();
        assert_eq!(retro.len(), 1);
        assert_eq!(retro[0].sign_after.index(), 11);
        assert!(retro[0].retrograde);
    }

    #[test]
    fn invalid_or_ambiguous_requests_fail_closed() {
        let invalid = SearchWindow {
            step_us: 0,
            ..window()
        };
        assert_eq!(
            station_candidates(&Turn, &body(), invalid).unwrap_err(),
            GenerationError::InvalidWindow
        );
        let ambiguous = Linear {
            start: 10.0,
            rate: 60.0,
        };
        assert_eq!(
            ingress_candidates(&ambiguous, &body(), window()).unwrap_err(),
            GenerationError::AmbiguousIngressStep
        );
        let nonfinite = Linear {
            start: f64::NAN,
            rate: 1.0,
        };
        assert_eq!(
            ingress_candidates(&nonfinite, &body(), window()).unwrap_err(),
            GenerationError::InvalidSample
        );
    }
}
