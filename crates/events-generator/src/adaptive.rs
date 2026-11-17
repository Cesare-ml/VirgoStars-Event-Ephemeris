/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Risk-triggered station scan, for offline generator QA only.
//! Not an interval-arithmetic proof of the absence of unsampled speed roots.

use virgostars_events_core::BodyId;

use crate::{
    checked_sample, ingress_candidates,
    lunar::{lunar_phase_candidates, phase_state, LunarPhaseCandidate, PhaseState},
    midpoint, next_time, station_candidates, zodiac_sign, GenerationError, IngressCandidate,
    LongitudeProvider, LongitudeSample, SearchWindow, StationCandidate,
};

#[derive(Default, Debug)]
pub struct AdaptiveStationReport {
    pub candidates: Vec<StationCandidate>,
    pub midpoint_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

/// Results from an *uncertified* tropical-ingress risk rescan.
#[derive(Default, Debug)]
pub struct AdaptiveIngressReport {
    pub candidates: Vec<IngressCandidate>,
    pub midpoint_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

/// Results from an *uncertified* lunar-quarter-angle risk rescan.
#[derive(Default, Debug)]
pub struct AdaptiveLunarReport {
    pub candidates: Vec<LunarPhaseCandidate>,
    pub midpoint_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

fn opposite_speed_signs(a: f64, b: f64) -> bool {
    (a < 0.0) != (b < 0.0)
}

fn nearest_tropical_boundary(longitude: f64) -> f64 {
    let within_sign = longitude.rem_euclid(30.0);
    within_sign.min(30.0 - within_sign)
}

struct AdaptiveScan<'a, P> {
    provider: &'a P,
    body: &'a BodyId,
    precision_us: i64,
    remaining_probes: &'a mut usize,
    report: AdaptiveStationReport,
}

impl<P: LongitudeProvider> AdaptiveScan<'_, P> {
    fn segment(
        &mut self,
        start_us: i64,
        end_us: i64,
        start_speed: f64,
        end_speed: f64,
        depth_remaining: u8,
    ) -> Result<(), GenerationError> {
        if depth_remaining > 0 && end_us as i128 - start_us as i128 > self.precision_us as i128 {
            let mid_us = midpoint(start_us, end_us);
            if mid_us > start_us && mid_us < end_us {
                if *self.remaining_probes == 0 {
                    return Err(GenerationError::TooManySteps);
                }
                *self.remaining_probes -= 1;
                self.report.midpoint_probes += 1;
                let mid_speed = checked_sample(self.provider, self.body, mid_us)?
                    .longitude_speed_degrees_per_day;
                // Two roots can hide between equal-sign endpoints. An opposite
                // midpoint sign or an unexpectedly deep speed dip triggers
                // finer local inspection; no-risk regions remain coarse.
                let risky = opposite_speed_signs(start_speed, mid_speed)
                    || opposite_speed_signs(mid_speed, end_speed)
                    || mid_speed.abs() * 2.0 < start_speed.abs().min(end_speed.abs());
                if risky {
                    self.report.refined_segments += 1;
                    self.segment(
                        start_us,
                        mid_us,
                        start_speed,
                        mid_speed,
                        depth_remaining - 1,
                    )?;
                    self.segment(mid_us, end_us, mid_speed, end_speed, depth_remaining - 1)?;
                    return Ok(());
                }
            }
        }

        self.report.leaf_segments += 1;
        let span_us = (end_us as i128 - start_us as i128) as i64;
        let leaf = SearchWindow {
            start_us,
            end_us,
            step_us: span_us,
            precision_us: self.precision_us.min(span_us),
        };
        self.report
            .candidates
            .extend(station_candidates(self.provider, self.body, leaf)?);
        Ok(())
    }
}

/// Scan motion-station candidates using midpoint speed-risk triage.
///
/// Every coarse step gets one midpoint probe. Only suspicious segments are
/// bisected recursively (up to `max_depth`), then the existing zero-crossing
/// bracketer is run on each resulting segment. `remaining_probes` is a shared
/// budget across chunks; exhaustion fails without claiming a complete scan.
/// A flat same-sign speed curve can still conceal two roots, so this is a
/// *heuristic crosscheck*, not certified astronomical coverage.
pub fn adaptive_station_scan<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    max_depth: u8,
    remaining_probes: &mut usize,
) -> Result<AdaptiveStationReport, GenerationError> {
    window.validate()?;
    if max_depth == 0 || max_depth > 8 || *remaining_probes == 0 {
        return Err(GenerationError::InvalidWindow);
    }
    if (window.step_us as i128) / (1_i128 << max_depth) < window.precision_us as i128 {
        return Err(GenerationError::InvalidWindow);
    }

    let mut scan = AdaptiveScan {
        provider,
        body,
        precision_us: window.precision_us,
        remaining_probes,
        report: AdaptiveStationReport::default(),
    };
    let mut start = window.start_us;
    let mut start_speed = checked_sample(provider, body, start)?.longitude_speed_degrees_per_day;
    while start < window.end_us {
        let end = next_time(start, window);
        let end_speed = checked_sample(provider, body, end)?.longitude_speed_degrees_per_day;
        scan.segment(start, end, start_speed, end_speed, max_depth)?;
        start = end;
        start_speed = end_speed;
    }
    scan.report
        .candidates
        .sort_by_key(|event| event.bracket_start_us);
    Ok(scan.report)
}

