/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! Public external-crate tests for bounded chronological recorded-event pages.

use virgostars_events_core::{
    BodyId, ChronologicalQueryError, Event, EventFamily, EventIndex, IngressEvent,
    MotionDirection, MotionEvent, SynodEvent, TimestampMicros, ZodiacSign,
    MAX_CHRONOLOGICAL_PAGE_SIZE,
};

fn body(value: &str) -> BodyId {
    BodyId::try_from(value).unwrap()
}

fn time(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

fn synod(value: &str, at: i64) -> Event {
    Event::Synod(SynodEvent::new(body(value), time(at), None).unwrap())
}

fn station(value: &str, at: i64) -> Event {
    Event::Motion(MotionEvent::new(
        body(value),
        time(at),
        MotionDirection::Direct,
        ZodiacSign::try_from(1_u8).unwrap(),
    ))
}

fn ingress(value: &str, at: i64) -> Event {
    Event::Ingress(IngressEvent::new(
        body(value),
        time(at),
        ZodiacSign::try_from(1_u8).unwrap(),
    ))
}

fn mixed_index() -> EventIndex {
    EventIndex::from_events(vec![
        ingress("Moon", 0),
        ingress("Mars", -10),
        synod("Moon", 0),
        station("Moon", 0),
        synod("Mercury", 0),
        ingress("Mercury", 0),
        ingress("Moon", 10),
    ])
    .unwrap()
}

#[test]
fn events_merge_by_time_family_and_body_with_deterministic_ties() {
    let index = mixed_index();
    let page = index.chronological_between(time(-10), time(11), 100, None).unwrap();
    let observed: Vec<_> = page
        .events
        .iter()
        .map(|event| (event.at().as_i64(), event.family(), event.body().as_str()))
        .collect();
    assert_eq!(
        observed,
        vec![
            (-10, EventFamily::Ingress, "Mars"),
            (0, EventFamily::Synod, "Mercury"),
            (0, EventFamily::Synod, "Moon"),
            (0, EventFamily::Motion, "Moon"),
            (0, EventFamily::Ingress, "Mercury"),
            (0, EventFamily::Ingress, "Moon"),
            (10, EventFamily::Ingress, "Moon"),
        ]
    );
    assert!(page.next_cursor.is_none());
}

#[test]
fn stable_cursor_pages_do_not_duplicate_or_skip_tied_events() {
    let index = mixed_index();
    let mut after = None;
    let mut observed = Vec::new();
    let mut page_count = 0;

    loop {
        let page = index
            .chronological_between(time(-10), time(11), 2, after.as_ref())
            .unwrap();
        assert!(!page.events.is_empty());
        assert!(page.events.len() <= 2);
        observed.extend(
            page.events
                .iter()
                .map(|event| (event.at(), event.family(), event.body().clone())),
        );
        page_count += 1;
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
        assert!(page_count < 10, "cursor must make progress");
    }

    let direct = index.chronological_between(time(-10), time(11), 100, None).unwrap();
    let expected: Vec<_> = direct.events.iter().map(|event| {
        (event.at(), event.family(), event.body().clone())
    }).collect();
    assert_eq!(page_count, 4);
    assert_eq!(observed, expected);
}

#[test]
fn half_open_boundaries_and_pre_epoch_instants_are_exact() {
    let index = mixed_index();
    let negative = index.chronological_between(time(-10), time(0), 3, None).unwrap();
    assert_eq!(negative.events.len(), 1);
    assert_eq!(negative.events[0].at(), time(-10));

    let at_zero = index.chronological_between(time(0), time(10), 10, None).unwrap();
    assert_eq!(at_zero.events.len(), 5);
    assert!(at_zero.events.iter().all(|event| event.at() == time(0)));

    let empty = index.chronological_between(time(10), time(10), 10, None).unwrap();
    assert!(empty.events.is_empty());
    assert!(empty.next_cursor.is_none());

    let unknown = EventIndex::default()
        .chronological_between(time(-10), time(11), 10, None)
        .unwrap();
    assert!(unknown.events.is_empty());
}

#[test]
fn reversed_intervals_and_unsafe_page_sizes_fail_closed() {
    let index = mixed_index();
    assert_eq!(
        index.chronological_between(time(10), time(0), 10, None).unwrap_err(),
        ChronologicalQueryError::ReversedRange { start: time(10), end: time(0) }
    );
    for limit in [0, MAX_CHRONOLOGICAL_PAGE_SIZE + 1] {
        assert_eq!(
            index.chronological_between(time(-10), time(11), limit, None).unwrap_err(),
            ChronologicalQueryError::InvalidLimit {
                requested: limit,
                max: MAX_CHRONOLOGICAL_PAGE_SIZE,
            }
        );
    }
    assert_eq!(
        index.chronological_between(
            time(-10), time(11), MAX_CHRONOLOGICAL_PAGE_SIZE, None
        ).unwrap().events.len(),
        7
    );
}

#[test]
fn stale_or_out_of_interval_cursors_are_rejected() {
    let index = mixed_index();
    let first = index.chronological_between(time(-10), time(11), 1, None).unwrap();
    let cursor = first.next_cursor.unwrap();
    assert_eq!(cursor.at(), time(-10));
    assert_eq!(cursor.interval_start(), time(-10));
    assert_eq!(cursor.interval_end(), time(11));
    assert_eq!(cursor.family(), EventFamily::Ingress);
    assert_eq!(cursor.body().as_str(), "Mars");

    assert_eq!(
        index.chronological_between(time(0), time(11), 1, Some(&cursor)).unwrap_err(),
        ChronologicalQueryError::InvalidCursor
    );
    // The cursor still falls within this changed interval, but the query
    // itself is different and cannot safely reuse the old pagination state.
    assert_eq!(
        index.chronological_between(time(-11), time(11), 1, Some(&cursor)).unwrap_err(),
        ChronologicalQueryError::InvalidCursor
    );
    let another_index = EventIndex::from_events(vec![ingress("Venus", -10)]).unwrap();
    assert_eq!(
        another_index
            .chronological_between(time(-10), time(11), 1, Some(&cursor))
            .unwrap_err(),
        ChronologicalQueryError::InvalidCursor
    );
}

#[test]
fn cursor_cannot_cross_independent_indices_with_identical_events() {
    // Event equality is insufficient for safe cursor ownership. Both indices
    // contain the same data and support the same interval, but were built
    // independently.
    let first_index = mixed_index();
    let second_index = mixed_index();
    let page = first_index
        .chronological_between(time(-10), time(11), 1, None)
        .unwrap();
    let cursor = page.next_cursor.unwrap();

    assert_eq!(
        second_index
            .chronological_between(time(-10), time(11), 1, Some(&cursor))
            .unwrap_err(),
        ChronologicalQueryError::InvalidCursor
    );

    // A clone is the same immutable logical index and retains its identity.
    let clone = first_index.clone();
    let next = clone
        .chronological_between(time(-10), time(11), 1, Some(&cursor))
        .unwrap();
    assert_eq!(next.events.len(), 1);
    assert_eq!(next.events[0].at(), time(0));
}

#[test]
fn pagination_does_not_infer_missing_astronomical_events() {
    let index = EventIndex::from_events(vec![
        ingress("Moon", 0),
        ingress("Moon", 10_000_000),
    ])
    .unwrap();
    let page = index.chronological_between(time(1), time(10_000_000), 5, None).unwrap();
    // The index does not invent an ingress for the enormous gap.
    assert!(page.events.is_empty());
    assert!(page.next_cursor.is_none());
    let after = index.chronological_between(time(1), time(10_000_001), 5, None).unwrap();
    assert_eq!(after.events.len(), 1);
    assert_eq!(after.events[0].at(), time(10_000_000));
}
