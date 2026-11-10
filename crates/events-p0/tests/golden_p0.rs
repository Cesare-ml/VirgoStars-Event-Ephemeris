/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE and NOTICE; Powered by VirgoStars — https://www.virgostars.it/
 */
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

const GOLDEN_SHA256: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

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
        // On Windows, multiple parallel tests can observe the same clock tick.
        // Include an atomic sequence and create exclusively so a fixture cannot
        // overwrite a live test's file and then delete it on Drop.
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "virgostars-p0-fixture-{}-{nanos}-{sequence}.bin", std::process::id()
        ));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        file.write_all(bytes).unwrap();
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn parallel_test_fixtures_have_distinct_live_paths() {
    // Simulates concurrent allocations within one OS clock tick.
    let bytes = fixture();
    let fixtures: Vec<_> = (0..64).map(|_| TempFile::new(&bytes)).collect();
    let paths: std::collections::HashSet<_> = fixtures.iter().map(|file| &file.0).collect();
    assert_eq!(paths.len(), fixtures.len());
    // None of the fixture paths may have been replaced or removed by a peer.
    for file in &fixtures {
        assert_eq!(fs::read(&file.0).unwrap(), bytes);
        let archive = P0Archive::open(&file.0).unwrap();
        assert_eq!(archive.record_count(), 8);
    }
}

fn stamp(value: i64) -> TimestampMicros {
    TimestampMicros::new(value)
}

fn body(value: &str) -> BodyId {
    BodyId::try_from(value).unwrap()
}

#[test]
fn exhaustive_audit_checks_every_golden_block_and_family_count() {
    let file = TempFile::new(&fixture());
    let archive = P0Archive::open(&file.0).unwrap();
    let audit = archive.verify_all().unwrap();
    assert_eq!(audit.records, 8);
    assert_eq!(audit.blocks, 4);
    assert_eq!(audit.by_family.get(&EventFamily::Ingress), Some(&3));
    assert_eq!(audit.by_family.get(&EventFamily::Motion), Some(&2));
    assert_eq!(audit.by_family.get(&EventFamily::Synod), Some(&3));
}

#[test]
fn exhaustive_audit_detects_corruption_in_an_unqueried_block() {
    let mut altered = fixture();
    let header_length = u32::from_le_bytes(altered[8..12].try_into().unwrap()) as usize;
    let first_compressed_byte = 12 + header_length;
    // Damage a block that is unrelated to the subsequent Mercury query.
    altered[first_compressed_byte] ^= 1;
    let file = TempFile::new(&altered);
    let archive = P0Archive::open(&file.0).unwrap();
    let mercury = body("Mercury");
    // A normal targeted query can still succeed while another block is corrupt.
    assert!(archive.at(EventFamily::Synod, &mercury, stamp(2_000_000)).unwrap().is_some());
    assert!(archive.verify_all().is_err());
}

#[test]
fn offline_cli_verifies_all_blocks_and_emits_machine_readable_counts() {
    let file = TempFile::new(&fixture());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_p0-verify"))
        .arg(&file.0)
        .output()
        .expect("Cargo should build the experimental verification CLI");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["format"], "VSEPH-P0-EXPERIMENTAL");
    assert_eq!(value["sha256"], GOLDEN_SHA256);
    assert_eq!(value["sha256_checked_against_expected"], false);
    assert_eq!(value["blocks"], 4);
    assert_eq!(value["records"], 8);
    assert_eq!(value["sections"]["ingress"], 3);
    assert_eq!(value["sections"]["motion"], 2);
    assert_eq!(value["sections"]["synod"], 3);
    assert_eq!(value["coverage_certified"], false);
}

#[test]
fn offline_cli_accepts_trusted_whole_archive_digest() {
    let file = TempFile::new(&fixture());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_p0-verify"))
        .arg(&file.0)
        .arg("--sha256")
        .arg(GOLDEN_SHA256)
        .output()
        .expect("Cargo should build P0 verification CLI");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["sha256"], GOLDEN_SHA256);
    assert_eq!(value["sha256_checked_against_expected"], true);
    assert_eq!(value["coverage_certified"], false);
}

#[test]
fn offline_cli_rejects_wrong_or_invalid_whole_file_digest() {
    let file = TempFile::new(&fixture());
    for bad in ["0".repeat(64), "ZZ".repeat(32), "abc".to_owned()] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_p0-verify"))
            .arg(&file.0)
            .arg("--sha256")
            .arg(&bad)
            .output()
            .expect("Cargo should build P0 verification CLI");
        assert!(!output.status.success(), "wrong digest was unexpectedly accepted");
        assert!(String::from_utf8_lossy(&output.stderr).contains("verification failed"));
    }
}

#[test]
fn full_file_hash_matches_golden_fixture_even_without_loading_blocks() {
    let file = TempFile::new(&fixture());
    let reader = P0Archive::open(&file.0).unwrap();
    assert_eq!(reader.archive_sha256().unwrap(), GOLDEN_SHA256);
    assert_eq!(reader.verify_archive_sha256(GOLDEN_SHA256).unwrap(), GOLDEN_SHA256);
    assert!(reader.verify_archive_sha256(&"0".repeat(64)).is_err());
}

#[test]
fn offline_cli_exits_nonzero_on_corrupt_archive() {
    let mut altered = fixture();
    let header_length = u32::from_le_bytes(altered[8..12].try_into().unwrap()) as usize;
    altered[12 + header_length] ^= 1;
    let file = TempFile::new(&altered);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_p0-verify"))
        .arg(&file.0)
        .output()
        .expect("Cargo should build the experimental verification CLI");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("verification failed"));
}