struct AdaptiveIngressScan<'a, P> {
    provider: &'a P,
    body: &'a BodyId,
    precision_us: i64,
    remaining_probes: &'a mut usize,
    report: AdaptiveIngressReport,
}

impl<P: LongitudeProvider> AdaptiveIngressScan<'_, P> {
    fn segment(
        &mut self,
        start_us: i64,
        end_us: i64,
        start: LongitudeSample,
        end: LongitudeSample,
        depth_remaining: u8,
    ) -> Result<(), GenerationError> {
        if depth_remaining > 0 && end_us as i128 - start_us as i128 > self.precision_us as i128 {
            let mid_us = midpoint(start_us, end_us);
            if mid_us > start_us && mid_us < end_us {
                if *self.remaining_probes == 0 {
                    return Err(GenerationError::TooManySteps);
                }
                *self.remaining_probes -= 1;
                self.report.midpoint_probes += 1;
                let mid = checked_sample(self.provider, self.body, mid_us)?;

                let boundary_start = nearest_tropical_boundary(start.longitude_degrees);
                let boundary_end = nearest_tropical_boundary(end.longitude_degrees);
                let boundary_mid = nearest_tropical_boundary(mid.longitude_degrees);
                // A return to the starting sign can hide a forward ingress
                // and retrograde re-entry. Refine if the midpoint changes
                // sign, speed reverses, or the midpoint nears a boundary.
                let risky = zodiac_sign(start.longitude_degrees)
                    != zodiac_sign(mid.longitude_degrees)
                    || zodiac_sign(mid.longitude_degrees) != zodiac_sign(end.longitude_degrees)
                    || opposite_speed_signs(
                        start.longitude_speed_degrees_per_day,
                        mid.longitude_speed_degrees_per_day,
                    )
                    || opposite_speed_signs(
                        mid.longitude_speed_degrees_per_day,
                        end.longitude_speed_degrees_per_day,
                    )
                    || boundary_mid * 2.0 < boundary_start.min(boundary_end);
                if risky {
                    self.report.refined_segments += 1;
                    self.segment(start_us, mid_us, start, mid, depth_remaining - 1)?;
                    self.segment(mid_us, end_us, mid, end, depth_remaining - 1)?;
                    return Ok(());
                }
            }
        }

        self.report.leaf_segments += 1;
        let span_us = (end_us as i128 - start_us as i128) as i64;
        self.report.candidates.extend(ingress_candidates(
            self.provider,
            self.body,
            SearchWindow {
                start_us,
                end_us,
                step_us: span_us,
                precision_us: self.precision_us.min(span_us),
            },
        )?);
        Ok(())
    }
}

