/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use std::cell::Cell;
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    latitude_crossings::{
        adaptive_ecliptic_plane_crossings_scan, adaptive_latitude_stations_scan,
        ecliptic_plane_crossing_candidates, latitude_station_candidates,
    },
    GenerationError, LatitudeProvider, LatitudeSample, SearchWindow,
};

struct ZeroShape {
    sign: f64,
    flat: bool,
}

impl LatitudeProvider for ZeroShape {
    fn sample_latitude(&self, _: &BodyId, at: TimestampMicros) -> Result<LatitudeSample, String> {
        let x = at.as_i64() as f64 - 50.0;
        let value = if self.flat {
            self.sign * x.signum() * (x.abs() - 2.0).max(0.0) * 0.01
        } else {
            self.sign * x * 0.01
        };
        // Invented independent fields exercise the two numerical scanners.
        Ok(LatitudeSample {
            latitude_degrees: value,
            latitude_speed_degrees_per_day: value,
        })
    }
}

#[test]
fn sampled_zero_plateau_is_ambiguous_for_base_and_adaptive_scans() {
    let body = BodyId::try_from("Mars").unwrap();
    for sign in [-1.0, 1.0] {
        for step in [50, 100] {
            let model = ZeroShape { sign, flat: true };
            let window = SearchWindow {
                start_us: 0,
                end_us: 100,
                step_us: step,
                precision_us: 1,
            };
            assert_eq!(
                ecliptic_plane_crossing_candidates(&model, &body, window, &mut 1000).unwrap_err(),
                GenerationError::AmbiguousLatitudeCrossing
            );
            assert_eq!(
                latitude_station_candidates(&model, &body, window, &mut 1000).unwrap_err(),
                GenerationError::AmbiguousLatitudeCrossing
            );
            assert_eq!(
                adaptive_ecliptic_plane_crossings_scan(&model, &body, window, 1, &mut 1000)
                    .unwrap_err(),
                GenerationError::AmbiguousLatitudeCrossing
            );
            assert_eq!(
                adaptive_latitude_stations_scan(&model, &body, window, 1, &mut 1000).unwrap_err(),
                GenerationError::AmbiguousLatitudeCrossing
            );
        }
    }
}

#[test]
fn exact_isolated_zero_remains_a_candidate() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 50,
        precision_us: 1,
    };
    for sign in [-1.0, 1.0] {
        let model = ZeroShape { sign, flat: false };
        let plane = ecliptic_plane_crossing_candidates(&model, &body, window, &mut 1000).unwrap();
        let stations =
            adaptive_latitude_stations_scan(&model, &body, window, 1, &mut 1000).unwrap();
        assert_eq!(plane.len(), 1);
        assert_eq!(stations.candidates.len(), 1);
        assert_eq!(
            (plane[0].bracket_start_us, plane[0].bracket_end_us),
            (50, 50)
        );
        assert_eq!(
            (
                stations.candidates[0].bracket_start_us,
                stations.candidates[0].bracket_end_us
            ),
            (50, 50)
        );
    }
}

#[test]
fn isolated_zero_neighbour_checks_use_the_shared_budget() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 50,
        precision_us: 1,
    };
    for sign in [-1.0, 1.0] {
        let model = ZeroShape { sign, flat: false };
        let mut plane_budget = 4;
        assert_eq!(
            ecliptic_plane_crossing_candidates(&model, &body, window, &mut plane_budget)
                .unwrap_err(),
            GenerationError::TooManySteps,
        );
        assert_eq!(plane_budget, 0);
        let mut station_budget = 4;
        assert_eq!(
            latitude_station_candidates(&model, &body, window, &mut station_budget).unwrap_err(),
            GenerationError::TooManySteps,
        );
        assert_eq!(station_budget, 0);
    }
}

struct FailureAtNeighbour {
    at: i64,
    calls: Cell<usize>,
}

impl LatitudeProvider for FailureAtNeighbour {
    fn sample_latitude(&self, _: &BodyId, at: TimestampMicros) -> Result<LatitudeSample, String> {
        self.calls.set(self.calls.get() + 1);
        if at.as_i64() == self.at {
            return Err("synthetic adjacent probe failure".to_string());
        }
        let value = (at.as_i64() as f64 - 50.0) * 0.01;
        Ok(LatitudeSample {
            latitude_degrees: value,
            latitude_speed_degrees_per_day: value,
        })
    }
}

#[test]
fn adjacent_probe_failure_returns_no_partial_root_report() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 1,
    };
    for fail_at in [49, 51] {
        let model = FailureAtNeighbour {
            at: fail_at,
            calls: Cell::new(0),
        };
        let mut budget = 1000;
        assert_eq!(
            ecliptic_plane_crossing_candidates(&model, &body, window, &mut budget).unwrap_err(),
            GenerationError::Provider("synthetic adjacent probe failure".to_string()),
        );
        assert_eq!(budget, 1000 - model.calls.get());
        model.calls.set(0);
        budget = 1000;
        assert_eq!(
            adaptive_latitude_stations_scan(&model, &body, window, 1, &mut budget).unwrap_err(),
            GenerationError::Provider("synthetic adjacent probe failure".to_string()),
        );
        assert_eq!(budget, 1000 - model.calls.get());
    }
}
