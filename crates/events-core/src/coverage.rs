/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0); see LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Distributed AS IS, without warranty.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Event completeness must be independently reviewed; an index of recorded
//! event dates is never proof of a continuous astronomical state.
//!
//! This policy is deliberately data-format-agnostic. The file-backed adapter
//! MUST verify its archive digest before accepting a reviewed policy.

use std::error::Error;
use std::fmt;

use crate::{BodyId, EventFamily, TimestampMicros};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageError {
    UnsupportedStateFamily,
    InvalidInterval,
    MissingReviewEvidence,
    OverlappingIntervals,
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedStateFamily => f.write_str("coverage windows are only for motion or ingress states"),
            Self::InvalidInterval => f.write_str("coverage window has an empty or inverted interval"),
            Self::MissingReviewEvidence => f.write_str("coverage window requires a reviewer and evidence"),
            Self::OverlappingIntervals => f.write_str("overlapping reviewed coverage intervals for one family/body"),
        }
    }
}

impl Error for CoverageError {}

/// A human-reviewed *complete* event range for one family and one body.
///
/// Windows have [start, end) semantics. The fields are intentionally private:
/// use new() to reject invalid ranges. The caller is responsible for actually
/// reviewing the evidence; source code cannot certify scientific completeness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewedWindow {
    family: EventFamily,
    body: BodyId,
    start: TimestampMicros,
    end_exclusive: TimestampMicros,
    reviewer: String,
    evidence: String,
}

impl ReviewedWindow {
    pub fn new(
        family: EventFamily,
        body: BodyId,
        start: TimestampMicros,
        end_exclusive: TimestampMicros,
        reviewer: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Result<Self, CoverageError> {
        if !matches!(family, EventFamily::Motion | EventFamily::Ingress) {
            return Err(CoverageError::UnsupportedStateFamily);
        }
        if start >= end_exclusive {
            return Err(CoverageError::InvalidInterval);
        }
        let reviewer = reviewer.into();
        let evidence = evidence.into();
        if reviewer.trim().is_empty() || evidence.trim().is_empty() {
            return Err(CoverageError::MissingReviewEvidence);
        }
        Ok(Self { family, body, start, end_exclusive, reviewer, evidence })
    }

    pub const fn family(&self) -> EventFamily { self.family }
    pub fn body(&self) -> &BodyId { &self.body }
    pub const fn start(&self) -> TimestampMicros { self.start }
    pub const fn end_exclusive(&self) -> TimestampMicros { self.end_exclusive }
    pub fn reviewer(&self) -> &str { &self.reviewer }
    pub fn evidence(&self) -> &str { &self.evidence }

    pub fn contains(&self, instant: TimestampMicros) -> bool {
        self.start <= instant && instant < self.end_exclusive
    }
}

/// An explicit distinction between last RECORDED evidence and a reviewed state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageAssessment<'a> {
    /// No reviewed window covers the requested instant.
    UnknownCoverage,
    /// A reviewed window exists, but no event within it establishes state.
    UnknownInitialState,
    /// An event at/after the reviewed window start establishes state at instant.
    Reviewed(&'a ReviewedWindow),
}

#[derive(Clone, Debug, Default)]
pub struct CoveragePolicy {
    windows: Vec<ReviewedWindow>,
}

impl CoveragePolicy {
    pub fn new(mut windows: Vec<ReviewedWindow>) -> Result<Self, CoverageError> {
        windows.sort_by(|a, b| {
            (a.family, &a.body, a.start).cmp(&(b.family, &b.body, b.start))
        });
        for pair in windows.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            if a.family == b.family && a.body == b.body && a.end_exclusive > b.start {
                return Err(CoverageError::OverlappingIntervals);
            }
        }
        Ok(Self { windows })
    }

    pub fn windows(&self) -> &[ReviewedWindow] { &self.windows }

    pub fn find(
        &self,
        family: EventFamily,
        body: &BodyId,
        instant: TimestampMicros,
    ) -> Option<&ReviewedWindow> {
        self.windows.iter().find(|window| {
            window.family == family && &window.body == body && window.contains(instant)
        })
    }