/// Midpoint-risk rescanning for tropical sign ingress/re-entry candidates.
///
/// All coarse segments receive one midpoint probe. Suspicious segments are
/// recursively bisected, sharing the remaining-probe budget across chunks.
/// The sign and speed criteria are *heuristics*: a smooth same-sign curve can
/// conceal two crossings between any three samples. No scientific coverage
/// certificate is implied by a successful comparison.
pub fn adaptive_ingress_scan<P: LongitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    max_depth: u8,
    remaining_probes: &mut usize,
) -> Result<AdaptiveIngressReport, GenerationError> {
    window.validate()?;
    if max_depth == 0 || max_depth > 8 || *remaining_probes == 0 {
        return Err(GenerationError::InvalidWindow);
    }
    if (window.step_us as i128) / (1_i128 << max_depth) < window.precision_us as i128 {
        return Err(GenerationError::InvalidWindow);
    }

    let mut scan = AdaptiveIngressScan {
        provider,
        body,
        precision_us: window.precision_us,
        remaining_probes,
        report: AdaptiveIngressReport::default(),
    };
    let mut start_us = window.start_us;
    let mut start = checked_sample(provider, body, start_us)?;
    while start_us < window.end_us {
        let end_us = next_time(start_us, window);
        let end = checked_sample(provider, body, end_us)?;
        scan.segment(start_us, end_us, start, end, max_depth)?;
        start_us = end_us;
        start = end;
    }
    scan.report
        .candidates
        .sort_by_key(|event| event.bracket_start_us);
    Ok(scan.report)
}

fn quarter_boundary_distance(angle: f64) -> f64 {
    let within_quarter = angle.rem_euclid(90.0);
    within_quarter.min(90.0 - within_quarter)
}

fn advancing_phase_angle(from: f64, to: f64) -> f64 {
    (to - from).rem_euclid(360.0)
}

struct AdaptiveLunarScan<'a, P> {
    provider: &'a P,
    sun: &'a BodyId,
    moon: &'a BodyId,
    precision_us: i64,
    remaining_probes: &'a mut usize,
    report: AdaptiveLunarReport,
}

impl<P: LongitudeProvider> AdaptiveLunarScan<'_, P> {
    fn segment(
        &mut self,
        start_us: i64,
        end_us: i64,
        start: PhaseState,
        end: PhaseState,
        depth_remaining: u8,
    ) -> Result<(), GenerationError> {
        if depth_remaining > 0 && end_us as i128 - start_us as i128 > self.precision_us as i128 {
            let mid_us = midpoint(start_us, end_us);
            if mid_us > start_us && mid_us < end_us {
                if *self.remaining_probes == 0 {
                    return Err(GenerationError::TooManySteps);
                }
                *self.remaining_probes -= 1;
                self.report.midpoint_probes += 1;
                let mid = phase_state(self.provider, self.sun, self.moon, mid_us)?;
                let coarse = advancing_phase_angle(start.angle_degrees, end.angle_degrees);
                let first = advancing_phase_angle(start.angle_degrees, mid.angle_degrees);
                let second = advancing_phase_angle(mid.angle_degrees, end.angle_degrees);
                // A full 360° loop can alias to a seemingly harmless coarse
                // delta. Even two modulo-small half steps can mask two loops;
                // use the supplied instantaneous relative speed as an
                // additional conservative undersampling warning.
                let days = (end_us as i128 - start_us as i128) as f64 / 86_400_000_000.0;
                let speed_estimate = (start.speed_degrees_per_day
                    + 2.0 * mid.speed_degrees_per_day
                    + end.speed_degrees_per_day)
                    * days
                    / 4.0;
                if first >= 30.0
                    || second >= 30.0
                    || first + second > coarse + 1e-7
                    || speed_estimate >= 30.0
                {
                    return Err(GenerationError::AmbiguousLunarStep);
                }

                let boundary_start = quarter_boundary_distance(start.angle_degrees);
                let boundary_end = quarter_boundary_distance(end.angle_degrees);
                let boundary_mid = quarter_boundary_distance(mid.angle_degrees);
                let speed_peak = mid.speed_degrees_per_day
                    > start.speed_degrees_per_day.max(end.speed_degrees_per_day) * 1.5;
                let risky = boundary_mid * 2.0 < boundary_start.min(boundary_end) || speed_peak;
                if risky {
                    self.report.refined_segments += 1;
                    self.segment(start_us, mid_us, start, mid, depth_remaining - 1)?;
                    self.segment(mid_us, end_us, mid, end, depth_remaining - 1)?;
                    return Ok(());
                }
            }
        }

        self.report.leaf_segments += 1;
        let span_us = (end_us as i128 - start_us as i128) as i64;
        self.report.candidates.extend(lunar_phase_candidates(
            self.provider,
            SearchWindow {
                start_us,
                end_us,
                step_us: span_us,
                precision_us: self.precision_us.min(span_us),
            },
        )?);
        Ok(())
    }
}

