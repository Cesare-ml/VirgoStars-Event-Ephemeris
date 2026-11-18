/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    latitude_extrema::{adaptive_latitude_speed_extrema_scan, latitude_speed_extremum_candidates},
    speed_extrema::{adaptive_speed_extrema_scan, speed_extremum_candidates},
    GenerationError, LatitudeProvider, LatitudeSample, LongitudeProvider, LongitudeSample,
    SearchWindow,
};

// Invented flat and smooth shapes exercise numerical logic only.
struct Model {
    sign: f64,
    flat: bool,
}

impl Model {
    fn speed(&self, at: TimestampMicros) -> f64 {
        let x = at.as_i64() as f64;
        if self.flat {
            self.sign * (5.0 - ((x - 50.0).abs() - 15.0).max(0.0) * 0.01)
        } else {
            self.sign * (5.0 - (x - 500.0).powi(2) * 0.000001)
        }
    }
}

impl LongitudeProvider for Model {
    fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        Ok(LongitudeSample {
            longitude_degrees: 0.0,
            longitude_speed_degrees_per_day: self.speed(at),
        })
    }
}

impl LatitudeProvider for Model {
    fn sample_latitude(&self, _: &BodyId, at: TimestampMicros) -> Result<LatitudeSample, String> {
        Ok(LatitudeSample {
            latitude_degrees: 0.0,
            latitude_speed_degrees_per_day: self.speed(at),
        })
    }
}

#[test]
fn flat_interior_peak_fails_closed_in_both_base_scans() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 50,
        precision_us: 5,
    };
    for sign in [1.0, -1.0] {
        let model = Model { sign, flat: true };
        assert_eq!(
            speed_extremum_candidates(&model, &body, window, &mut 1000).unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum,
        );
        assert_eq!(
            latitude_speed_extremum_candidates(&model, &body, window, &mut 1000).unwrap_err(),
            GenerationError::AmbiguousLatitudeSpeedExtremum,
        );
    }
}

#[test]
fn flat_interior_peak_fails_closed_in_both_adaptive_scans() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 5,
    };
    for sign in [1.0, -1.0] {
        let model = Model { sign, flat: true };
        assert_eq!(
            adaptive_speed_extrema_scan(&model, &body, window, 1, &mut 1000).unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum,
        );
        assert_eq!(
            adaptive_latitude_speed_extrema_scan(&model, &body, window, 1, &mut 1000).unwrap_err(),
            GenerationError::AmbiguousLatitudeSpeedExtremum,
        );
    }
}

#[test]
fn equal_symmetric_probes_can_still_bracket_a_strict_smooth_peak() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 1000,
        step_us: 500,
        precision_us: 10,
    };
    for sign in [1.0, -1.0] {
        let model = Model { sign, flat: false };
        let longitude = speed_extremum_candidates(&model, &body, window, &mut 1000).unwrap();
        let latitude =
            latitude_speed_extremum_candidates(&model, &body, window, &mut 1000).unwrap();
        assert_eq!(longitude.len(), 1);
        assert_eq!(latitude.len(), 1);
        assert!(longitude[0].bracket_start_us <= 500 && longitude[0].bracket_end_us >= 500);
        assert!(latitude[0].bracket_start_us <= 500 && latitude[0].bracket_end_us >= 500);
        assert!(longitude[0].bracket_end_us - longitude[0].bracket_start_us <= 10);
        assert!(latitude[0].bracket_end_us - latitude[0].bracket_start_us <= 10);
    }
}

#[test]
fn symmetric_tie_breaking_consumes_the_shared_provider_budget() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 1000,
        step_us: 500,
        precision_us: 10,
    };
    for sign in [1.0, -1.0] {
        let model = Model { sign, flat: false };
        let mut longitude_budget = 5;
        assert_eq!(
            speed_extremum_candidates(&model, &body, window, &mut longitude_budget).unwrap_err(),
            GenerationError::TooManySteps,
        );
        assert_eq!(longitude_budget, 0);
        let mut latitude_budget = 5;
        assert_eq!(
            latitude_speed_extremum_candidates(&model, &body, window, &mut latitude_budget)
                .unwrap_err(),
            GenerationError::TooManySteps,
        );
        assert_eq!(latitude_budget, 0);
    }
}
