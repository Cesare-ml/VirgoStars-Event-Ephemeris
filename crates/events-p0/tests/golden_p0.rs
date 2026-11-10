/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE and NOTICE; Powered by VirgoStars — https://www.virgostars.it/
 */
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use virgostars_events_core::{BodyId, Event, EventFamily, TimestampMicros};
use virgostars_events_p0::P0Archive;

const GOLDEN_SHA256: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";

fn fixture() -> Vec<u8> {
    let bytes = STANDARD.decode(include_str!("fixtures/synthetic.p0.base64").trim())
        .expect("independently encoded fixture");
    assert_eq!(bytes.len(), 1470);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), GOLDEN_SHA256);
    bytes
}

struct TempFile(PathBuf);

impl TempFile {
    fn new(bytes: &[u8]) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "virgostars-p0-fixture-{}-{nanos}.bin", std::process::id()
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn stamp(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

fn body(value: &str) -> BodyId {
    BodyId::try_from(value).unwrap()
}

#[test]
fn opens_independent_python_generated_golden_bytes() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    assert_eq!(reader.record_count(), 8);
    assert_eq!(reader.block_count(), 4);
    let moon = body("Moon");
    let first = reader.at(EventFamily::Synod, &moon, stamp(0)).unwrap().unwrap();
    assert_eq!(first.source_id, 10);
    assert_eq!(first.at().as_i64(), -876_544);
    let next = reader.next(EventFamily::Synod, &moon, stamp(0)).unwrap().unwrap();
    assert_eq!(next.source_id, 12);
    assert_eq!(next.at().as_i64(), 1_500_000);
}

#[test]
fn strict_equality_and_repeated_signs_are_recorded_without_interpolation() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    let moon = body("Moon");
    assert_eq!(reader.previous(EventFamily::Ingress, &moon, stamp(-876_544)).unwrap(), None);
    assert_eq!(reader.at(EventFamily::Ingress, &moon, stamp(-876_544)).unwrap().unwrap().source_id, 41);
    assert_eq!(reader.next(EventFamily::Ingress, &moon, stamp(3_000_000)).unwrap().unwrap().source_id, 43);
    assert_eq!(reader.previous(EventFamily::Ingress, &moon, stamp(3_000_000)).unwrap().unwrap().source_id, 41);
    let between = reader.between(EventFamily::Ingress, &moon, stamp(3_000_000), stamp(5_000_000)).unwrap();
    assert_eq!(between.len(), 1);
    assert_eq!(between[0].source_id, 42);
    let final_record = reader.at(EventFamily::Ingress, &moon, stamp(5_000_000)).unwrap().unwrap();
    match final_record.event {
        Event::Ingress(ingress) => assert_eq!(ingress.sign().index(), 3),
        _ => panic!("expected ingress"),
    }
}

#[test]
fn synod_null_empty_and_ordinal_phases_survive_decoding() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    let first = reader.synod_at(&body("Moon"), stamp(0)).unwrap().unwrap();
    match &first.record.event {
        Event::Synod(synod) => assert!(synod.phases().is_none()),
        _ => panic!("expected synod"),
    }
    let second = first.next().unwrap().unwrap();
    assert_eq!(second.record.source_id, 12);
    assert_eq!(second.previous().unwrap().unwrap().record.source_id, 10);
    match &second.record.event {
        Event::Synod(synod) => {
            let phases = synod.phases().unwrap();
            assert_eq!(phases.len(), 3);
            assert_eq!(phases[0].name(), "extra_1");
            assert_eq!(phases[0].at().as_i64(), 1_700_000);
        }
        _ => panic!("expected synod"),
    }
    let mercury = reader.at(EventFamily::Synod, &body("Mercury"), stamp(2_000_000)).unwrap().unwrap();
    match mercury.event {
        Event::Synod(synod) => assert!(synod.phases().unwrap().is_empty()),
        _ => panic!("expected synod"),
    }
}

#[test]
fn motion_preserves_sign_direction_and_source_id() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    let mars = body("Mars");
    let first = reader.at(EventFamily::Motion, &mars, stamp(0)).unwrap().unwrap();
    assert_eq!(first.source_id, 31);
    match first.event {
        Event::Motion(event) => {
            assert_eq!(event.sign().index(), 4);
            assert_eq!(event.direction().legacy_code(), "r");
        }
        _ => panic!("expected motion"),
    }
    let second = reader.next(EventFamily::Motion, &mars, stamp(0)).unwrap().unwrap();
    match second.event {
        Event::Motion(event) => assert_eq!(event.direction().legacy_code(), "d"),
        _ => panic!("expected motion"),
    }
}

#[test]
fn header_rejects_bad_magic_header_length_and_trailing_bytes() {
    let original = fixture();
    let mut bad_magic = original.clone();
    bad_magic[0] ^= 0xff;
    assert!(P0Archive::open(TempFile::new(&bad_magic).0).is_err());

    let mut bad_length = original.clone();
    bad_length[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(P0Archive::open(TempFile::new(&bad_length).0).is_err());

    let mut trailing = original;
    trailing.extend_from_slice(b"appended");
    assert!(P0Archive::open(TempFile::new(&trailing).0).is_err());
}

#[test]
fn damaged_compressed_block_cannot_pass_a_query() {
    let mut bad = fixture();
    let end = bad.len() - 1;
    bad[end] ^= 0x40;
    let file = TempFile::new(&bad);
    // Header and index remain intact, but block integrity is checked lazily.
    let reader = P0Archive::open(&file.0).unwrap();
    assert!(reader.at(EventFamily::Synod, &body("Moon"), stamp(1_500_000)).is_err());
}

#[test]
fn queries_reject_reversed_ranges_and_unknown_bodies() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    let venus = body("Venus");
    assert!(reader.at(EventFamily::Ingress, &venus, stamp(100)).unwrap().is_none());
    assert!(reader.next(EventFamily::Ingress, &venus, stamp(100)).unwrap().is_none());
    assert!(reader.between(EventFamily::Ingress, &venus, stamp(200), stamp(100)).is_err());
    assert!(reader.between(EventFamily::Ingress, &venus, stamp(100), stamp(100)).unwrap().is_empty());
}

/// Optional full-data integration: execute explicitly with VSEPH_P0_PATH set.
#[test]
#[ignore = "requires an assembled 2026-11-10 dataset path"]
fn reads_real_snapshot_without_certifying_coverage() {
    let path = std::env::var("VSEPH_P0_PATH").expect("set VSEPH_P0_PATH to reconstructed snapshot");
    let reader = P0Archive::open(path).unwrap();
    assert_eq!(reader.record_count(), 380_844);
    assert_eq!(reader.block_count(), 761);
    let mercury = body("Mercury");
    let _ = reader.next(EventFamily::Synod, &mercury, stamp(0)).unwrap();
}
