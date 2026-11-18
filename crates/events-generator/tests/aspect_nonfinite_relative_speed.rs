/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    aspects::{adaptive_exact_aspect_scan, exact_aspect_candidates, potential_aspect_tangencies},
    GenerationError, LongitudeProvider, LongitudeSample, SearchWindow,
};

// Finite individual synthetic speeds can overflow their relative difference.
struct OverflowAtMidpoint;

impl LongitudeProvider for OverflowAtMidpoint {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let time = at.as_i64();
        let mars = body.as_str() == "Mars";
        let speed = if time == 50 {
            if mars {
                -f64::MAX
            } else {
                f64::MAX
            }
        } else if mars {
            0.0
        } else if time < 50 {
            -1.0
        } else {
            1.0
        };
        Ok(LongitudeSample {
            longitude_degrees: if mars { 0.0 } else { 59.0 + time as f64 / 50.0 },
            longitude_speed_degrees_per_day: speed,
        })
    }
}

#[test]
fn reject_overflowed_relative_speed_during_each_kind_of_scan() {
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 1,
    };
    assert_eq!(
        exact_aspect_candidates(&OverflowAtMidpoint, &mars, &venus, window).unwrap_err(),
        GenerationError::InvalidSample,
    );
    assert_eq!(
        adaptive_exact_aspect_scan(&OverflowAtMidpoint, &mars, &venus, window, 1, &mut 100)
            .unwrap_err(),
        GenerationError::InvalidSample,
    );
    assert_eq!(
        potential_aspect_tangencies(&OverflowAtMidpoint, &mars, &venus, window, 0.01).unwrap_err(),
        GenerationError::InvalidSample,
    );
}

struct OverflowedLongitudeDifference;

impl LongitudeProvider for OverflowedLongitudeDifference {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let time = at.as_i64();
        let mars = body.as_str() == "Mars";
        Ok(LongitudeSample {
            longitude_degrees: if time == 50 {
                if mars {
                    -f64::MAX
                } else {
                    f64::MAX
                }
            } else if mars {
                0.0
            } else {
                59.0 + time as f64 / 50.0
            },
            longitude_speed_degrees_per_day: if mars {
                0.0
            } else if time < 50 {
                -1.0
            } else {
                1.0
            },
        })
    }
}

#[test]
fn reject_overflowed_longitude_difference_during_each_kind_of_scan() {
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 1,
    };
    assert_eq!(
        exact_aspect_candidates(&OverflowedLongitudeDifference, &mars, &venus, window).unwrap_err(),
        GenerationError::InvalidSample,
    );
    assert_eq!(
        adaptive_exact_aspect_scan(
            &OverflowedLongitudeDifference,
            &mars,
            &venus,
            window,
            1,
            &mut 100
        )
        .unwrap_err(),
        GenerationError::InvalidSample,
    );
    assert_eq!(
        potential_aspect_tangencies(&OverflowedLongitudeDifference, &mars, &venus, window, 0.01)
            .unwrap_err(),
        GenerationError::InvalidSample,
    );
}
