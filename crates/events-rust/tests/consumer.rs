/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use virgostars_event_ephemeris::{EventFamily, VirgoStarsEventEphemeris};

const GOLDEN_SHA: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestArchive(PathBuf);

impl TestArchive {
    fn open_fixture() -> Self {
        let fixture = STANDARD
            .decode(include_str!("../../events-p0/tests/fixtures/synthetic.p0.base64").trim())
            .unwrap();
        let id = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "virgostars-rust-consumer-{}-{id}.p0",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&fixture).unwrap();
        Self(path)
    }
}

impl Drop for TestArchive {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn full_integrity_gate_before_reading() {
    let fixture = TestArchive::open_fixture();
    let events = VirgoStarsEventEphemeris::open_verified(&fixture.0, GOLDEN_SHA).unwrap();
    assert_eq!(events.verified_sha256(), GOLDEN_SHA);
    assert_eq!(events.audit().records, 8);
    assert_eq!(events.audit().blocks, 4);
    assert_eq!(events.synods_at(2_000_000).unwrap().len(), 2);

    assert!(VirgoStarsEventEphemeris::open_verified(&fixture.0, &"0".repeat(64)).is_err());
    assert!(VirgoStarsEventEphemeris::open_verified(&fixture.0, "not-a-hash").is_err());
}

#[test]
fn app_queries_preserve_recorded_boundaries() {
    let fixture = TestArchive::open_fixture();
    let events = VirgoStarsEventEphemeris::open_verified(&fixture.0, GOLDEN_SHA).unwrap();
    let first = events
        .next(EventFamily::Motion, "Mars", i64::MIN)
        .unwrap()
        .unwrap();
    let at = first.at().as_i64();
    assert_eq!(
        events.at(EventFamily::Motion, "Mars", at).unwrap().unwrap(),
        first
    );
    assert!(events
        .previous(EventFamily::Motion, "Mars", at)
        .unwrap()
        .is_none());
    let within = events
        .between(EventFamily::Motion, "Mars", at, at + 1)
        .unwrap();
    assert_eq!(within, vec![first]);
    assert!(events
        .between(EventFamily::Motion, " Mars", at, at + 1)
        .is_err());
    assert!(events
        .between(EventFamily::Motion, "Mars", at + 1, at)
        .is_err());
}
