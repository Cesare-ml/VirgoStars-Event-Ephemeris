/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Geocentric tropical lunar phase *candidate brackets* from Sun/Moon longitudes.
//! These are not the authoritative legacy `sinodi-func.js` source phase rows.

use virgostars_events_core::BodyId;

use crate::{
    checked_sample, midpoint, needs_refinement, next_time, GenerationError, LongitudeProvider,
    SearchWindow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LunarPhase {
    NewMoon,
    FirstQuarter,
    FullMoon,
    LastQuarter,
}

impl LunarPhase {
    pub fn code(self) -> &'static str {
        match self {
            Self::NewMoon => "new_moon",
            Self::FirstQuarter => "first_quarter",
            Self::FullMoon => "full_moon",
            Self::LastQuarter => "last_quarter",
        }
    }

    fn longitude_target(self) -> f64 {
        match self {
            Self::NewMoon => 0.0,
            Self::FirstQuarter => 90.0,
            Self::FullMoon => 180.0,
            Self::LastQuarter => 270.0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LunarPhaseCandidate {
    pub phase: LunarPhase,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
}

fn phase_angle<P: LongitudeProvider>(
    provider: &P,
    sun: &BodyId,
    moon: &BodyId,
    at_us: i64,
) -> Result<f64, GenerationError> {
    let sun_position = checked_sample(provider, sun, at_us)?;
    let moon_position = checked_sample(provider, moon, at_us)?;
    if moon_position.longitude_speed_degrees_per_day <= sun_position.longitude_speed_degrees_per_day
    {
        return Err(GenerationError::AmbiguousLunarStep);
    }
    Ok((moon_position.longitude_degrees - sun_position.longitude_degrees).rem_euclid(360.0))
}

/// Moon phases at 0°/90°/180°/270° of the Moon–Sun tropical longitude gap.
///
/// The relative angle must advance less than 30° in each scan step; anything
/// larger or retrograde fails closed. Multiple unsampled crossings inside a
/// step are not proved absent; adaptive/golden scientific validation remains
/// mandatory before building a new data release.
pub fn lunar_phase_candidates<P: LongitudeProvider>(
    provider: &P,
    window: SearchWindow,
) -> Result<Vec<LunarPhaseCandidate>, GenerationError> {
    window.validate()?;
    let moon = BodyId::try_from("Moon").expect("known Moon body");
    let sun = BodyId::try_from("Sun").expect("known Sun body");
    let mut candidates = Vec::new();
    let mut t = window.start_us;
    let mut prior = phase_angle(provider, &sun, &moon, t)?;

    while t < window.end_us {
        let end = next_time(t, window);
        let after = phase_angle(provider, &sun, &moon, end)?;
        let progress = (after - prior).rem_euclid(360.0);
        if progress >= 30.0 {
            return Err(GenerationError::AmbiguousLunarStep);
        }
        for phase in [
            LunarPhase::NewMoon,
            LunarPhase::FirstQuarter,
            LunarPhase::FullMoon,
            LunarPhase::LastQuarter,
        ] {
            let offset = (phase.longitude_target() - prior).rem_euclid(360.0);
            if offset > 0.0 && offset <= progress {
                let mut left = t;
                let mut right = end;
                while needs_refinement(left, right, window.precision_us) {
                    let mid = midpoint(left, right);
                    let middle = phase_angle(provider, &sun, &moon, mid)?;
                    let advanced = (middle - prior).rem_euclid(360.0);
                    if advanced >= 30.0 {
                        return Err(GenerationError::AmbiguousLunarStep);
                    }
                    if advanced >= offset {
                        right = mid;
                    } else {
                        left = mid;
                    }
                }
                candidates.push(LunarPhaseCandidate {
                    phase,
                    bracket_start_us: left,
                    bracket_end_us: right,
                });
            }
        }
        t = end;
        prior = after;
    }

    candidates.sort_by_key(|candidate| candidate.bracket_start_us);
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LongitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct MockMoonSun;
    impl LongitudeProvider for MockMoonSun {
        fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            let sun = 10.0 + days;
            if body.as_str() == "Sun" {
                Ok(LongitudeSample {
                    longitude_degrees: sun,
                    longitude_speed_degrees_per_day: 1.0,
                })
            } else {
                Ok(LongitudeSample {
                    longitude_degrees: sun + 90.0 * days,
                    longitude_speed_degrees_per_day: 91.0,
                })
            }
        }
    }

    #[test]
    fn detects_all_four_lunar_phase_brackets_and_wrap() {
        let events = lunar_phase_candidates(
            &MockMoonSun,
            SearchWindow {
                start_us: 0,
                end_us: 4 * DAY + DAY / 4,
                step_us: DAY / 4,
                precision_us: 1_000_000,
            },
        )
        .unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].phase, LunarPhase::FirstQuarter);
        assert_eq!(events[1].phase, LunarPhase::FullMoon);
        assert_eq!(events[2].phase, LunarPhase::LastQuarter);
        assert_eq!(events[3].phase, LunarPhase::NewMoon);
        for (index, event) in events.iter().enumerate() {
            let exact = (index as i64 + 1) * DAY;
            assert!(event.bracket_start_us <= exact && event.bracket_end_us >= exact);
            assert!(event.bracket_end_us - event.bracket_start_us <= 1_000_000);
        }
    }

    #[test]
    fn undersampled_lunar_phase_scan_fails_closed() {
        let error = lunar_phase_candidates(
            &MockMoonSun,
            SearchWindow {
                start_us: 0,
                end_us: 2 * DAY,
                step_us: DAY,
                precision_us: 1_000_000,
            },
        )
        .unwrap_err();
        assert_eq!(error, GenerationError::AmbiguousLunarStep);
    }
}
