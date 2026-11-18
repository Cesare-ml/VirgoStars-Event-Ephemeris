/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use std::cell::Cell;
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    aspects::{adaptive_exact_aspect_scan, exact_aspect_candidates, ExactAspectKind},
    LongitudeProvider, LongitudeSample, SearchWindow,
};

const DAY: i64 = 86_400_000_000;
const PRECISION: i64 = 1_000_000;

// A deliberately invented linear model; it is not a physical ephemeris.
struct LinearPair {
    origin: i64,
    target: f64,
    rate: f64,
    rotation: f64,
    calls: Cell<usize>,
}

impl LongitudeProvider for LinearPair {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        self.calls.set(self.calls.get() + 1);
        let elapsed = (at.as_i64() as i128 - self.origin as i128) as f64 / DAY as f64;
        let (relative, speed) = if body.as_str() == "Mars" {
            (0.0, 0.0)
        } else {
            (self.target + self.rate * (elapsed - 0.5), self.rate)
        };
        Ok(LongitudeSample {
            longitude_degrees: (13.0 + self.rotation + relative).rem_euclid(360.0),
            longitude_speed_degrees_per_day: speed,
        })
    }
}

#[test]
fn aspects_preserve_pair_order_rotation_epoch_and_signed_motion() {
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let targets = [
        (0, ExactAspectKind::Conjunction),
        (60, ExactAspectKind::Sextile),
        (90, ExactAspectKind::Square),
        (120, ExactAspectKind::Trine),
        (240, ExactAspectKind::Trine),
        (270, ExactAspectKind::Square),
        (300, ExactAspectKind::Sextile),
    ];
    for (target, kind) in targets {
        for rate in [4.0, -4.0] {
            for origin in [
                i64::MIN + DAY,
                -2_200_000_000_000_000,
                1_500_000_000_000_000,
                i64::MAX - DAY,
            ] {
                for rotation in [0.0, 137.0] {
                    let model = LinearPair {
                        origin,
                        target: f64::from(target),
                        rate,
                        rotation,
                        calls: Cell::new(0),
                    };
                    let window = SearchWindow {
                        start_us: origin,
                        end_us: origin + DAY,
                        step_us: DAY / 4,
                        precision_us: PRECISION,
                    };
                    let direct = exact_aspect_candidates(&model, &mars, &venus, window).unwrap();
                    let reversed = exact_aspect_candidates(&model, &venus, &mars, window).unwrap();
                    assert_eq!(
                        direct, reversed,
                        "pair order: {target} {rate} {origin} {rotation}"
                    );
                    assert_eq!(direct.len(), 1, "expected one synthetic crossing");
                    let candidate = &direct[0];
                    assert_eq!(
                        (candidate.aspect, candidate.relative_target_degrees),
                        (kind, target)
                    );
                    assert!(candidate.bracket_start_us <= origin + DAY / 2);
                    assert!(candidate.bracket_end_us >= origin + DAY / 2);
                    assert!(candidate.bracket_end_us - candidate.bracket_start_us <= PRECISION);

                    model.calls.set(0);
                    let mut budget = 1_000;
                    let adaptive =
                        adaptive_exact_aspect_scan(&model, &mars, &venus, window, 2, &mut budget)
                            .unwrap();
                    assert_eq!(adaptive.candidates.len(), 1);
                    let candidate = &adaptive.candidates[0];
                    assert_eq!(
                        (candidate.aspect, candidate.relative_target_degrees),
                        (kind, target)
                    );
                    assert!(candidate.bracket_start_us <= origin + DAY / 2);
                    assert!(candidate.bracket_end_us >= origin + DAY / 2);
                    assert!(candidate.bracket_end_us - candidate.bracket_start_us <= PRECISION);
                    assert_eq!(model.calls.get(), 2 * (1_000 - budget));
                }
            }
        }
    }
}
