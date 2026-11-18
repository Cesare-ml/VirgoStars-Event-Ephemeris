/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    aspects::{adaptive_exact_aspect_scan, exact_aspect_candidates},
    LongitudeProvider, LongitudeSample, SearchWindow,
};

struct Linear {
    crossing: i64,
    sign: f64,
}

impl LongitudeProvider for Linear {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let mars = body.as_str() == "Mars";
        Ok(LongitudeSample {
            longitude_degrees: if mars {
                0.0
            } else {
                60.0 + self.sign * (at.as_i64() - self.crossing) as f64 * 0.01
            },
            longitude_speed_degrees_per_day: if mars { 0.0 } else { self.sign * 864_000_000.0 },
        })
    }
}

#[test]
fn crossings_at_grid_and_window_boundaries_are_not_duplicated() {
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 200,
        step_us: 100,
        precision_us: 1,
    };
    for sign in [-1.0, 1.0] {
        for crossing in [0, 25, 50, 100, 150, 200] {
            let model = Linear { crossing, sign };
            let direct = exact_aspect_candidates(&model, &mars, &venus, window).unwrap();
            let adaptive =
                adaptive_exact_aspect_scan(&model, &mars, &venus, window, 2, &mut 1000).unwrap();
            let expected = if crossing == 200 { 0 } else { 1 };
            assert_eq!(direct.len(), expected, "direct: {sign} {crossing}");
            assert_eq!(
                adaptive.candidates.len(),
                expected,
                "adaptive: {sign} {crossing}"
            );
            for c in direct.iter().chain(&adaptive.candidates) {
                assert!(c.bracket_start_us <= crossing && c.bracket_end_us >= crossing);
                assert!(c.bracket_end_us - c.bracket_start_us <= 1);
            }
        }
    }
}
