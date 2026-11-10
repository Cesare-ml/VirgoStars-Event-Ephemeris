/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B, and NOTICE. Provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Immutable in-memory index for precomputed events.
//! All queries return recorded events, NOT certified astronomical states.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{BodyId, Event, SynodEvent, TimestampMicros};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EventFamily {
    Synod,
    Motion,
    Ingress,
}

impl Event {
    pub const fn family(&self) -> EventFamily {
        match self {
            Self::Synod(_) => EventFamily::Synod,
            Self::Motion(_) => EventFamily::Motion,
            Self::Ingress(_) => EventFamily::Ingress,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexError {
    DuplicateInstant {
        family: EventFamily,
        body: BodyId,
        at: TimestampMicros,
    },
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInstant { family, body, at } => {
                write!(f, "duplicate {:?} for {} at {}", family, body, at.as_i64())
            }
        }
    }
}

impl Error for IndexError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    ReversedRange {
        start: TimestampMicros,
        end: TimestampMicros,
    },
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReversedRange { start, end } => {
                write!(f, "range {}..{} is reversed", start.as_i64(), end.as_i64())
            }
        }
    }
}

impl Error for QueryError {}

/// Cursor for an immutable chronological event index.
///
/// It identifies the last returned event by (timestamp, family, body), in
/// that order. A cursor is valid only for the same immutable EventIndex and
/// the same half-open query interval. It is not a serializable dataset ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChronologicalCursor {
    at: TimestampMicros,
    family: EventFamily,
    body: BodyId,
}

impl ChronologicalCursor {
    pub const fn at(&self) -> TimestampMicros {
        self.at
    }

    pub const fn family(&self) -> EventFamily {
        self.family
    }

    pub fn body(&self) -> &BodyId {
        &self.body
    }
}

/// A bounded chronological page of *recorded* events, never coverage proof.
#[derive(Clone, Debug)]
pub struct ChronologicalPage<'a> {
    pub events: Vec<&'a Event>,
    /// Present only when another page exists in the requested interval.
    pub next_cursor: Option<ChronologicalCursor>,
}

/// Errors for bounded, chronological queries on the immutable event index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChronologicalQueryError {
    ReversedRange {
        start: TimestampMicros,
        end: TimestampMicros,
    },
    InvalidLimit {
        requested: usize,
        max: usize,
    },
    InvalidCursor,
}

impl fmt::Display for ChronologicalQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReversedRange { start, end } => write!(
                f,
                "chronological range {}..{} is reversed",
                start.as_i64(),
                end.as_i64()
            ),
            Self::InvalidLimit { requested, max } => {
                write!(f, "chronological page size {requested} must be 1..={max}")
            }
            Self::InvalidCursor => {
                f.write_str("chronological cursor does not match this index and interval")
            }
        }
    }
}

impl Error for ChronologicalQueryError {}

/// Hard cap for an in-memory chronological page, independent of source size.
pub const MAX_CHRONOLOGICAL_PAGE_SIZE: usize = 10_000;

/// Sorted, immutable, per-family/body recorded event index.
/// Queries do NOT infer continuity, completeness, or a scientifically valid state.
#[derive(Clone, Debug, Default)]
pub struct EventIndex {
    series: BTreeMap<(EventFamily, BodyId), Vec<Event>>,
}

impl EventIndex {
    /// Sort supplied records and reject identical family/body/timestamps.
    pub fn from_events(events: impl IntoIterator<Item = Event>) -> Result<Self, IndexError> {
        let mut series: BTreeMap<(EventFamily, BodyId), Vec<Event>> = BTreeMap::new();
        for event in events {
            let key = (event.family(), event.body().clone());
            series.entry(key).or_default().push(event);
        }
        for ((family, body), records) in &mut series {
            records.sort_by_key(Event::at);
            for pair in records.windows(2) {
                if pair[0].at() == pair[1].at() {
                    return Err(IndexError::DuplicateInstant {
                        family: *family,
                        body: body.clone(),
                        at: pair[0].at(),
                    });
                }
            }
        }
        Ok(Self { series })
    }

