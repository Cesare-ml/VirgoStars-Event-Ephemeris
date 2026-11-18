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

// Deliberately synthetic parabolas: neither coordinates nor speeds are
// intended to model a physical body or provide an astronomical reference.
struct SyntheticPeak(f64);

impl LongitudeProvider for SyntheticPeak {
    fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let x = at.as_i64() as f64 - 3.0;
        Ok(LongitudeSample {
            longitude_degrees: 0.0,
            longitude_speed_degrees_per_day: self.0 * (4.0 - x * x),
        })
    }
}

impl LatitudeProvider for SyntheticPeak {
    fn sample_latitude(&self, _: &BodyId, at: TimestampMicros) -> Result<LatitudeSample, String> {
        let x = at.as_i64() as f64 - 3.0;
        Ok(LatitudeSample {
            latitude_degrees: 0.0,
            latitude_speed_degrees_per_day: self.0 * (4.0 - x * x),
        })
    }
}

fn window() -> SearchWindow {
    SearchWindow {
        start_us: 0,
        end_us: 6,
        step_us: 3,
        precision_us: 1,
    }
}

#[test]
fn longitude_peaks_never_silently_exceed_requested_precision() {
    for sign in [1.0, -1.0] {
        let result = speed_extremum_candidates(
            &SyntheticPeak(sign),
            &BodyId::try_from("Mars").unwrap(),
            window(),
            &mut 100,
        );
        assert_eq!(result.unwrap_err(), GenerationError::AmbiguousSpeedExtremum);
        let mut relaxed = window();
        relaxed.precision_us = 3;
        let candidates = speed_extremum_candidates(
            &SyntheticPeak(sign),
            &BodyId::try_from("Mars").unwrap(),
            relaxed,
            &mut 100,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates
            .iter()
            .all(|c| c.bracket_end_us - c.bracket_start_us <= 3));
    }
}

#[test]
fn latitude_peaks_never_silently_exceed_requested_precision() {
    for sign in [1.0, -1.0] {
        let result = latitude_speed_extremum_candidates(
            &SyntheticPeak(sign),
            &BodyId::try_from("Mars").unwrap(),
            window(),
            &mut 100,
        );
        assert_eq!(
            result.unwrap_err(),
            GenerationError::AmbiguousLatitudeSpeedExtremum
        );
        let mut relaxed = window();
        relaxed.precision_us = 3;
        let candidates = latitude_speed_extremum_candidates(
            &SyntheticPeak(sign),
            &BodyId::try_from("Mars").unwrap(),
            relaxed,
            &mut 100,
        )
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates
            .iter()
            .all(|c| c.bracket_end_us - c.bracket_start_us <= 3));
    }
}

struct SyntheticAdaptivePeak(f64);

impl LongitudeProvider for SyntheticAdaptivePeak {
    fn sample(&self, _: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let x = at.as_i64() as f64 - 8.0;
        Ok(LongitudeSample {
            longitude_degrees: 0.0,
            longitude_speed_degrees_per_day: self.0 * (64.0 - x * x),
        })
    }
}

impl LatitudeProvider for SyntheticAdaptivePeak {
    fn sample_latitude(&self, _: &BodyId, at: TimestampMicros) -> Result<LatitudeSample, String> {
        let x = at.as_i64() as f64 - 8.0;
        Ok(LatitudeSample {
            latitude_degrees: 0.0,
            latitude_speed_degrees_per_day: self.0 * (64.0 - x * x),
        })
    }
}

#[test]
fn adaptive_peak_scans_propagate_unattainable_precision() {
    let body = BodyId::try_from("Mars").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 16,
        step_us: 16,
        precision_us: 1,
    };
    for sign in [1.0, -1.0] {
        let model = SyntheticAdaptivePeak(sign);
        let longitude = adaptive_speed_extrema_scan(&model, &body, window, 1, &mut 100);
        assert_eq!(
            longitude.unwrap_err(),
            GenerationError::AmbiguousSpeedExtremum
        );
        let latitude = adaptive_latitude_speed_extrema_scan(&model, &body, window, 1, &mut 100);
        assert_eq!(
            latitude.unwrap_err(),
            GenerationError::AmbiguousLatitudeSpeedExtremum
        );
    }
}