    /// Assess time coverage from an authoritative *last recorded event* lookup.
    ///
    /// Passing the most recent event's time is not alone sufficient: an event
    /// strictly before the reviewed interval does not establish its state.
    pub fn assess(
        &self,
        family: EventFamily,
        body: &BodyId,
        instant: TimestampMicros,
        last_recorded_at: Option<TimestampMicros>,
    ) -> CoverageAssessment<'_> {
        let Some(window) = self.find(family, body, instant) else {
            return CoverageAssessment::UnknownCoverage;
        };
        match last_recorded_at {
            Some(at) if window.start <= at && at <= instant => CoverageAssessment::Reviewed(window),
            _ => CoverageAssessment::UnknownInitialState,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moon() -> BodyId {
        BodyId::try_from("Moon").unwrap()
    }

    fn window(family: EventFamily, start: i64, end: i64) -> ReviewedWindow {
        ReviewedWindow::new(
            family,
            moon(),
            TimestampMicros::new(start),
            TimestampMicros::new(end),
            "independent-reviewer",
            "reproducible synthetic test evidence",
        ).unwrap()
    }

    #[test]
    fn empty_policy_is_explicitly_unknown() {
        let policy = CoveragePolicy::default();
        assert_eq!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(10), Some(TimestampMicros::new(0))),
            CoverageAssessment::UnknownCoverage,
        );
    }

    #[test]
    fn boundaries_are_start_inclusive_end_exclusive() {
        let policy = CoveragePolicy::new(vec![window(EventFamily::Ingress, 0, 20)]).unwrap();
        assert!(matches!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(0), Some(TimestampMicros::new(0))),
            CoverageAssessment::Reviewed(_),
        ));
        assert!(matches!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(19), Some(TimestampMicros::new(10))),
            CoverageAssessment::Reviewed(_),
        ));
        assert_eq!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(20), Some(TimestampMicros::new(10))),
            CoverageAssessment::UnknownCoverage,
        );
    }

    #[test]
    fn previous_window_event_cannot_establish_current_state() {
        let policy = CoveragePolicy::new(vec![window(EventFamily::Ingress, 11, 1000)]).unwrap();
        assert_eq!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(500), Some(TimestampMicros::new(10))),
            CoverageAssessment::UnknownInitialState,
        );
        assert!(matches!(
            policy.assess(EventFamily::Ingress, &moon(), TimestampMicros::new(700), Some(TimestampMicros::new(600))),
            CoverageAssessment::Reviewed(_),
        ));
    }

    #[test]
    fn motion_and_ingress_coverage_are_independent() {
        let policy = CoveragePolicy::new(vec![window(EventFamily::Ingress, 0, 20)]).unwrap();
        assert_eq!(
            policy.assess(EventFamily::Motion, &moon(), TimestampMicros::new(5), Some(TimestampMicros::new(0))),
            CoverageAssessment::UnknownCoverage,
        );
    }

    #[test]
    fn invalid_windows_and_overlaps_are_rejected() {
        assert_eq!(
            ReviewedWindow::new(EventFamily::Synod, moon(), TimestampMicros::new(0), TimestampMicros::new(10), "reviewer", "evidence"),
            Err(CoverageError::UnsupportedStateFamily),
        );
        assert_eq!(
            ReviewedWindow::new(EventFamily::Ingress, moon(), TimestampMicros::new(10), TimestampMicros::new(10), "reviewer", "evidence"),
            Err(CoverageError::InvalidInterval),
        );
        assert_eq!(
            ReviewedWindow::new(EventFamily::Ingress, moon(), TimestampMicros::new(0), TimestampMicros::new(10), "", "evidence"),
            Err(CoverageError::MissingReviewEvidence),
        );
        assert!(matches!(
            CoveragePolicy::new(vec![window(EventFamily::Ingress, 0, 20), window(EventFamily::Ingress, 10, 30)]),
            Err(CoverageError::OverlappingIntervals),
        ));
        // Adjacent [0,20) and [20,30) intervals are not overlapping.
        assert!(CoveragePolicy::new(vec![window(EventFamily::Ingress, 20, 30), window(EventFamily::Ingress, 0, 20)]).is_ok());
    }
}