    pub fn series(&self, family: EventFamily, body: &BodyId) -> Option<&[Event]> {
        self.series.get(&(family, body.clone())).map(Vec::as_slice)
    }

    /// Latest recorded event at or before the instant.
    pub fn at(&self, family: EventFamily, body: &BodyId, instant: TimestampMicros) -> Option<&Event> {
        let events = self.series(family, body)?;
        let end = events.partition_point(|event| event.at() <= instant);
        end.checked_sub(1).map(|position| &events[position])
    }

    /// First recorded event strictly later than the instant.
    pub fn next(&self, family: EventFamily, body: &BodyId, instant: TimestampMicros) -> Option<&Event> {
        let events = self.series(family, body)?;
        events.get(events.partition_point(|event| event.at() <= instant))
    }

    /// Last recorded event strictly earlier than the instant.
    pub fn previous(
        &self,
        family: EventFamily,
        body: &BodyId,
        instant: TimestampMicros,
    ) -> Option<&Event> {
        let events = self.series(family, body)?;
        let end = events.partition_point(|event| event.at() < instant);
        end.checked_sub(1).map(|position| &events[position])
    }

    /// Half-open recorded-event interval [start,end).
    pub fn between(
        &self,
        family: EventFamily,
        body: &BodyId,
        start: TimestampMicros,
        end: TimestampMicros,
    ) -> Result<&[Event], QueryError> {
        if end < start {
            return Err(QueryError::ReversedRange { start, end });
        }
        let Some(events) = self.series(family, body) else {
            return Ok(&[]);
        };
        let first = events.partition_point(|event| event.at() < start);
        let last = events.partition_point(|event| event.at() < end);
        Ok(&events[first..last])
    }

    pub fn synod_at(&self, body: &BodyId, instant: TimestampMicros) -> Option<SynodHandle<'_>> {
        let events = self.series(EventFamily::Synod, body)?;
        let end = events.partition_point(|event| event.at() <= instant);
        Some(SynodHandle {
            index: self,
            body: body.clone(),
            position: end.checked_sub(1)?,
        })
    }

    /// Return the next bounded page of recorded events in [start,end).
    ///
    /// Stable ordering: timestamp ascending, then EventFamily, then BodyId.
    /// A family/body series cannot contain two events at the same timestamp,
    /// so no additional ordinal is required for the current in-memory model.
    ///
    /// Merge uses one ordered head per nonempty series; it does not allocate
    /// every matching event or claim continuous astronomical coverage.
    /// A returned cursor is valid only for this exact immutable index and
    /// interval. A zero or oversized page size is rejected.
    pub fn chronological_between(
        &self,
        start: TimestampMicros,
        end: TimestampMicros,
        limit: usize,
        after: Option<&ChronologicalCursor>,
    ) -> Result<ChronologicalPage<'_>, ChronologicalQueryError> {
        if end < start {
            return Err(ChronologicalQueryError::ReversedRange { start, end });
        }
        if limit == 0 || limit > MAX_CHRONOLOGICAL_PAGE_SIZE {
            return Err(ChronologicalQueryError::InvalidLimit {
                requested: limit,
                max: MAX_CHRONOLOGICAL_PAGE_SIZE,
            });
        }
        if let Some(cursor) = after {
            let valid = cursor.at >= start
                && cursor.at < end
                && self.series(cursor.family, &cursor.body).is_some_and(|records| {
                    let pos = records.partition_point(|event| event.at() < cursor.at);
                    records.get(pos).is_some_and(|event| event.at() == cursor.at)
                });
            if !valid {
                return Err(ChronologicalQueryError::InvalidCursor);
            }
        }
        if start == end {
            return Ok(ChronologicalPage {
                events: Vec::new(),
                next_cursor: None,
            });
        }

        // Each map entry owns the current head of one (family, body) series;
        // advancing that head costs O(log number_of_series).
        let mut heads: BTreeMap<
            (TimestampMicros, EventFamily, &BodyId),
            (&[Event], usize),
        > = BTreeMap::new();
        for ((family, body), series) in &self.series {
            let records = series.as_slice();
            let pos = records.partition_point(|event| {
                event.at() < start
                    || after.is_some_and(|cursor| {
                        (event.at(), *family, body)
                            <= (cursor.at, cursor.family, &cursor.body)
                    })
            });
            if let Some(event) = records.get(pos) {
                if event.at() < end {
                    heads.insert((event.at(), *family, body), (records, pos));
                }
            }
        }

        let mut events = Vec::with_capacity(limit.min(self.len()));
        while events.len() < limit {
            let Some(((_, family, body), (records, position))) = heads.pop_first() else {
                break;
            };
            events.push(&records[position]);
            let next_position = position + 1;
            if let Some(next) = records.get(next_position) {
                if next.at() < end {
                    heads.insert((next.at(), family, body), (records, next_position));
                }
            }
        }
        let next_cursor = if heads.is_empty() {
            None
        } else {
            events.last().map(|event| ChronologicalCursor {
                at: event.at(),
                family: event.family(),
                body: event.body().clone(),
            })
        };
        Ok(ChronologicalPage { events, next_cursor })
    }

    pub fn len(&self) -> usize {
        self.series.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }
}