// Build checksum-valid synthetic archives to prove that source-ID checks are
// semantic, not merely block-hash corruption checks.
fn ingress_archive_for_bodies(blocks: Vec<(&str, i64, i64, usize, Vec<u8>)>) -> Vec<u8> {
    let mut payload = Vec::new();
    let mut entries = Vec::new();
    let mut total = 0usize;
    for (body, first, last, count, raw) in blocks {
        let mut encoder = flate2::write::ZlibEncoder::new(
            Vec::new(), flate2::Compression::default(),
        );
        encoder.write_all(&raw).unwrap();
        let compressed = encoder.finish().unwrap();
        entries.push(serde_json::json!({
            "body": body,
            "family": "ingress",
            "start": payload.len(),
            "length": compressed.len(),
            "raw_length": raw.len(),
            "count": count,
            "min_us": first.to_string(),
            "max_us": last.to_string(),
            "raw_sha256": format!("{:x}", Sha256::digest(&raw))
        }));
        total += count;
        payload.extend_from_slice(&compressed);
    }
    let header = serde_json::json!({
        "format": "VSEPH-P0-EXPERIMENTAL",
        "format_version": [0, 0, 1],
        "codec": "zlib",
        "block_records_max": 512,
        "sections": {"ingress": total},
        "table_of_contents": entries
    });
    let serialized = serde_json::to_vec(&header).unwrap();
    let mut bytes = b"VSEPH-P0".to_vec();
    bytes.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&serialized);
    bytes.extend_from_slice(&payload);
    bytes
}

fn ingress_only_archive(blocks: Vec<(i64, i64, usize, Vec<u8>)>) -> Vec<u8> {
    ingress_archive_for_bodies(blocks.into_iter()
        .map(|(first, last, count, raw)| ("Moon", first, last, count, raw))
        .collect())
}

#[test]
fn reused_source_id_across_different_bodies_in_same_family_rejected() {
    let mut moon = 0_i64.to_le_bytes().to_vec();
    moon.extend_from_slice(&[4, 0, 0]);
    let mut venus = 0_i64.to_le_bytes().to_vec();
    venus.extend_from_slice(&[4, 0, 1]);
    let file = TempFile::new(&ingress_archive_for_bodies(vec![
        ("Moon", 0, 0, 1, moon),
        ("Venus", 0, 0, 1, venus),
    ]));
    let archive = P0Archive::open(&file.0).unwrap();
    assert_eq!(
        archive.at(EventFamily::Ingress, &body("Venus"), stamp(0))
            .unwrap().unwrap().source_id,
        4
    );
    // Row IDs belong to the source ingress table, NOT to each body.
    assert!(archive.verify_all().is_err());
}

#[test]
fn zero_postgresql_source_id_rejected_even_if_block_hash_is_valid() {
    let mut block = 0_i64.to_le_bytes().to_vec();
    block.extend_from_slice(&[0, 0, 1]); // ID 0, delta 0, Taurus
    let file = TempFile::new(&ingress_only_archive(vec![(0, 0, 1, block)]));
    let archive = P0Archive::open(&file.0).unwrap();
    assert!(archive.at(EventFamily::Ingress, &body("Moon"), stamp(0)).is_err());
    assert!(archive.verify_all().is_err());
}

#[test]
fn reused_source_id_within_one_block_rejected_despite_valid_sha256() {
    let mut block = 0_i64.to_le_bytes().to_vec();
    block.extend_from_slice(&[4, 0, 0, 4, 10, 1]); // ID 4, twice, at 0 and 10
    let file = TempFile::new(&ingress_only_archive(vec![(0, 10, 2, block)]));
    let archive = P0Archive::open(&file.0).unwrap();
    assert!(archive.at(EventFamily::Ingress, &body("Moon"), stamp(10)).is_err());
    assert!(archive.verify_all().is_err());
}

#[test]
fn reused_source_id_across_separate_blocks_requires_full_audit() {
    let mut first = 0_i64.to_le_bytes().to_vec();
    first.extend_from_slice(&[4, 0, 0]); // Moon ID 4 at time 0
    let mut second = 10_i64.to_le_bytes().to_vec();
    second.extend_from_slice(&[4, 0, 1]); // Same ID 4 at time 10
    let file = TempFile::new(&ingress_only_archive(vec![
        (0, 0, 1, first), (10, 10, 1, second),
    ]));
    let archive = P0Archive::open(&file.0).unwrap();
    // Either individual block is correctly formed, so a targeted lookup works.
    assert_eq!(
        archive.at(EventFamily::Ingress, &body("Moon"), stamp(10))
            .unwrap().unwrap().source_id,
        4,
    );
    // The complete archive's original row identity is not consistent.
    assert!(archive.verify_all().is_err());
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
    let bad_magic_file = TempFile::new(&bad_magic);
    assert!(P0Archive::open(&bad_magic_file.0).is_err());

    let mut bad_length = original.clone();
    bad_length[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    let bad_length_file = TempFile::new(&bad_length);
    assert!(P0Archive::open(&bad_length_file.0).is_err());

    let mut trailing = original;
    trailing.extend_from_slice(b"appended");
    let trailing_file = TempFile::new(&trailing);
    assert!(P0Archive::open(&trailing_file.0).is_err());
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
    let audit = reader.verify_all().unwrap();
    assert_eq!(audit.blocks, 761);
    assert_eq!(audit.records, 380_844);
    assert_eq!(audit.by_family.get(&EventFamily::Synod), Some(&70_554));
    assert_eq!(audit.by_family.get(&EventFamily::Motion), Some(&266_516));
    assert_eq!(audit.by_family.get(&EventFamily::Ingress), Some(&43_774));
    let mercury = body("Mercury");
    let _ = reader.next(EventFamily::Synod, &mercury, stamp(0)).unwrap();
}
