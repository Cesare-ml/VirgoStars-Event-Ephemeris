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
use std::sync::Arc;
use std::thread;
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

#[test]
fn queries_use_authenticated_copy_after_original_is_overwritten_and_removed() {
    let fixture = TestArchive::open_fixture();
    let events = VirgoStarsEventEphemeris::open_verified(&fixture.0, GOLDEN_SHA).unwrap();
    let first = events
        .next(EventFamily::Motion, "Mars", i64::MIN)
        .unwrap()
        .unwrap();

    fs::write(&fixture.0, b"replaced with non-archive bytes").unwrap();
    assert_eq!(
        events.next(EventFamily::Motion, "Mars", i64::MIN).unwrap(),
        Some(first.clone())
    );
    fs::remove_file(&fixture.0).unwrap();
    assert_eq!(
        events
            .at(EventFamily::Motion, "Mars", first.at().as_i64())
            .unwrap(),
        Some(first)
    );
    assert_eq!(events.verified_sha256(), GOLDEN_SHA);
}

#[test]
fn caller_selected_private_cache_is_removed_on_drop() {
    let fixture = TestArchive::open_fixture();
    let cache_root = tempfile::tempdir().unwrap();
    let expected_first = {
        let events =
            VirgoStarsEventEphemeris::open_verified_in(&fixture.0, GOLDEN_SHA, cache_root.path())
                .unwrap();
        assert_eq!(fs::read_dir(cache_root.path()).unwrap().count(), 1);
        let first = events.next(EventFamily::Motion, "Mars", i64::MIN).unwrap();
        assert!(first.is_some());
        first
    };
    assert_eq!(fs::read_dir(cache_root.path()).unwrap().count(), 0);
    assert!(expected_first.is_some());
}

#[test]
fn rejected_snapshot_does_not_leak_files_or_read_untrusted_source() {
    let fixture = TestArchive::open_fixture();
    let cache_root = tempfile::tempdir().unwrap();
    assert!(VirgoStarsEventEphemeris::open_verified_in(
        &fixture.0,
        &"0".repeat(64),
        cache_root.path()
    )
    .is_err());
    assert_eq!(fs::read_dir(cache_root.path()).unwrap().count(), 0);

    let missing = cache_root.path().join("not-a-file.p0");
    let invalid = VirgoStarsEventEphemeris::open_verified(&missing, "INVALID")
        .err()
        .unwrap();
    assert!(format!("{invalid}").contains("SHA-256"));
    assert!(VirgoStarsEventEphemeris::open_verified_in(&fixture.0, GOLDEN_SHA, &missing).is_err());
}

#[test]
fn verified_reader_supports_concurrent_read_only_native_consumers() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VirgoStarsEventEphemeris>();

    let fixture = TestArchive::open_fixture();
    let events = Arc::new(VirgoStarsEventEphemeris::open_verified(&fixture.0, GOLDEN_SHA).unwrap());
    let expected = events.next(EventFamily::Motion, "Mars", i64::MIN).unwrap();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let shared = Arc::clone(&events);
            thread::spawn(move || {
                assert_eq!(shared.synods_at(2_000_000).unwrap().len(), 2);
                shared.next(EventFamily::Motion, "Mars", i64::MIN).unwrap()
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), expected);
    }
}