/// Borrowed navigation handle; next/previous stay within the same body.
#[derive(Clone, Debug)]
pub struct SynodHandle<'a> {
    index: &'a EventIndex,
    body: BodyId,
    position: usize,
}

impl<'a> SynodHandle<'a> {
    pub fn event(&self) -> &SynodEvent {
        let events = self.index.series(EventFamily::Synod, &self.body)
            .expect("handle references an indexed series");
        match &events[self.position] {
            Event::Synod(event) => event,
            _ => unreachable!("synod series contains only synods"),
        }
    }

    pub fn next(&self) -> Option<Self> {
        let events = self.index.series(EventFamily::Synod, &self.body)?;
        if self.position + 1 >= events.len() {
            return None;
        }
        Some(Self {
            index: self.index,
            body: self.body.clone(),
            position: self.position + 1,
        })
    }

    pub fn previous(&self) -> Option<Self> {
        Some(Self {
            index: self.index,
            body: self.body.clone(),
            position: self.position.checked_sub(1)?,
        })
    }

    /// This is the next recorded anchor, not proof of a complete cycle.
    pub fn next_recorded_start(&self) -> Option<TimestampMicros> {
        self.next().map(|next| next.event().start())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IngressEvent, MotionDirection, MotionEvent, ZodiacSign};

    fn body(s: &str) -> BodyId {
        BodyId::try_from(s).unwrap()
    }

    fn ingress(name: &str, time: i64, sign: i32) -> Event {
        Event::Ingress(IngressEvent::new(
            body(name),
            TimestampMicros::new(time),
            ZodiacSign::try_from(sign).unwrap(),
        ))
    }

    fn synod(name: &str, time: i64) -> Event {
        Event::Synod(
            SynodEvent::new(body(name), TimestampMicros::new(time), None).unwrap()
        )
    }

    #[test]
    fn no_records_cannot_become_state() {
        let index = EventIndex::default();
        let moon = body("Moon");
        assert!(index.is_empty());
        assert!(index.at(EventFamily::Ingress, &moon, TimestampMicros::new(0)).is_none());
        assert!(index.next(EventFamily::Ingress, &moon, TimestampMicros::new(0)).is_none());
        assert!(index.previous(EventFamily::Ingress, &moon, TimestampMicros::new(0)).is_none());
        assert!(index.synod_at(&moon, TimestampMicros::new(0)).is_none());
    }

