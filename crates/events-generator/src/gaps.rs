/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Detect **candidate sparse spans**, not proven absent astronomical events.
//! Thresholds are caller-supplied per body/family, never guessed by the library.

use virgostars_events_core::{BodyId, EventFamily, TimestampMicros};
use virgostars_events_p0::{P0Archive, P0Error, P0Record};

const MAX_RECORD_SCANS: usize = 1_000_000;
const MAX_GAPS: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparseSpanKind {
    BetweenRecords,
    LeadingBoundary,
    TrailingBoundary,
    NoRecords,
}

impl SparseSpanKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::BetweenRecords => "between_recorded_anchors",
            Self::LeadingBoundary => "leading_unconfirmed_boundary",
            Self::TrailingBoundary => "trailing_unconfirmed_boundary",
            Self::NoRecords => "no_recorded_anchors_in_window",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseSpan {
    pub kind: SparseSpanKind,
    pub start_us: i64,
    pub end_us: i64,
    pub previous_original_source_id: Option<u64>,
    pub next_original_source_id: Option<u64>,
    pub previous_recorded_us: Option<i64>,
    pub next_recorded_us: Option<i64>,
}

fn timestamp(record: &P0Record) -> i64 {
    record.at().as_i64()
}

fn report_span(
    result: &mut Vec<SparseSpan>,
    kind: SparseSpanKind,
    start: i64,
    end: i64,
    threshold_us: i64,
    previous: Option<&P0Record>,
    next: Option<&P0Record>,
) -> Result<(), P0Error> {
    if end as i128 - start as i128 <= threshold_us as i128 {
        return Ok(());
    }
    if result.len() >= MAX_GAPS {
        return Err(P0Error::Invalid(
            "candidate sparse-span count exceeds limit".into(),
        ));
    }
    result.push(SparseSpan {
        kind,
        start_us: start,
        end_us: end,
        previous_original_source_id: previous.map(|p| p.source_id),
        next_original_source_id: next.map(|p| p.source_id),
        previous_recorded_us: previous.map(timestamp),
        next_recorded_us: next.map(timestamp),
    });
    Ok(())
}

/// Audit a **bounded UTC window** for unexpectedly sparse recorded intervals.
///
/// A threshold must be based on the target family's documented scientific
/// recurrence, e.g. a separately reviewed 45-day *heuristic* for Moon synods.
/// Sparse boundary intervals are **unconfirmed** even if much longer: the
/// archive may simply not cover the requested horizon. Does not mutate P0.
pub fn detect_sparse_spans(
    archive: &P0Archive,
    family: EventFamily,
    body: &BodyId,
    start_us: i64,
    end_us: i64,
    threshold_us: i64,
) -> Result<Vec<SparseSpan>, P0Error> {
    if end_us <= start_us || threshold_us <= 0 {
        return Err(P0Error::Invalid(
            "invalid gap window or family-specific threshold".into(),
        ));
    }

    let mut spans = Vec::new();
    let mut previous = archive.at(family, body, TimestampMicros::new(start_us))?;
    let mut cursor = start_us;
    let mut scanned = 0;

    loop {
        if scanned >= MAX_RECORD_SCANS {
            return Err(P0Error::Invalid("record scan limit exceeded".into()));
        }
        let next = archive.next(family, body, TimestampMicros::new(cursor))?;
        let Some(event) = next else {
            break;
        };
        let at_us = timestamp(&event);
        if at_us <= cursor {
            return Err(P0Error::Invalid(
                "non-monotonic original event timestamps".into(),
            ));
        }
        if at_us > end_us {
            break;
        }

        if let Some(previous_event) = previous.as_ref() {
            report_span(
                &mut spans,
                SparseSpanKind::BetweenRecords,
                timestamp(previous_event).max(start_us),
                at_us,
                threshold_us,
                Some(previous_event),
                Some(&event),
            )?;
        } else {
            report_span(
                &mut spans,
                SparseSpanKind::LeadingBoundary,
                start_us,
                at_us,
                threshold_us,
                None,
                Some(&event),
            )?;
        }

        cursor = at_us;
        previous = Some(event);
        scanned += 1;
    }

    if let Some(previous_event) = previous.as_ref() {
        report_span(
            &mut spans,
            SparseSpanKind::TrailingBoundary,
            timestamp(previous_event).max(start_us),
            end_us,
            threshold_us,
            Some(previous_event),
            None,
        )?;
    } else {
        report_span(
            &mut spans,
            SparseSpanKind::NoRecords,
            start_us,
            end_us,
            threshold_us,
            None,
            None,
        )?;
    }
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_output_never_calls_boundaries_certified() {
        let mut result = Vec::new();
        report_span(
            &mut result,
            SparseSpanKind::NoRecords,
            -1_000,
            10_000,
            1_000,
            None,
            None,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind.code(), "no_recorded_anchors_in_window");
        assert_eq!(result[0].start_us, -1_000);
        assert_eq!(result[0].next_original_source_id, None);
        assert!(report_span(
            &mut result,
            SparseSpanKind::NoRecords,
            0,
            1_000,
            1_000,
            None,
            None
        )
        .is_ok());
        assert_eq!(result.len(), 1);
    }
}
