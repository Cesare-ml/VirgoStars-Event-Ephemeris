/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Experimental local extrema of signed geocentric ecliptic-latitude speed.
//!
//! These are *candidates*, not authoritative astronomical events. A coarse
//! three-point scan can miss hidden peaks or alias a multi-modal interval;
//! the refinement assumes unimodality within an observed candidate bracket.
//! No data is written to the source archive or exposed by the offline reader.

use std::collections::BTreeMap;
use virgostars_events_core::BodyId;

use crate::{
    latitude_crossings::checked_latitude_speed, midpoint, needs_refinement, next_time,
    GenerationError, LatitudeProvider, SearchWindow,
};

/// A maximum of northward signed speed or maximum magnitude southward speed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatitudeSpeedExtremumKind {
    /// Positive local maximum of `dβ/dt`.
    NorthwardMaximum,
    /// Negative local minimum of `dβ/dt` (greatest southward magnitude).
    SouthwardMagnitudeMaximum,
}

/// A time bracket around a *possible* local peak and its measured signed speed.
#[derive(Clone, Debug, PartialEq)]
pub struct LatitudeSpeedExtremumCandidate {
    pub body: BodyId,
    pub kind: LatitudeSpeedExtremumKind,
    pub bracket_start_us: i64,
    pub bracket_end_us: i64,
    /// Signed geocentric ecliptic-latitude speed, degrees/day.
    pub sampled_signed_speed_degrees_per_day: f64,
}

/// Diagnostics for a bounded, heuristic quarter-point latitude-speed rescan.
/// These counters and candidates are not a completeness certificate.
#[derive(Debug, Default)]
pub struct AdaptiveLatitudeSpeedExtremaReport {
    pub candidates: Vec<LatitudeSpeedExtremumCandidate>,
    /// Unique probes inside coarse grid intervals (excludes refinement probes).
    pub interior_probes: usize,
    pub refined_segments: usize,
    pub leaf_segments: usize,
}

fn classify_three_speeds(
    before: f64,
    center: f64,
    after: f64,
) -> Result<Option<LatitudeSpeedExtremumKind>, GenerationError> {
    if before == center || center == after {
        // A sampled speed plateau can conceal arbitrarily many true peaks.
        return Err(GenerationError::AmbiguousLatitudeSpeedExtremum);
    }
    Ok(if before < center && center > after && center > 0.0 {
        Some(LatitudeSpeedExtremumKind::NorthwardMaximum)
    } else if before > center && center < after && center < 0.0 {
        Some(LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum)
    } else {
        None
    })
}

fn refine_sampled_peak<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    bracket: (i64, i64),
    precision_us: i64,
    kind: LatitudeSpeedExtremumKind,
    remaining_probes: &mut usize,
) -> Result<LatitudeSpeedExtremumCandidate, GenerationError> {
    let (mut left, mut right) = bracket;
    while needs_refinement(left, right, precision_us) {
        let width = right as i128 - left as i128;
        // Integer-ratio golden-section probes keep large signed UTC epochs
        // precise without converting their entire value to floating point.
        let first = (left as i128 + width * 382 / 1000) as i64;
        let second = (left as i128 + width * 618 / 1000) as i64;
        if first <= left || second >= right || first >= second {
            break;
        }
        let first_speed = checked_latitude_speed(provider, body, first, remaining_probes)?;
        let second_speed = checked_latitude_speed(provider, body, second, remaining_probes)?;
        let move_right = match kind {
            LatitudeSpeedExtremumKind::NorthwardMaximum => first_speed < second_speed,
            LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum => first_speed > second_speed,
        };
        if move_right {
            left = first;
        } else {
            right = second;
        }
    }
    let speed = checked_latitude_speed(provider, body, midpoint(left, right), remaining_probes)?;
    if (kind == LatitudeSpeedExtremumKind::NorthwardMaximum && speed <= 0.0)
        || (kind == LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum && speed >= 0.0)
    {
        return Err(GenerationError::AmbiguousLatitudeSpeedExtremum);
    }
    Ok(LatitudeSpeedExtremumCandidate {
        body: body.clone(),
        kind,
        bracket_start_us: left,
        bracket_end_us: right,
        sampled_signed_speed_degrees_per_day: speed,
    })
}

