/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! Deterministic stage labels for *recorded* synod cycle phase boundaries.
//!
//! This is not astronomical interpolation and does NOT attest that the source
//! dataset contains every actual synod. Coverage is reviewed separately.

use crate::{SynodEvent, TimestampMicros};

/// Conservative source-data discontinuity guard for consecutive Moon anchors.
///
/// In the currently authorized owner snapshot, all but two consecutive
/// recorded Moon intervals are below 30 days; two gaps extend 383/413 days.
/// 45 days flags only clear continuity anomalies, not astronomical completeness.
pub const MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_MICROS: i64 = 45 * 86_400 * 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynodStage {
    D1,
    R2,
    R3,
    D4,
    Cresc1,
    Cresc2,
    Cal1,
    Cal2,
}

impl SynodStage {
    pub const fn code(self) -> &'static str {
        match self {
            Self::D1 => "D1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::D4 => "D4",
            Self::Cresc1 => "CRESC1",
            Self::Cresc2 => "CRESC2",
            Self::Cal1 => "CAL1",
            Self::Cal2 => "CAL2",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynodStageStatus {
    /// Based only on stored event/extra timestamps, NOT certified coverage.
    DerivedFromRecordedThresholds,
    BeforeRecordedAnchor,
    AtOrAfterNextRecordedAnchor,
    MissingPhaseThresholds,
    IncompletePhaseThresholds,
    UnsupportedBody,
    InvalidCycleBounds,
    /// Fourth stage cannot safely extend past an unknown next anchor.
    UnknownOpenEndedCycle,
    /// The next recorded new moon lies implausibly far away: intervening
    /// lunar cycles are absent from the source snapshot.
    DiscontinuousMoonAnchors,
}

impl SynodStageStatus {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DerivedFromRecordedThresholds => "derived_from_recorded_thresholds",
            Self::BeforeRecordedAnchor => "before_recorded_anchor",
            Self::AtOrAfterNextRecordedAnchor => "at_or_after_next_recorded_anchor",
            Self::MissingPhaseThresholds => "unknown_missing_phase_thresholds",
            Self::IncompletePhaseThresholds => "unknown_incomplete_phase_thresholds",
            Self::UnsupportedBody => "unknown_unsupported_phase_convention",
            Self::InvalidCycleBounds => "unknown_invalid_cycle_bounds",
            Self::UnknownOpenEndedCycle => "unknown_open_ended_cycle",
            Self::DiscontinuousMoonAnchors => "unknown_discontinuous_moon_anchors",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SynodStageAssessment {
    pub stage: Option<SynodStage>,
    pub started_at: Option<TimestampMicros>,
    pub status: SynodStageStatus,
}

impl SynodStageAssessment {
    fn unknown(status: SynodStageStatus) -> Self {
        Self {
            stage: None,
            started_at: None,
            status,
        }
    }
}

/// Returns the four-part stage at an instant using the source's three
/// strictly increasing extra timestamps. A known next anchor acts as an
/// exclusive cycle end. This never derives astronomical completeness.
///
/// For Moon: CRESC1, CRESC2, CAL1, CAL2. For the nine supported
/// planetary/solar bodies: D1, R2, R3, D4. For other bodies, there is no
/// universal four-stage convention. Missing or incomplete phase extras
/// are not guessed. Even a known stage is "recorded evidence" only.
pub fn assess_synod_stage(
    event: &SynodEvent,
    next_recorded_start: Option<TimestampMicros>,
    requested_at: TimestampMicros,
) -> SynodStageAssessment {
    if requested_at < event.start() {
        return SynodStageAssessment::unknown(SynodStageStatus::BeforeRecordedAnchor);
    }
    if let Some(next) = next_recorded_start {
        if next <= event.start() {
            return SynodStageAssessment::unknown(SynodStageStatus::InvalidCycleBounds);
        }
        if requested_at >= next {
            return SynodStageAssessment::unknown(
                SynodStageStatus::AtOrAfterNextRecordedAnchor,
            );
        }
    }

    let moon = event.body().as_str() == "Moon";
    if !moon
        && !matches!(
            event.body().as_str(),
            "Mercury"
                | "Venus"
                | "Sun"
                | "Mars"
                | "Jupiter"
                | "Saturn"
                | "Uranus"
                | "Neptune"
                | "Pluto"
        )
    {
        return SynodStageAssessment::unknown(SynodStageStatus::UnsupportedBody);
    }

    let Some(phases) = event.phases() else {
        return SynodStageAssessment::unknown(SynodStageStatus::MissingPhaseThresholds);
    };
    if phases.len() != 3 {
        return SynodStageAssessment::unknown(SynodStageStatus::IncompletePhaseThresholds);
    }
    let boundaries = [phases[0].at(), phases[1].at(), phases[2].at()];
    if boundaries[0] <= event.start()
        || boundaries[1] <= boundaries[0]
        || boundaries[2] <= boundaries[1]
        || next_recorded_start.is_some_and(|next| boundaries[2] >= next)
    {
        return SynodStageAssessment::unknown(SynodStageStatus::InvalidCycleBounds);
    }

    let (phase, started_at) = if requested_at < boundaries[0] {
        (0, event.start())
    } else if requested_at < boundaries[1] {
        (1, boundaries[0])
    } else if requested_at < boundaries[2] {
        (2, boundaries[1])
    } else {
        let Some(next) = next_recorded_start else {
            // There is no known end for the last segment. Do not extend
            // a recorded last cycle indefinitely into the future.
            return SynodStageAssessment::unknown(SynodStageStatus::UnknownOpenEndedCycle);
        };
        if moon
            && next
                .as_i64()
                .saturating_sub(event.start().as_i64())
                > MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_MICROS
        {
            // Do not call 1919/2018 "CAL2" for many months because the
            // source omits intervening Moon cycles. The recorded anchor is
            // still returned independently by the file-backed reader.
            return SynodStageAssessment::unknown(
                SynodStageStatus::DiscontinuousMoonAnchors,
            );
        }
        (3, boundaries[2])
    };
    let stage = match (moon, phase) {
        (false, 0) => SynodStage::D1,
        (false, 1) => SynodStage::R2,
        (false, 2) => SynodStage::R3,
        (false, _) => SynodStage::D4,
        (true, 0) => SynodStage::Cresc1,
        (true, 1) => SynodStage::Cresc2,
        (true, 2) => SynodStage::Cal1,
        (true, _) => SynodStage::Cal2,
    };
    SynodStageAssessment {
        stage: Some(stage),
        started_at: Some(started_at),
        status: SynodStageStatus::DerivedFromRecordedThresholds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyId, SynodPhase};

    fn synod(body: &str, extras: Option<Vec<i64>>) -> SynodEvent {
        SynodEvent::new(
            BodyId::try_from(body).unwrap(),
            TimestampMicros::new(100),
            extras.map(|times| {
                times
                    .into_iter()
                    .enumerate()
                    .map(|(i, at)| {
                        SynodPhase::new(format!("extra_{}", i + 1), TimestampMicros::new(at))
                            .unwrap()
                    })
                    .collect()
            }),
        )
        .unwrap()
    }

    fn assess(
        body: &str,
        extras: Option<Vec<i64>>,
        next: Option<i64>,
        t: i64,
    ) -> SynodStageAssessment {
        assess_synod_stage(
            &synod(body, extras),
            next.map(TimestampMicros::new),
            TimestampMicros::new(t),
        )
    }

    #[test]
    fn planetary_boundaries_are_inclusive_at_each_new_stage() {
        let expected = [
            (100, SynodStage::D1, 100),
            (109, SynodStage::D1, 100),
            (110, SynodStage::R2, 110),
            (119, SynodStage::R2, 110),
            (120, SynodStage::R3, 120),
            (129, SynodStage::R3, 120),
            (130, SynodStage::D4, 130),
            (199, SynodStage::D4, 130),
        ];
        for (t, code, begin) in expected {
            let result = assess("Mercury", Some(vec![110, 120, 130]), Some(200), t);
            assert_eq!(result.stage, Some(code), "at {t}");
            assert_eq!(result.started_at, Some(TimestampMicros::new(begin)));
            assert_eq!(result.status, SynodStageStatus::DerivedFromRecordedThresholds);
            assert_eq!(result.stage.unwrap().code(), code.code());
        }
        assert_eq!(
            assess("Mercury", Some(vec![110, 120, 130]), Some(200), 200).status,
            SynodStageStatus::AtOrAfterNextRecordedAnchor
        );
    }

    #[test]
    fn moon_stage_boundaries_match_four_recorded_quarter_stages() {
        for (t, code) in [
            (100, SynodStage::Cresc1),
            (109, SynodStage::Cresc1),
            (110, SynodStage::Cresc2),
            (120, SynodStage::Cal1),
            (130, SynodStage::Cal2),
            (199, SynodStage::Cal2),
        ] {
            assert_eq!(
                assess("Moon", Some(vec![110, 120, 130]), Some(200), t).stage,
                Some(code)
            );
        }
    }

    #[test]
    fn null_empty_partial_and_unsupported_bodies_never_default_to_d1() {
        assert_eq!(
            assess("Mercury", None, Some(200), 100).status,
            SynodStageStatus::MissingPhaseThresholds
        );
        for phases in [vec![], vec![110], vec![110, 120], vec![110, 120, 130, 140]] {
            assert_eq!(
                assess("Mercury", Some(phases), Some(200), 100).status,
                SynodStageStatus::IncompletePhaseThresholds
            );
        }
        assert_eq!(
            assess("true Node", Some(vec![110, 120, 130]), Some(200), 115).status,
            SynodStageStatus::UnsupportedBody
        );
    }

    #[test]
    fn no_next_cycle_must_not_assume_an_unbounded_last_phase() {
        let phases = Some(vec![110, 120, 130]);
        assert_eq!(
            assess("Moon", phases.clone(), None, 125).stage,
            Some(SynodStage::Cal1)
        );
        let last = assess("Moon", phases, None, 1_000_000_000);
        assert_eq!(last.stage, None);
        assert_eq!(last.status, SynodStageStatus::UnknownOpenEndedCycle);
    }

    #[test]
    fn long_moon_record_gap_is_not_reported_as_a_year_long_cal2_phase() {
        let next = 100 + 413 * 86_400 * 1_000_000_i64;
        let extras = Some(vec![110, 120, 130]);
        // Earlier source phase thresholds remain recorded evidence.
        assert_eq!(
            assess("Moon", extras.clone(), Some(next), 120).stage,
            Some(SynodStage::Cal1)
        );
        // The fourth stage has no trustworthy end during the source gap.
        let gap = assess("Moon", extras.clone(), Some(next), 131);
        assert_eq!(gap.stage, None);
        assert_eq!(gap.started_at, None);
        assert_eq!(gap.status, SynodStageStatus::DiscontinuousMoonAnchors);
        assert_eq!(gap.status.code(), "unknown_discontinuous_moon_anchors");
        assert_eq!(
            assess("Moon", extras.clone(), Some(next), next - 1).status,
            SynodStageStatus::DiscontinuousMoonAnchors
        );
        // Normal 30-day recorded intervals retain their fourth Moon stage.
        let normal_next = 100 + 30 * 86_400 * 1_000_000_i64;
        assert_eq!(
            assess("Moon", extras, Some(normal_next), 131).stage,
            Some(SynodStage::Cal2)
        );
        // This guard is Moon-specific: planetary synod intervals differ.
        assert_eq!(
            assess("Mars", Some(vec![110, 120, 130]), Some(next), 131).stage,
            Some(SynodStage::D4)
        );
    }

    #[test]
    fn rejects_invalid_cycle_bounds_and_handles_pre_anchor_times() {
        assert_eq!(
            assess("Venus", Some(vec![110, 120, 130]), Some(150), 99).status,
            SynodStageStatus::BeforeRecordedAnchor
        );
        assert_eq!(
            assess("Venus", Some(vec![110, 120, 130]), Some(100), 100).status,
            SynodStageStatus::InvalidCycleBounds
        );
        assert_eq!(
            assess("Venus", Some(vec![110, 120, 130]), Some(125), 115).status,
            SynodStageStatus::InvalidCycleBounds
        );
    }
}
