/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B, and NOTICE. Provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
use virgostars_events_core::{
    BodyId, Event, EventFamily, EventIndex, IngressEvent, SynodEvent, TimestampMicros, ZodiacSign,
};

fn body(value: &str) -> BodyId {
    BodyId::try_from(value).unwrap()
}

fn stamp(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

fn ingress(value: i64, sign: i32) -> Event {
    Event::Ingress(IngressEvent::new(
        body("Moon"),
        stamp(value),
        ZodiacSign::try_from(sign).unwrap(),
    ))
}

#[test]
fn public_index_api_can_navigate_without_loading_a_database() {
    let index = EventIndex::from_events(vec![
        Event::Synod(SynodEvent::new(body("Moon"), stamp(20), None).unwrap()),
        Event::Synod(SynodEvent::new(body("Moon"), stamp(10), None).unwrap()),
        ingress(-876_544, 0),
        ingress(30, 1),
    ]).unwrap();
    let moon = body("Moon");

    let synod = index.synod_at(&moon, stamp(10)).unwrap();
    assert_eq!(synod.event().start().as_i64(), 10);
    assert_eq!(synod.next().unwrap().event().start().as_i64(), 20);
    assert!(synod.previous().is_none());

    let recorded = index.at(EventFamily::Ingress, &moon, stamp(0)).unwrap();
    assert_eq!(recorded.at().as_i64(), -876_544);
    assert_eq!(index.next(EventFamily::Ingress, &moon, stamp(0)).unwrap().at().as_i64(), 30);
}

#[test]
fn missing_body_is_not_a_reason_to_infer_an_event() {
    let index = EventIndex::from_events(vec![ingress(10, 0)]).unwrap();
    let mars = body("Mars");
    assert!(index.at(EventFamily::Ingress, &mars, stamp(100)).is_none());
    assert!(index.between(EventFamily::Ingress, &mars, stamp(0), stamp(100)).unwrap().is_empty());
}