    #[test]
    fn exact_equal_events_have_strict_next_and_previous() {
        let moon = body("Moon");
        let index = EventIndex::from_events(vec![
            ingress("Moon", 20, 1),
            ingress("Moon", 0, 0),
            ingress("Moon", 10, 11),
        ]).unwrap();
        assert_eq!(index.len(), 3);
        assert_eq!(index.at(EventFamily::Ingress, &moon, TimestampMicros::new(10)).unwrap().at().as_i64(), 10);
        assert_eq!(index.next(EventFamily::Ingress, &moon, TimestampMicros::new(10)).unwrap().at().as_i64(), 20);
        assert_eq!(index.previous(EventFamily::Ingress, &moon, TimestampMicros::new(10)).unwrap().at().as_i64(), 0);
        assert!(index.next(EventFamily::Ingress, &moon, TimestampMicros::new(20)).is_none());
        assert!(index.previous(EventFamily::Ingress, &moon, TimestampMicros::new(0)).is_none());
    }

    #[test]
    fn interval_is_half_open_and_reversed_range_is_error() {
        let moon = body("Moon");
        let index = EventIndex::from_events(vec![
            ingress("Moon", 0, 0),
            ingress("Moon", 10, 11),
            ingress("Moon", 20, 0),
        ]).unwrap();
        let selected = index.between(EventFamily::Ingress, &moon, TimestampMicros::new(10), TimestampMicros::new(20)).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].at().as_i64(), 10);
        assert!(index.between(EventFamily::Ingress, &moon, TimestampMicros::new(10), TimestampMicros::new(10)).unwrap().is_empty());
        assert_eq!(
            index.between(EventFamily::Ingress, &moon, TimestampMicros::new(20), TimestampMicros::new(10)),
            Err(QueryError::ReversedRange { start: TimestampMicros::new(20), end: TimestampMicros::new(10) }),
        );
    }

    #[test]
    fn reject_same_family_body_instant_but_not_cross_family() {
        let error = EventIndex::from_events(vec![ingress("Moon", 10, 0), ingress("Moon", 10, 1)]).unwrap_err();
        assert_eq!(error, IndexError::DuplicateInstant {
            family: EventFamily::Ingress, body: body("Moon"), at: TimestampMicros::new(10),
        });
        let other = Event::Motion(MotionEvent::new(
            body("Moon"), TimestampMicros::new(10),
            MotionDirection::Direct, ZodiacSign::try_from(0_i32).unwrap(),
        ));
        assert_eq!(EventIndex::from_events(vec![ingress("Moon", 10, 0), other]).unwrap().len(), 2);
    }

    #[test]
    fn same_body_navigation_and_pre_epoch_timestamps() {
        let mercury = body("Mercury");
        let index = EventIndex::from_events(vec![
            synod("Mercury", 100),
            synod("Moon", 150),
            synod("Mercury", -876_544),
            synod("Mercury", 300),
        ]).unwrap();
        let current = index.synod_at(&mercury, TimestampMicros::new(100)).unwrap();
        assert_eq!(current.previous().unwrap().event().start().as_i64(), -876_544);
        assert_eq!(current.next().unwrap().event().start().as_i64(), 300);
        assert_eq!(current.next_recorded_start().unwrap().as_i64(), 300);
        assert!(current.next().unwrap().next().is_none());
        assert!(index.synod_at(&mercury, TimestampMicros::new(-876_545)).is_none());
    }

    #[test]
    fn gap_query_returns_recorded_event_without_verifying_sign() {
        let moon = body("Moon");
        let index = EventIndex::from_events(vec![
            ingress("Moon", 0, 1),
            ingress("Moon", 10_000_000, 0),
        ]).unwrap();
        assert_eq!(index.at(EventFamily::Ingress, &moon, TimestampMicros::new(5_000_000)).unwrap().at().as_i64(), 0);
        assert_eq!(index.next(EventFamily::Ingress, &moon, TimestampMicros::new(5_000_000)).unwrap().at().as_i64(), 10_000_000);
    }
}
