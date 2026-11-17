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
    checked_sample, midpoint, next_time, station_candidates, GenerationError, LongitudeProvider,
    SearchWindow, StationCandidate,
};

#[derive(Default, Debug)]
pub struct AdaptiveStationReport {
    pub candidates: Vec<StationCandidate>,
    pub midpoint_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

fn opposite_speed_signs(a: f64, b: f64) -> bool {
    (a < 0.0) != (b < 0.0)
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
}
