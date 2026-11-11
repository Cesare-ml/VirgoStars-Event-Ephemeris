/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! Python-M0 precursor: batch latest recorded synod queries in the Rust P0.
//! Uses a pre-existing *synthetic* Python-generated independent golden archive.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use virgostars_events_core::{BodyId, Event, EventFamily, TimestampMicros};
use virgostars_events_p0::P0Archive;

const GOLDEN_SHA256: &str =
    "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

struct FixtureFile(PathBuf);

impl FixtureFile {
    fn new() -> Self {
        let raw = STANDARD
            .decode(include_str!("fixtures/synthetic.p0.base64").trim())
            .unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&raw)), GOLDEN_SHA256);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "virgostars-synod-batch-{}-{nanos}-{id}.p0",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&raw).unwrap();
        Self(path)
    }
}

impl Drop for FixtureFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn body(value: &str) -> BodyId {
    BodyId::try_from(value).unwrap()
}

fn t(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

#[test]
fn before_first_anchor_preserves_all_known_bodies_with_no_fictitious_cycle() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let snapshots = archive.synods_at(t(-876_545)).unwrap();
    assert_eq!(
        snapshots.keys().map(|body| body.as_str()).collect::<Vec<_>>(),
        vec!["Mercury", "Moon"]
    );
    for snapshot in snapshots.values() {
        assert!(snapshot.record.is_none());
        assert!(snapshot.next_recorded_start.is_none());
        assert!(snapshot.phase.is_none());
    }
}

#[test]
fn latest_inclusive_moon_anchor_and_next_start_match_indexed_navigation() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();

    let before = archive.synods_at(t(-876_544)).unwrap();
    let moon = &before[&body("Moon")];
    assert_eq!(moon.record.as_ref().unwrap().source_id, 10);
    assert_eq!(moon.record.as_ref().unwrap().at(), t(-876_544));
    assert_eq!(moon.next_recorded_start, Some(t(1_500_000)));
    assert!(moon.phase.as_ref().unwrap().stage.is_none());

    let at_second = archive.synods_at(t(1_500_000)).unwrap();
    let moon = &at_second[&body("Moon")];
    assert_eq!(moon.record.as_ref().unwrap().source_id, 12);
    assert_eq!(moon.record.as_ref().unwrap().at(), t(1_500_000));
    assert_eq!(moon.next_recorded_start, None);
    let phase = moon.phase.as_ref().unwrap();
    assert_eq!(phase.stage.unwrap().code(), "CRESC1");
    assert_eq!(phase.started_at, Some(t(1_500_000)));

    let same = archive
        .at(EventFamily::Synod, &body("Moon"), t(1_500_000))
        .unwrap()
        .unwrap();
    assert_eq!(same, *moon.record.as_ref().unwrap());
}

#[test]
fn mercury_without_previous_anchor_and_empty_extras_return_explicit_unknown() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();

    let earlier = archive.synods_at(t(1_999_999)).unwrap();
    assert!(earlier[&body("Mercury")].record.is_none());

    let exact = archive.synods_at(t(2_000_000)).unwrap();
    let mercury = &exact[&body("Mercury")];
    assert_eq!(mercury.record.as_ref().unwrap().source_id, 21);
    match &mercury.record.as_ref().unwrap().event {
        Event::Synod(event) => assert!(event.phases().unwrap().is_empty()),
        _ => panic!("synod index returned non-synod event"),
    }
    let phase = mercury.phase.as_ref().unwrap();
    assert_eq!(phase.stage, None);
    assert_eq!(phase.status.code(), "unknown_incomplete_phase_thresholds");
}

#[test]
fn source_id_and_phase_thresholds_are_stable_across_batch_queries() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let results = archive.synods_at(t(2_000_000)).unwrap();
    for (body, snapshot) in results {
        let single = archive.at(EventFamily::Synod, &body, t(2_000_000)).unwrap();
        assert_eq!(single, snapshot.record, "batch differs from indexed at");
        if let Some(current) = single {
            let next = archive
                .next(EventFamily::Synod, &body, current.at())
                .unwrap()
                .map(|record| record.at());
            assert_eq!(next, snapshot.next_recorded_start);
            assert!(snapshot.phase.is_some());
        }
    }
}