/// Find observed local peaks of signed latitude speed on `[start_us, end_us)`.
///
/// Strict three-point slope reversals distinguish positive northward maxima
/// from negative southward minima. A local unimodal refinement narrows each
/// observed bracket to the requested UTC precision and returns the speed
/// sampled at its midpoint. The caller owns the *total* provider-probe budget.
/// Adjacent equal speed samples, invalid positions/speeds, an insufficient
/// scan grid and budget exhaustion fail closed. Endpoint-only peaks and two
/// extrema hidden within a coarse sample interval are not discoverable here:
/// **a successful scan never certifies per-body temporal coverage.**
pub fn latitude_speed_extremum_candidates<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    remaining_probes: &mut usize,
) -> Result<Vec<LatitudeSpeedExtremumCandidate>, GenerationError> {
    window.validate()?;
    let mut before_at = window.start_us;
    let mut before_speed = checked_latitude_speed(provider, body, before_at, remaining_probes)?;
    let mut center_at = next_time(before_at, window);
    if center_at >= window.end_us {
        return Err(GenerationError::InsufficientSpeedSamples);
    }
    let mut center_speed = checked_latitude_speed(provider, body, center_at, remaining_probes)?;
    let mut result = Vec::new();

    while center_at < window.end_us {
        let after_at = next_time(center_at, window);
        let after_speed = checked_latitude_speed(provider, body, after_at, remaining_probes)?;
        if let Some(kind) = classify_three_speeds(before_speed, center_speed, after_speed)? {
            result.push(refine_sampled_peak(
                provider,
                body,
                (before_at, after_at),
                window.precision_us,
                kind,
                remaining_probes,
            )?);
        }
        before_at = center_at;
        before_speed = center_speed;
        center_at = after_at;
        center_speed = after_speed;
    }
    result.sort_by_key(|candidate| candidate.bracket_start_us);
    Ok(result)
}

fn suspicious_latitude_speed_half(before: f64, mid: f64, after: f64) -> bool {
    let reversal = (before < mid && mid > after) || (before > mid && mid < after);
    let secant_mid = before / 2.0 + after / 2.0;
    reversal || (mid - secant_mid).abs() > (after - before).abs() / 2.0
}

struct AdaptiveLatitudeSpeedScan<'a, P> {
    provider: &'a P,
    body: &'a BodyId,
    remaining_probes: &'a mut usize,
    sampled: BTreeMap<i64, f64>,
    report: AdaptiveLatitudeSpeedExtremaReport,
}

impl<P: LatitudeProvider> AdaptiveLatitudeSpeedScan<'_, P> {
    fn sample(&mut self, at_us: i64, interior: bool) -> Result<f64, GenerationError> {
        if let Some(speed) = self.sampled.get(&at_us) {
            return Ok(*speed);
        }
        let speed = checked_latitude_speed(self.provider, self.body, at_us, self.remaining_probes)?;
        self.sampled.insert(at_us, speed);
        if interior {
            self.report.interior_probes += 1;
        }
        Ok(speed)
    }

    fn segment(
        &mut self,
        start_us: i64,
        end_us: i64,
        start_speed: f64,
        end_speed: f64,
        depth_remaining: u8,
        precision_us: i64,
    ) -> Result<(), GenerationError> {
        let span = end_us as i128 - start_us as i128;
        if depth_remaining == 0 || span <= precision_us as i128 * 4 {
            self.report.leaf_segments += 1;
            return Ok(());
        }
        let quarter = (start_us as i128 + span / 4) as i64;
        let center = midpoint(start_us, end_us);
        let three_quarters = (start_us as i128 + span * 3 / 4) as i64;
        if !(start_us < quarter
            && quarter < center
            && center < three_quarters
            && three_quarters < end_us)
        {
            self.report.leaf_segments += 1;
            return Ok(());
        }

        // Quarters, not just the midpoint: two opposing extrema can both be
        // invisible at the endpoints and midpoint of the coarse interval.
        let q1 = self.sample(quarter, true)?;
        let mid = self.sample(center, true)?;
        let q3 = self.sample(three_quarters, true)?;
        for (left, right, a, b, c) in [
            (start_us, center, start_speed, q1, mid),
            (center, end_us, mid, q3, end_speed),
        ] {
            if depth_remaining > 1 && suspicious_latitude_speed_half(a, b, c) {
                self.report.refined_segments += 1;
                self.segment(left, right, a, c, depth_remaining - 1, precision_us)?;
            } else {
                self.report.leaf_segments += 1;
            }
        }
        Ok(())
    }
}

