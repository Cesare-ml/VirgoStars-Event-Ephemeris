/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
use std::cell::Cell;
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::{
    aspects::{adaptive_exact_aspect_scan, exact_aspect_candidates},
    GenerationError, LongitudeProvider, LongitudeSample, SearchWindow,
};

struct CountingProvider(Cell<usize>);

impl LongitudeProvider for CountingProvider {
    fn sample(&self, body: &BodyId, _at: TimestampMicros) -> Result<LongitudeSample, String> {
        self.0.set(self.0.get() + 1);
        Ok(LongitudeSample {
            longitude_degrees: if body.as_str() == "Mars" { 0.0 } else { 1.0 },
            longitude_speed_degrees_per_day: 0.0,
        })
    }
}

#[test]
fn one_pair_probe_budget_does_not_permit_unbounded_provider_calls() {
    let provider = CountingProvider(Cell::new(0));
    let mut budget = 1;
    let result = adaptive_exact_aspect_scan(
        &provider,
        &BodyId::try_from("Mars").unwrap(),
        &BodyId::try_from("Venus").unwrap(),
        SearchWindow {
            start_us: 0,
            end_us: 100,
            step_us: 100,
            precision_us: 1,
        },
        1,
        &mut budget,
    );
    assert_eq!(result.unwrap_err(), GenerationError::TooManySteps);
    assert_eq!(provider.0.get(), 2);
    assert_eq!(budget, 0);
}

#[test]
fn quiet_leaf_endpoint_samples_consume_budget() {
    for (initial, succeeds, calls) in [(4, false, 8), (5, true, 10)] {
        let provider = CountingProvider(Cell::new(0));
        let mut budget = initial;
        let result = adaptive_exact_aspect_scan(
            &provider,
            &BodyId::try_from("Mars").unwrap(),
            &BodyId::try_from("Venus").unwrap(),
            SearchWindow {
                start_us: 0,
                end_us: 100,
                step_us: 100,
                precision_us: 1,
            },
            1,
            &mut budget,
        );
        assert_eq!(result.is_ok(), succeeds);
        if let Err(error) = result {
            assert_eq!(error, GenerationError::TooManySteps);
        }
        assert_eq!(provider.0.get(), calls);
        assert_eq!(budget, 0);
    }
}

struct CrossingProvider(Cell<usize>);

impl LongitudeProvider for CrossingProvider {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        self.0.set(self.0.get() + 1);
        Ok(LongitudeSample {
            longitude_degrees: if body.as_str() == "Mars" {
                0.0
            } else {
                59.0 + 2.0 * (at.as_i64() as f64) / 100.0
            },
            longitude_speed_degrees_per_day: 0.0,
        })
    }
}

#[test]
fn root_refinement_cannot_bypass_the_shared_budget() {
    let provider = CrossingProvider(Cell::new(0));
    let mut budget = 5;
    let result = adaptive_exact_aspect_scan(
        &provider,
        &BodyId::try_from("Mars").unwrap(),
        &BodyId::try_from("Venus").unwrap(),
        SearchWindow {
            start_us: 0,
            end_us: 100,
            step_us: 100,
            precision_us: 1,
        },
        1,
        &mut budget,
    );
    assert_eq!(result.unwrap_err(), GenerationError::TooManySteps);
    assert_eq!(provider.0.get(), 10);
    assert_eq!(budget, 0);
}

struct FailingSecondProvider(Cell<usize>);

impl LongitudeProvider for FailingSecondProvider {
    fn sample(&self, body: &BodyId, _at: TimestampMicros) -> Result<LongitudeSample, String> {
        self.0.set(self.0.get() + 1);
        if body.as_str() == "Venus" {
            return Err("synthetic provider failure".to_owned());
        }
        Ok(LongitudeSample {
            longitude_degrees: 0.0,
            longitude_speed_degrees_per_day: 0.0,
        })
    }
}

#[test]
fn provider_failure_consumes_the_started_pair_probe_and_returns_no_report() {
    let provider = FailingSecondProvider(Cell::new(0));
    let mut budget = 4;
    let result = adaptive_exact_aspect_scan(
        &provider,
        &BodyId::try_from("Mars").unwrap(),
        &BodyId::try_from("Venus").unwrap(),
        SearchWindow {
            start_us: 0,
            end_us: 100,
            step_us: 100,
            precision_us: 1,
        },
        1,
        &mut budget,
    );
    assert!(matches!(result, Err(GenerationError::Provider(_))));
    assert_eq!(provider.0.get(), 2);
    assert_eq!(budget, 3);
}

struct FailingAfterCrossing(Cell<usize>);

impl LongitudeProvider for FailingAfterCrossing {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        self.0.set(self.0.get() + 1);
        if body.as_str() == "Venus" && at.as_i64() == 200 {
            return Err("synthetic later failure".to_string());
        }
        Ok(LongitudeSample {
            longitude_degrees: if body.as_str() == "Mars" {
                0.0
            } else {
                59.0 + (at.as_i64() as f64) * 0.02
            },
            longitude_speed_degrees_per_day: 0.0,
        })
    }
}

#[test]
fn later_provider_failure_never_exposes_earlier_candidates() {
    let provider = FailingAfterCrossing(Cell::new(0));
    let mars = BodyId::try_from("Mars").unwrap();
    let venus = BodyId::try_from("Venus").unwrap();
    let mut window = SearchWindow {
        start_us: 0,
        end_us: 100,
        step_us: 100,
        precision_us: 1,
    };
    assert_eq!(
        exact_aspect_candidates(&provider, &mars, &venus, window)
            .unwrap()
            .len(),
        1
    );
    window.end_us = 200;
    assert_eq!(
        exact_aspect_candidates(&provider, &mars, &venus, window).unwrap_err(),
        GenerationError::Provider("synthetic later failure".to_string()),
    );
    provider.0.set(0);
    let mut budget = 100;
    assert_eq!(
        adaptive_exact_aspect_scan(&provider, &mars, &venus, window, 1, &mut budget).unwrap_err(),
        GenerationError::Provider("synthetic later failure".to_string()),
    );
    assert_eq!(provider.0.get(), 2 * (100 - budget));
    assert!(provider.0.get() > 4);
}
