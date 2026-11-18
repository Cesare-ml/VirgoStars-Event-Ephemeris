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

// Invented two-cycle oscillation: both endpoints and the midpoint agree,
// although the continuous model crosses the aspect in between.
struct AliasedOscillation;

impl LongitudeProvider for AliasedOscillation {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        let phase = 4.0 * std::f64::consts::PI * at.as_i64() as f64 / 100.0;
        let mars = body.as_str() == "Mars";
        Ok(LongitudeSample {
            longitude_degrees: if mars { 0.0 } else { 60.0 + 0.2 * phase.sin() },
            longitude_speed_degrees_per_day: if mars {
                0.0
            } else {
                0.2 * 4.0 * std::f64::consts::PI / 100.0 * phase.cos() * 86_400_000_000.0
            },
        })
    }
}

#[test]
fn agreeing_grid_and_midpoint_can_hide_opposite_sign_crossings() {
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 1,
    };
    let model = AliasedOscillation;
    let at_12 = model
        .sample(&venus, TimestampMicros::new(12))
        .unwrap()
        .longitude_degrees;
    let at_37 = model
        .sample(&venus, TimestampMicros::new(37))
        .unwrap()
        .longitude_degrees;
    assert!(at_12 > 60.0 && at_37 < 60.0);
    assert!(exact_aspect_candidates(&model, &mars, &venus, window)
        .unwrap()
        .is_empty());
    let report = adaptive_exact_aspect_scan(&model, &mars, &venus, window, 2, &mut 1000).unwrap();
    assert!(report.candidates.is_empty());
    assert_eq!(report.refined_segments, 0);
    // This demonstrates a blind spot, not absence of events or coverage.
}
