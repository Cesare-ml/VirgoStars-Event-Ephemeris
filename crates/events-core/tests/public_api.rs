/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (the "License"); you may not use this file except in
 * compliance with the License. You may obtain a copy at
 * https://opensource.org/license/CPAL-1.0.
 *
 * The Original Code is VirgoStars-Event-Ephemeris source code.
 * The Original Developer and Initial Developer is Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY KIND.
 * See repository LICENSE (Exhibits A and B) and NOTICE.
 *
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
use virgostars_events_core::{
    probe_magic, BodyId, Event, IngressEvent, ModelError, MotionDirection, MotionEvent, SynodEvent,
    SynodPhase, TimestampMicros, ZodiacSign, VSEPH_MAGIC,
};

#[test]
fn public_types_work_from_an_external_crate() {
    let moon = BodyId::try_from("Moon").unwrap();
    let anchor = TimestampMicros::new(-1_000_000);
    let first_quarter = SynodPhase::new("firstQuarter", TimestampMicros::new(0)).unwrap();
    let synod = SynodEvent::new(moon.clone(), anchor, Some(vec![first_quarter])).unwrap();
    let event = Event::Synod(synod);
    assert_eq!(event.body(), &moon);
    assert_eq!(event.at().as_i64(), -1_000_000);

    let sign = ZodiacSign::try_from(4_u8).unwrap();
    let ingress = Event::Ingress(IngressEvent::new(moon, TimestampMicros::new(0), sign));
    assert_eq!(ingress.at().as_i64(), 0);

    let mars = BodyId::try_from("Mars").unwrap();
    let direction = MotionDirection::from_legacy_code("r").unwrap();
    let station = Event::Motion(MotionEvent::new(
        mars,
        TimestampMicros::new(123_456),
        direction,
        sign,
    ));
    assert_eq!(station.body().as_str(), "Mars");
    assert_eq!(station.at().as_i64(), 123_456);
}

#[test]
fn public_validation_is_strict_and_format_probe_is_not_a_decoder() {
    assert_eq!(BodyId::try_from(""), Err(ModelError::InvalidBody));
    assert_eq!(ZodiacSign::try_from(-1_i32), Err(ModelError::InvalidSign(-1)));
    assert!(probe_magic(&VSEPH_MAGIC).is_ok());
    // A magic match alone must not be mistaken for a decoded dataset.
}
