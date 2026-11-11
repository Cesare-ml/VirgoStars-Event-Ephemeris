/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * See LICENSE and NOTICE; Powered by VirgoStars — https://www.virgostars.it/
 */
//! Cross-language policy semantics: synthetic fixtures only, not verified
//! astronomical coverage. The experimental P0 archive is not VSEPH v1.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use virgostars_events_core::{BodyId, Event, TimestampMicros};
use virgostars_events_p0::{CoveredState, P0Archive};

const GOLDEN_SHA256: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
static NEXT_TEST_FILE_ID: AtomicU64 = AtomicU64::new(0);

struct TestFile(PathBuf);

impl TestFile {
    fn new(name: &str, bytes: &[u8]) -> Self {
<<<<<<< Updated upstream
        // Windows clock resolution is not unique for concurrently running tests.
        // Combine process ID, clock tick and atomic counter, then create
        // exclusively so one test cannot overwrite/delete another's fixture.
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = NEXT_TEST_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vseph-coverage-{}-{nanos}-{sequence}-{name}", std::process::id()
=======
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vseph-coverage-{}-{nanos}-{name}",
            std::process::id()
>>>>>>> Stashed changes
        ));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        file.write_all(bytes).unwrap();
        Self(path)
    }
}

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn policy_and_archive_fixtures_do_not_collide_under_parallel_allocation() {
    let bytes = STANDARD.decode(include_str!("fixtures/synthetic.p0.base64").trim()).unwrap();
    let fixtures: Vec<_> = (0..64)
        .map(|_| TestFile::new("synthetic.p0", &bytes))
        .collect();
    let paths: std::collections::HashSet<_> = fixtures.iter().map(|file| &file.0).collect();
    assert_eq!(paths.len(), fixtures.len());
    for file in &fixtures {
        assert_eq!(fs::read(&file.0).unwrap(), bytes);
        assert_eq!(P0Archive::open(&file.0).unwrap().record_count(), 8);
    }
}

fn fixture() -> TestFile {
    let bytes = STANDARD
        .decode(include_str!("fixtures/synthetic.p0.base64").trim())
        .unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), GOLDEN_SHA256);
    TestFile::new("synthetic.p0", &bytes)
}

fn policy_file(hash: &str, windows: Vec<Value>) -> TestFile {
    let document = json!({
        "schema": "virgostars-reviewed-coverage/v0",
        "verification": "human-reviewed-only",
        "archive_sha256": hash,
        "reviewed_windows": windows
    });
    TestFile::new("coverage.json", &serde_json::to_vec(&document).unwrap())
}

fn reviewed_window(family: &str, body: &str, start: i64, end: i64) -> Value {
    json!({
        "family": family,
        "body": body,
        "start_us": start.to_string(),
        "end_us_exclusive": end.to_string(),
        "reviewer": "synthetic-test-reviewer",
        "evidence": "fabricated examples; explicit test-only completeness"
    })
}

fn instant(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

fn body(name: &str) -> BodyId {
    BodyId::try_from(name).unwrap()
}

#[test]
fn no_reviewed_ranges_fails_closed_even_with_real_event_index() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let policy_file = policy_file(GOLDEN_SHA256, vec![]);
    let covered = archive.with_coverage_policy(&policy_file.0).unwrap();
    assert_eq!(covered.reviewed_window_count(), 0);
    assert_eq!(covered.archive_sha256(), GOLDEN_SHA256);
    match covered.sign_at(&body("Moon"), instant(4_000_000)).unwrap() {
        CoveredState::UnknownCoverage {
            last_recorded: Some(row),
        } => {
            assert_eq!(row.source_id, 42);
            assert_eq!(row.at().as_i64(), 3_000_000);
        }
        other => panic!("expected unknown coverage with recorded evidence: {other:?}"),
    }
}