/// Opt-in lunar phase-crossing risk check using midpoint Moon–Sun angles.
///
/// Validates each midpoint against the original modulo-360 advancement and
/// a speed-derived warning threshold. Only suspicious quarters are further
/// subdivided. Equal coarse and fine candidate sets are *not* a completeness
/// certificate: this remains a sample-dependent synthetic quality gate.
pub fn adaptive_lunar_scan<P: LongitudeProvider>(
    provider: &P,
    window: SearchWindow,
    max_depth: u8,
    remaining_probes: &mut usize,
) -> Result<AdaptiveLunarReport, GenerationError> {
    window.validate()?;
    if max_depth == 0 || max_depth > 8 || *remaining_probes == 0 {
        return Err(GenerationError::InvalidWindow);
    }
    if (window.step_us as i128) / (1_i128 << max_depth) < window.precision_us as i128 {
        return Err(GenerationError::InvalidWindow);
    }
    let sun = BodyId::try_from("Sun").expect("known Sun");
    let moon = BodyId::try_from("Moon").expect("known Moon");
    let mut scan = AdaptiveLunarScan {
        provider,
        sun: &sun,
        moon: &moon,
        precision_us: window.precision_us,
        remaining_probes,
        report: AdaptiveLunarReport::default(),
    };
    let mut start_us = window.start_us;
    let mut start = phase_state(provider, &sun, &moon, start_us)?;
    while start_us < window.end_us {
        let end_us = next_time(start_us, window);
        let end = phase_state(provider, &sun, &moon, end_us)?;
        scan.segment(start_us, end_us, start, end, max_depth)?;
        start_us = end_us;
        start = end;
    }
    scan.report
        .candidates
        .sort_by_key(|event| event.bracket_start_us);
    Ok(scan.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LongitudeSample, TimestampMicros};

    const DAY: i64 = 86_400_000_000;

    struct DoubleStation;
    impl LongitudeProvider for DoubleStation {
        fn sample(&self, _body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: 0.0,
                longitude_speed_degrees_per_day: (days - 0.25) * (days - 0.75),
            })
        }
    }

    struct DoubleIngress;
    impl LongitudeProvider for DoubleIngress {
        fn sample(&self, _body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let days = at.as_i64() as f64 / DAY as f64;
            Ok(LongitudeSample {
                longitude_degrees: 29.0 + 8.0 * (0.25 - (days - 0.5).powi(2)),
                longitude_speed_degrees_per_day: -16.0 * (days - 0.5),
            })
        }
    }

    struct WraparoundIngress;
    impl LongitudeProvider for WraparoundIngress {
        fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
            let mut sample = DoubleIngress.sample(body, at)?;
            sample.longitude_degrees += 330.0;
            Ok(sample)
        }
    }

    struct LunarOrbit {
        relative_rate: f64,
    }
    impl LongitudeProvider for LunarOrbit {
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
                    longitude_degrees: sun + 10.0 + self.relative_rate * days,
                    longitude_speed_degrees_per_day: 1.0 + self.relative_rate,
                })
            }
        }
    }

    fn window() -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: DAY,
            step_us: DAY,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn finds_two_hidden_stations_by_refining_only_risky_segments() {
        let mars = BodyId::try_from("Mars").unwrap();
        assert!(station_candidates(&DoubleStation, &mars, window())
            .unwrap()
            .is_empty());
        let mut budget = 100;
        let report =
            adaptive_station_scan(&DoubleStation, &mars, window(), 6, &mut budget).unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert!(report.refined_segments > 0);
        assert!(report.midpoint_probes > report.refined_segments);
        assert!(report.leaf_segments < 64);
    }

    #[test]
    fn bounded_probe_budget_stops_before_a_misleading_result() {
        let mars = BodyId::try_from("Mars").unwrap();
        let mut budget = 1;
        assert_eq!(
            adaptive_station_scan(&DoubleStation, &mars, window(), 6, &mut budget).unwrap_err(),
            GenerationError::TooManySteps
        );
    }

    #[test]
    fn finds_direct_ingress_and_retrograde_reentry_hidden_in_one_coarse_step() {
        let mars = BodyId::try_from("Mars").unwrap();
        assert!(ingress_candidates(&DoubleIngress, &mars, window())
            .unwrap()
            .is_empty());
        let mut budget = 100;
        let report =
            adaptive_ingress_scan(&DoubleIngress, &mars, window(), 6, &mut budget).unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].sign_after.index(), 1);
        assert!(!report.candidates[0].retrograde);
        assert_eq!(report.candidates[1].sign_after.index(), 0);
        assert!(report.candidates[1].retrograde);
        assert!(report.refined_segments > 0);
        assert!(report.leaf_segments < 64);
    }

    #[test]
    fn ingress_midpoint_budget_is_shared_and_fail_closed() {
        let mars = BodyId::try_from("Mars").unwrap();
        let mut budget = 1;
        assert_eq!(
            adaptive_ingress_scan(&DoubleIngress, &mars, window(), 6, &mut budget).unwrap_err(),
            GenerationError::TooManySteps
        );
    }

    #[test]
    fn detects_pisces_to_aries_and_reentry_across_zodiac_wrap() {
        let mars = BodyId::try_from("Mars").unwrap();
        let mut budget = 100;
        let report =
            adaptive_ingress_scan(&WraparoundIngress, &mars, window(), 6, &mut budget).unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].sign_after.index(), 0);
        assert!(!report.candidates[0].retrograde);
        assert_eq!(report.candidates[1].sign_after.index(), 11);
        assert!(report.candidates[1].retrograde);
    }

    #[test]
    fn lunar_midpoint_speed_guard_rejects_alias_of_two_full_phase_revolutions() {
        let fast = LunarOrbit {
            relative_rate: 730.0,
        };
        assert!(lunar_phase_candidates(&fast, window()).unwrap().is_empty());
        let mut budget = 100;
        assert_eq!(
            adaptive_lunar_scan(&fast, window(), 6, &mut budget).unwrap_err(),
            GenerationError::AmbiguousLunarStep
        );
    }

    #[test]
    fn lunar_adaptive_scan_reproduces_normal_quarter_phase_brackets() {
        let normal = LunarOrbit {
            relative_rate: 12.0,
        };
        let days = SearchWindow {
            start_us: 0,
            end_us: 40 * DAY,
            step_us: DAY / 4,
            precision_us: 1_000_000,
        };
        let coarse = lunar_phase_candidates(&normal, days).unwrap();
        assert_eq!(coarse.len(), 5);
        let mut budget = 2000;
        let report = adaptive_lunar_scan(&normal, days, 6, &mut budget).unwrap();
        assert_eq!(report.candidates.len(), coarse.len());
        for (a, b) in report.candidates.iter().zip(coarse.iter()) {
            assert_eq!(a.phase, b.phase);
            let a_mid = a.bracket_start_us as i128 + a.bracket_end_us as i128;
            let b_mid = b.bracket_start_us as i128 + b.bracket_end_us as i128;
            assert!((a_mid - b_mid).abs() <= 2_000_000);
        }
    }

    #[test]
    fn lunar_midpoint_probe_budget_fails_without_partial_results() {
        let normal = LunarOrbit {
            relative_rate: 12.0,
        };
        let mut budget = 1;
        assert_eq!(
            adaptive_lunar_scan(
                &normal,
                SearchWindow {
                    end_us: 2 * DAY,
                    ..window()
                },
                6,
                &mut budget
            )
            .unwrap_err(),
            GenerationError::TooManySteps
        );
    }
}