/// Bounded, **uncertified** quarter-point latitude-speed rescan.
///
/// Each coarse step gets 1/4, 1/2 and 3/4 samples. A sampled turning point or
/// pronounced secant deviation prompts recursive half-interval inspection,
/// with `max_depth` in 1..=8. Chronologically deduplicated speed samples are
/// classified into strict signed peaks and refined under the *same* caller
/// budget. An empty report or two agreeing scans cannot prove completeness:
/// sub-quarter or periodic peaks can still be missed. The window is half-open;
/// the end instant is used only as a probe, never as a candidate center.
pub fn adaptive_latitude_speed_extrema_scan<P: LatitudeProvider>(
    provider: &P,
    body: &BodyId,
    window: SearchWindow,
    max_depth: u8,
    remaining_probes: &mut usize,
) -> Result<AdaptiveLatitudeSpeedExtremaReport, GenerationError> {
    window.validate()?;
    if !(1..=8).contains(&max_depth)
        || (window.step_us as i128) / (1_i128 << (max_depth as u32 + 2))
            < window.precision_us as i128
    {
        return Err(GenerationError::InvalidWindow);
    }

    let mut scan = AdaptiveLatitudeSpeedScan {
        provider,
        body,
        remaining_probes,
        sampled: BTreeMap::new(),
        report: AdaptiveLatitudeSpeedExtremaReport::default(),
    };
    let mut start = window.start_us;
    let mut start_speed = scan.sample(start, false)?;
    while start < window.end_us {
        let end = next_time(start, window);
        let end_speed = scan.sample(end, false)?;
        scan.segment(
            start,
            end,
            start_speed,
            end_speed,
            max_depth,
            window.precision_us,
        )?;
        start = end;
        start_speed = end_speed;
    }

    let ordered: Vec<_> = scan.sampled.into_iter().collect();
    if ordered.len() < 3 {
        return Err(GenerationError::InsufficientSpeedSamples);
    }
    for triple in ordered.windows(3) {
        let [(first_at, first), (_, center), (last_at, last)] = triple else {
            unreachable!("windows(3) yields exactly three items")
        };
        if let Some(kind) = classify_three_speeds(*first, *center, *last)? {
            scan.report.candidates.push(refine_sampled_peak(
                provider,
                body,
                (*first_at, *last_at),
                window.precision_us,
                kind,
                scan.remaining_probes,
            )?);
        }
    }
    scan.report
        .candidates
        .sort_by_key(|candidate| candidate.bracket_start_us);
    Ok(scan.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LatitudeSample;
    use virgostars_events_core::TimestampMicros;

    const DAY: i64 = 86_400_000_000;

    struct Model<F>(F);
    impl<F: Fn(f64) -> (f64, f64)> LatitudeProvider for Model<F> {
        fn sample_latitude(
            &self,
            _body: &BodyId,
            at: TimestampMicros,
        ) -> Result<LatitudeSample, String> {
            let (latitude, speed) = (self.0)(at.as_i64() as f64 / DAY as f64);
            Ok(LatitudeSample {
                latitude_degrees: latitude,
                latitude_speed_degrees_per_day: speed,
            })
        }
    }

    fn body() -> BodyId {
        BodyId::try_from("Mars").unwrap()
    }

    fn window(end: i64, step: i64) -> SearchWindow {
        SearchWindow {
            start_us: 0,
            end_us: end,
            step_us: step,
            precision_us: 1_000_000,
        }
    }

    #[test]
    fn positive_northward_speed_peak_is_refined() {
        let model = Model(|d: f64| (2.0 * d - (d - 1.2).powi(3) / 3.0, 2.0 - (d - 1.2).powi(2)));
        let mut budget = 100;
        let candidates =
            latitude_speed_extremum_candidates(&model, &body(), window(3 * DAY, DAY), &mut budget)
                .unwrap();
        assert_eq!(candidates.len(), 1);
        let peak = &candidates[0];
        assert_eq!(peak.kind, LatitudeSpeedExtremumKind::NorthwardMaximum);
        assert!(peak.bracket_start_us <= DAY * 12 / 10);
        assert!(DAY * 12 / 10 <= peak.bracket_end_us);
        assert!(peak.bracket_end_us - peak.bracket_start_us <= 1_000_000);
        assert!((peak.sampled_signed_speed_degrees_per_day - 2.0).abs() < 0.00001);
    }

    #[test]
    fn negative_southward_speed_peak_preserves_its_sign() {
        let model = Model(|d: f64| (-2.0 * d + (d - 1.2).powi(3) / 3.0, -2.0 + (d - 1.2).powi(2)));
        let mut budget = 100;
        let peaks =
            latitude_speed_extremum_candidates(&model, &body(), window(3 * DAY, DAY), &mut budget)
                .unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(
            peaks[0].kind,
            LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum
        );
        assert!(peaks[0].sampled_signed_speed_degrees_per_day < -1.99999);
        assert!(peaks[0].bracket_start_us <= DAY * 12 / 10);
        assert!(DAY * 12 / 10 <= peaks[0].bracket_end_us);
    }

    #[test]
    fn positive_minimum_and_negative_maximum_are_not_target_kinds() {
        for peak_sign in [-1.0, 1.0] {
            let model = Model(move |d: f64| {
                (
                    peak_sign * (d - 1.0).powi(3) / 3.0,
                    peak_sign * (d - 1.0).powi(2),
                )
            });
            let mut budget = 100;
            assert!(latitude_speed_extremum_candidates(
                &model,
                &body(),
                window(2 * DAY, DAY),
                &mut budget,
            )
            .unwrap()
            .is_empty());
        }
    }

    #[test]
    fn constant_speed_plateau_is_rejected() {
        let model = Model(|d: f64| (d, 1.0));
        let mut budget = 20;
        assert_eq!(
            latitude_speed_extremum_candidates(&model, &body(), window(2 * DAY, DAY), &mut budget,),
            Err(GenerationError::AmbiguousLatitudeSpeedExtremum)
        );
    }

    #[test]
    fn strictly_monotone_speed_has_no_peaks() {
        let model = Model(|d: f64| (d * d / 2.0, d));
        let mut budget = 20;
        assert!(latitude_speed_extremum_candidates(
            &model,
            &body(),
            window(3 * DAY, DAY),
            &mut budget,
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn less_than_three_time_points_is_insufficient() {
        let model = Model(|d: f64| (d, 1.0));
        let mut budget = 20;
        assert_eq!(
            latitude_speed_extremum_candidates(&model, &body(), window(DAY, DAY), &mut budget),
            Err(GenerationError::InsufficientSpeedSamples)
        );
    }

    #[test]
    fn peak_at_exclusive_end_or_inclusive_start_is_not_claimed() {
        for peak_at in [0.0, 2.0] {
            let model = Model(move |d: f64| {
                (
                    2.0 * d - (d - peak_at).powi(3) / 3.0,
                    2.0 - (d - peak_at).powi(2),
                )
            });
            let mut budget = 60;
            assert!(latitude_speed_extremum_candidates(
                &model,
                &body(),
                window(2 * DAY, DAY),
                &mut budget,
            )
            .unwrap()
            .is_empty());
        }
    }

    #[test]
    fn insufficient_probe_budget_fails_closed() {
        let model = Model(|d: f64| (2.0 * d - (d - 1.2).powi(3) / 3.0, 2.0 - (d - 1.2).powi(2)));
        let mut budget = 4;
        assert_eq!(
            latitude_speed_extremum_candidates(&model, &body(), window(3 * DAY, DAY), &mut budget,),
            Err(GenerationError::TooManySteps)
        );
        assert_eq!(budget, 0);
    }

    #[test]
    fn invalid_latitude_or_speed_fails_closed() {
        for (lat, speed) in [(f64::NAN, 0.1), (12.0, f64::INFINITY), (90.1, 0.2)] {
            let model = Model(move |_d: f64| (lat, speed));
            let mut budget = 20;
            assert_eq!(
                latitude_speed_extremum_candidates(
                    &model,
                    &body(),
                    window(2 * DAY, DAY),
                    &mut budget,
                ),
                Err(GenerationError::InvalidSample)
            );
        }
    }

    #[test]
    fn two_unseen_extrema_between_equal_coarse_endpoints_are_not_certified() {
        // An even number of extrema can alias to a monotone coarse sequence.
        let model = Model(|d: f64| {
            let phase = 4.0 * std::f64::consts::PI * d;
            (
                (1.0 - phase.cos()) / (4.0 * std::f64::consts::PI),
                phase.sin(),
            )
        });
        let mut budget = 20;
        let result =
            latitude_speed_extremum_candidates(&model, &body(), window(2 * DAY, DAY), &mut budget);
        // Speeds at 0,1,2 days are floating-point approximations of zero;
        // they cannot establish the intervening 1/8 and 3/8-day peaks.
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn adaptive_quarter_probes_reveal_hidden_northward_and_southward_peaks() {
        let model = Model(|d: f64| {
            let phase = 2.0 * std::f64::consts::PI * d;
            (
                (1.0 - phase.cos()) / (2.0 * std::f64::consts::PI),
                phase.sin(),
            )
        });
        let mut budget = 100;
        let coarse =
            latitude_speed_extremum_candidates(&model, &body(), window(2 * DAY, DAY), &mut budget)
                .unwrap();
        assert!(coarse.is_empty());

        let mut budget = 500;
        let report = adaptive_latitude_speed_extrema_scan(
            &model,
            &body(),
            window(2 * DAY, DAY),
            1,
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.candidates.len(), 4);
        assert_eq!(report.interior_probes, 6);
        for (candidate, (day_numerator, kind)) in report.candidates.iter().zip([
            (1, LatitudeSpeedExtremumKind::NorthwardMaximum),
            (3, LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum),
            (5, LatitudeSpeedExtremumKind::NorthwardMaximum),
            (7, LatitudeSpeedExtremumKind::SouthwardMagnitudeMaximum),
        ]) {
            let at = DAY * day_numerator / 4;
            assert_eq!(candidate.kind, kind);
            assert!(candidate.bracket_start_us <= at && at <= candidate.bracket_end_us);
            assert!(candidate.bracket_end_us - candidate.bracket_start_us <= 1_000_000);
            assert!(candidate.sampled_signed_speed_degrees_per_day.abs() > 0.99999);
        }
    }

    #[test]
    fn adaptive_recursion_counts_refinement_and_shares_the_budget() {
        let model = Model(|d: f64| {
            let phase = 2.0 * std::f64::consts::PI * d;
            (
                (1.0 - phase.cos()) / (2.0 * std::f64::consts::PI),
                phase.sin(),
            )
        });
        let mut budget = 500;
        let report = adaptive_latitude_speed_extrema_scan(
            &model,
            &body(),
            window(2 * DAY, DAY),
            3,
            &mut budget,
        )
        .unwrap();
        assert_eq!(report.candidates.len(), 4);
        assert!(report.refined_segments > 0);
        assert!(report.interior_probes > 6);
        assert!(report.leaf_segments >= 4);
        assert!(budget < 500);
    }

    #[test]
    fn adaptive_rejects_invalid_depth_or_too_coarse_resolution() {
        let model = Model(|d: f64| (d * d / 2.0, d));
        for depth in [0, 9] {
            let mut budget = 100;
            assert_eq!(
                adaptive_latitude_speed_extrema_scan(
                    &model,
                    &body(),
                    window(2 * DAY, DAY),
                    depth,
                    &mut budget,
                )
                .err(),
                Some(GenerationError::InvalidWindow)
            );
            assert_eq!(budget, 100);
        }
        let mut budget = 100;
        assert_eq!(
            adaptive_latitude_speed_extrema_scan(
                &model,
                &body(),
                window(5_000_000, 1_000_000),
                2,
                &mut budget,
            )
            .err(),
            Some(GenerationError::InvalidWindow)
        );
    }

    #[test]
    fn adaptive_budget_exhaustion_is_explicit() {
        let model = Model(|d: f64| (d * d / 2.0, d));
        let mut budget = 3;
        assert_eq!(
            adaptive_latitude_speed_extrema_scan(
                &model,
                &body(),
                window(2 * DAY, DAY),
                1,
                &mut budget,
            )
            .err(),
            Some(GenerationError::TooManySteps)
        );
        assert_eq!(budget, 0);
    }

    #[test]
    fn adaptive_plateau_fails_closed_and_monotone_speed_is_empty() {
        let flat = Model(|d: f64| (d, 1.0));
        let mut budget = 100;
        assert_eq!(
            adaptive_latitude_speed_extrema_scan(
                &flat,
                &body(),
                window(2 * DAY, DAY),
                1,
                &mut budget,
            )
            .err(),
            Some(GenerationError::AmbiguousLatitudeSpeedExtremum)
        );

        let monotone = Model(|d: f64| (d * d / 2.0, d));
        let mut budget = 100;
        let report = adaptive_latitude_speed_extrema_scan(
            &monotone,
            &body(),
            window(2 * DAY, DAY),
            1,
            &mut budget,
        )
        .unwrap();
        assert!(report.candidates.is_empty());
    }
}