#[test]
fn reviewed_bounds_are_half_open_and_pre_epoch_state_is_exact() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let policy_file = policy_file(
        GOLDEN_SHA256,
        vec![reviewed_window("ingress", "Moon", -876_544, 4_000_000)],
    );
    let covered = archive.with_coverage_policy(&policy_file.0).unwrap();
    for (at, expected_id) in [(-876_544, 41), (3_000_000, 42), (3_999_999, 42)] {
        match covered.sign_at(&body("Moon"), instant(at)).unwrap() {
            CoveredState::Reviewed {
                record,
                reviewer,
                window_start,
                window_end_exclusive,
                ..
            } => {
                assert_eq!(record.source_id, expected_id);
                assert_eq!(reviewer, "synthetic-test-reviewer");
                assert_eq!(window_start.as_i64(), -876_544);
                assert_eq!(window_end_exclusive.as_i64(), 4_000_000);
                match record.event {
                    Event::Ingress(_) => {}
                    _ => panic!("expected an ingress event"),
                }
            }
            other => panic!("expected reviewed state: {other:?}"),
        }
    }
    assert!(matches!(
        covered.sign_at(&body("Moon"), instant(4_000_000)).unwrap(),
        CoveredState::UnknownCoverage { .. }
    ));
}

#[test]
fn window_without_start_event_must_not_use_previous_window_state() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let policy_file = policy_file(
        GOLDEN_SHA256,
        vec![reviewed_window("ingress", "Moon", 0, 4_000_000)],
    );
    let covered = archive.with_coverage_policy(&policy_file.0).unwrap();
    assert!(matches!(
        covered.sign_at(&body("Moon"), instant(2_000_000)).unwrap(),
        CoveredState::UnknownInitialState { .. }
    ));
    assert!(matches!(
        covered.sign_at(&body("Moon"), instant(3_000_000)).unwrap(),
        CoveredState::Reviewed { .. }
    ));
}

#[test]
fn motion_and_ingress_reviews_are_separate_even_for_matching_timestamps() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let policy_file = policy_file(
        GOLDEN_SHA256,
        vec![reviewed_window("motion", "Mars", 0, 3_000_000)],
    );
    let covered = archive.with_coverage_policy(&policy_file.0).unwrap();
    assert!(matches!(
        covered.sign_at(&body("Mars"), instant(2_200_000)).unwrap(),
        CoveredState::UnknownCoverage { .. }
    ));
    match covered
        .direction_at(&body("Mars"), instant(2_200_000))
        .unwrap()
    {
        CoveredState::Reviewed { record, .. } => match record.event {
            Event::Motion(motion) => assert_eq!(motion.direction().legacy_code(), "d"),
            _ => panic!("expected recorded motion"),
        },
        other => panic!("expected reviewed motion: {other:?}"),
    }
}

#[test]
fn policy_must_be_bound_to_exact_archive_hash() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let other_hash = "0".repeat(64);
    let policy_file = policy_file(&other_hash, vec![]);
    assert!(archive.with_coverage_policy(&policy_file.0).is_err());
}

#[test]
fn unreviewed_or_overlapping_ranges_are_rejected() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let bad = vec![
        reviewed_window("ingress", "Moon", 0, 20),
        reviewed_window("ingress", "Moon", 10, 30),
    ];
    let overlapping_policy = policy_file(GOLDEN_SHA256, bad);
    assert!(archive.with_coverage_policy(&overlapping_policy.0).is_err());

    let mut invalid = reviewed_window("ingress", "Moon", 0, 20);
    invalid["evidence"] = Value::String(String::new());
    let invalid_file = policy_file(GOLDEN_SHA256, vec![invalid]);
    assert!(archive.with_coverage_policy(&invalid_file.0).is_err());

    let mut invalid = reviewed_window("ingress", "Moon", 0, 20);
    invalid["start_us"] = Value::String("+0".into());
    let invalid_file = policy_file(GOLDEN_SHA256, vec![invalid]);
    assert!(archive.with_coverage_policy(&invalid_file.0).is_err());
}

#[test]
fn archive_mutation_after_review_attachment_is_detected() {
    let file = fixture();
    let archive = P0Archive::open(&file.0).unwrap();
    let policy_file = policy_file(
        GOLDEN_SHA256,
        vec![reviewed_window("ingress", "Moon", -876_544, 4_000_000)],
    );
    let covered = archive.with_coverage_policy(&policy_file.0).unwrap();
    let mut bytes = fs::read(&file.0).unwrap();
    let end = bytes.len() - 1;
    bytes[end] ^= 1;
    fs::write(&file.0, bytes).unwrap();
    assert!(covered.sign_at(&body("Moon"), instant(3_000_000)).is_err());
}
