/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 */
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SHA: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
const DAY: i64 = 86_400_000_000;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("approved.p0");
    fs::write(
        &path,
        STANDARD
            .decode(include_str!("../../events-p0/tests/fixtures/synthetic.p0.base64").trim())
            .unwrap(),
    )
    .unwrap();
    (directory, path)
}

fn provider() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join(if cfg!(windows) {
        "mock_provider.exe"
    } else {
        "mock_provider"
    });
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_provider.rs");
    let built = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")))
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    (directory, executable)
}

fn generate(archive: &Path, provider: Option<&Path>) -> serde_json::Value {
    generate_with_cadence(archive, provider, false)
}

fn generate_with_cadence(
    archive: &Path,
    provider: Option<&Path>,
    half_step: bool,
) -> serde_json::Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"));
    cmd.args([
        "--mode",
        if provider.is_some() {
            "calculate"
        } else {
            "plan"
        },
        "--archive",
        archive.to_str().unwrap(),
        "--sha256",
        SHA,
        "--family",
        "ingresses",
        "--body",
        "Mars",
        "--start-us",
        "0",
        "--end-us",
        "172800000000",
        "--threshold-us",
        "1000000000",
    ]);
    if let Some(provider) = provider {
        cmd.args([
            "--step-us",
            &DAY.to_string(),
            "--precision-us",
            "1000000",
            "--provider-program",
            provider.to_str().unwrap(),
        ]);
    }
    if half_step {
        cmd.args(["--cadence-check", "half"]);
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn generate_lunar(archive: &Path, provider: &Path) -> serde_json::Value {
    let end = (40 * DAY).to_string();
    let step = (DAY / 4).to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            "synods",
            "--body",
            "Moon",
            "--start-us",
            "0",
            "--end-us",
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &step,
            "--precision-us",
            "1000000",
            "--provider-program",
            provider.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn generate_motion_stations(archive: &Path, provider: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            "motions",
            "--body",
            "Mars",
            "--start-us",
            "0",
            "--end-us",
            &DAY.to_string(),
            "--threshold-us",
            "1000000000",
            "--step-us",
            &(DAY / 8).to_string(),
            "--precision-us",
            "1000000",
            "--provider-program",
            provider.to_str().unwrap(),
        ])
        .env("MOCK_DOUBLE_STATION", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn generate_hidden_pair(archive: &Path, provider: &Path, family: &str) -> serde_json::Value {
    let flag = if family == "motions" {
        "MOCK_DOUBLE_STATION"
    } else {
        "MOCK_DOUBLE_INGRESS"
    };
    let output = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            family,
            "--body",
            "Mars",
            "--start-us",
            "0",
            "--end-us",
            &DAY.to_string(),
            "--threshold-us",
            "1000000000",
            "--step-us",
            &DAY.to_string(),
            "--precision-us",
            "1000000",
            "--provider-program",
            provider.to_str().unwrap(),
        ])
        .env(flag, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stage: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(stage["candidates"].as_array().unwrap().is_empty());
    assert!(!stage["sparse_spans"].as_array().unwrap().is_empty());
    stage
}

fn review(archive: &Path, stage: &serde_json::Value, digest: &str) -> Output {
    review_with_reference(archive, stage, digest, None, None, None)
}

fn review_with_reference(
    archive: &Path,
    stage: &serde_json::Value,
    digest: &str,
    provider: Option<&Path>,
    name: Option<&str>,
    offset: Option<&str>,
) -> Output {
    review_with_rescan(archive, stage, digest, provider, name, offset, None, None)
}

#[allow(clippy::too_many_arguments)]
fn review_with_rescan(
    archive: &Path,
    stage: &serde_json::Value,
    digest: &str,
    provider: Option<&Path>,
    name: Option<&str>,
    offset: Option<&str>,
    rescan_step: Option<&str>,
    hidden_pair_flag: Option<&str>,
) -> Output {
    review_with_rescan_and_digest(
        archive,
        stage,
        digest,
        provider,
        name,
        offset,
        rescan_step,
        hidden_pair_flag,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn review_with_rescan_and_digest(
    archive: &Path,
    stage: &serde_json::Value,
    digest: &str,
    provider: Option<&Path>,
    name: Option<&str>,
    offset: Option<&str>,
    rescan_step: Option<&str>,
    hidden_pair_flag: Option<&str>,
    trusted_reference_executable_sha256: Option<&str>,
) -> Output {
    let staging = tempfile::NamedTempFile::new().unwrap();
    fs::write(staging.path(), serde_json::to_vec(stage).unwrap()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_vseph-review-candidates"));
    command.args([
        "--archive",
        archive.to_str().unwrap(),
        "--sha256",
        digest,
        "--staging",
        staging.path().to_str().unwrap(),
    ]);
    if let Some(provider) = provider {
        command.args(["--reference-provider-program", provider.to_str().unwrap()]);
    }
    if let Some(trusted_digest) = trusted_reference_executable_sha256 {
        command.args(["--reference-provider-sha256", trusted_digest]);
    }
    if let Some(rescan_step) = rescan_step {
        command.args(["--reference-rescan-step-us", rescan_step]);
    }
    if let Some(name) = name {
        command.env("MOCK_PROVIDER_NAME", name);
    }
    if let Some(offset) = offset {
        command.env("MOCK_LONGITUDE_OFFSET", offset);
    }
    if let Some(hidden_pair_flag) = hidden_pair_flag {
        command.env(hidden_pair_flag, "1");
    }
    command.output().unwrap()
}

fn independent_event_fixture(stage: &serde_json::Value) -> serde_json::Value {
    let spans: Vec<_> = stage["sparse_spans"]
        .as_array()
        .unwrap()
        .iter()
        .map(|span| {
            serde_json::json!({
                "start_us": span["start_us"],
                "end_us": span["end_us"]
            })
        })
        .collect();
    let events: Vec<_> = stage["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| {
            let begin = candidate["bracket_start_us"].as_i64().unwrap();
            let end = candidate["bracket_end_us"].as_i64().unwrap();
            let mut event = serde_json::json!({
                "gap_index": candidate["gap_index"],
                "at_us": begin + (end - begin) / 2
            });
            for key in ["direction_after", "sign_after_index", "retrograde", "phase"] {
                if let Some(value) = candidate.get(key) {
                    event[key] = value.clone();
                }
            }
            event
        })
        .collect();
    serde_json::json!({
        "schema": "vseph-reference-events-v1",
        "source": "synthetic-independent-oracle",
        "version": "test-1",
        "family": stage["family"],
        "body": stage["body"],
        "requested_window": stage["requested_window"],
        "sparse_spans": spans,
        "events": events
    })
}

fn review_event_fixture(
    archive: &Path,
    stage: &serde_json::Value,
    golden: &serde_json::Value,
    tolerance: Option<&str>,
    corrupt_digest: bool,
) -> Output {
    let staging = tempfile::NamedTempFile::new().unwrap();
    fs::write(staging.path(), serde_json::to_vec(stage).unwrap()).unwrap();
    let reference = tempfile::NamedTempFile::new().unwrap();
    let bytes = serde_json::to_vec(golden).unwrap();
    fs::write(reference.path(), &bytes).unwrap();
    let digest = if corrupt_digest {
        "0".repeat(64)
    } else {
        format!("{:x}", Sha256::digest(&bytes))
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_vseph-review-candidates"));
    command.args([
        "--archive",
        archive.to_str().unwrap(),
        "--sha256",
        SHA,
        "--staging",
        staging.path().to_str().unwrap(),
        "--reference-events",
        reference.path().to_str().unwrap(),
        "--reference-events-sha256",
        &digest,
    ]);
    if let Some(tolerance) = tolerance {
        command.args(["--reference-tolerance-us", tolerance]);
    }
    command.output().unwrap()
}

#[test]
fn authenticated_reference_events_compare_all_supported_event_families() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    for stage in [
        generate(&archive, Some(&mock)),
        generate_lunar(&archive, &mock),
        generate_motion_stations(&archive, &mock),
    ] {
        assert!(!stage["candidates"].as_array().unwrap().is_empty());
        let golden = independent_event_fixture(&stage);
        let output = review_event_fixture(&archive, &stage, &golden, None, false);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["reference_event_digest_authenticated"], true);
        assert_eq!(
            report["reference_event_timestamps_matched"],
            stage["candidates"].as_array().unwrap().len()
        );
        assert_eq!(report["reference_events_correspondence_consistent"], true);
        assert_eq!(report["reference_events_scientifically_independent"], false);
        assert_eq!(report["reference_timestamp_tolerance_us"], 0);
        assert_eq!(report["reference_max_abs_midpoint_offset_us"], 0);
        assert_eq!(report["reference_max_outside_bracket_us"], 0);
        assert_eq!(report["reference_events_requiring_tolerance"], 0);
        assert_eq!(report["astronomical_accuracy_verified"], false);
        assert_eq!(report["safe_to_import"], false);
    }
}

#[test]
fn reference_event_comparison_fails_closed_on_bad_trust_and_false_golden_data() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let golden = independent_event_fixture(&stage);
    assert!(!review_event_fixture(&archive, &stage, &golden, None, true)
        .status
        .success());

    let mut wrong_time = golden.clone();
    wrong_time["events"][0]["at_us"] = serde_json::json!(0);
    let failed = review_event_fixture(&archive, &stage, &wrong_time, None, false);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());

    let mut wrong_sign = golden.clone();
    wrong_sign["events"][0]["sign_after_index"] = serde_json::json!(11);
    assert!(
        !review_event_fixture(&archive, &stage, &wrong_sign, None, false)
            .status
            .success()
    );

    let mut missing = golden.clone();
    missing["events"] = serde_json::json!([]);
    assert!(
        !review_event_fixture(&archive, &stage, &missing, None, false)
            .status
            .success()
    );

    let mut extra = golden.clone();
    extra["events"]
        .as_array_mut()
        .unwrap()
        .push(golden["events"][0].clone());
    assert!(!review_event_fixture(&archive, &stage, &extra, None, false)
        .status
        .success());

    let mut wrong_spans = golden.clone();
    wrong_spans["sparse_spans"][0]["end_us"] = serde_json::json!(42);
    assert!(
        !review_event_fixture(&archive, &stage, &wrong_spans, None, false)
            .status
            .success()
    );

    let mut invented_field = golden.clone();
    invented_field["events"][0]["source_id"] = serde_json::json!(12);
    assert!(
        !review_event_fixture(&archive, &stage, &invented_field, None, false)
            .status
            .success()
    );

    let precision = stage["calculation_settings"]["precision_us"]
        .as_i64()
        .unwrap();
    assert!(!review_event_fixture(
        &archive,
        &stage,
        &golden,
        Some(&(precision + 1).to_string()),
        false
    )
    .status
    .success());
}

#[test]
fn reference_time_tolerance_has_an_explicit_precision_cap() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let mut golden = independent_event_fixture(&stage);
    let end = stage["candidates"][0]["bracket_end_us"].as_i64().unwrap();
    golden["events"][0]["at_us"] = serde_json::json!(end + 50);
    let strict = review_event_fixture(&archive, &stage, &golden, None, false);
    assert!(!strict.status.success());
    let relaxed = review_event_fixture(&archive, &stage, &golden, Some("50"), false);
    assert!(
        relaxed.status.success(),
        "{}",
        String::from_utf8_lossy(&relaxed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&relaxed.stdout).unwrap();
    assert_eq!(report["reference_timestamp_tolerance_us"], 50);
    assert_eq!(report["reference_max_outside_bracket_us"], 50);
    assert_eq!(report["reference_events_requiring_tolerance"], 1);
    let start = stage["candidates"][0]["bracket_start_us"].as_i64().unwrap();
    let middle = start + (end - start) / 2;
    assert_eq!(
        report["reference_max_abs_midpoint_offset_us"],
        (end + 50 - middle) as u64
    );
}

#[test]
fn reference_metric_is_measured_even_when_no_tolerance_is_needed() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let mut golden = independent_event_fixture(&stage);
    let bracket = &stage["candidates"][0];
    let end = bracket["bracket_end_us"].as_i64().unwrap();
    let start = bracket["bracket_start_us"].as_i64().unwrap();
    golden["events"][0]["at_us"] = serde_json::json!(end);
    let result = review_event_fixture(&archive, &stage, &golden, None, false);
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["reference_max_abs_midpoint_offset_us"],
        (end - start + 1) / 2
    );
    assert_eq!(report["reference_max_outside_bracket_us"], 0);
    assert_eq!(report["reference_events_requiring_tolerance"], 0);
}

#[test]
fn independently_recalculates_original_gaps_for_plan_and_calculation() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    for doc in [generate(&archive, None), generate(&archive, Some(&mock))] {
        let output = review(&archive, &doc, SHA);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["source_archive_authenticated"], true);
        assert_eq!(report["staging_structure_verified"], true);
        assert_eq!(report["coverage_certified"], false);
        assert_eq!(report["astronomical_accuracy_verified"], false);
        assert_eq!(report["safe_to_import"], false);
        assert!(report["reference_timestamp_tolerance_us"].is_null());
        assert!(report["reference_max_abs_midpoint_offset_us"].is_null());
        assert!(report["reference_max_outside_bracket_us"].is_null());
        assert!(report["reference_events_requiring_tolerance"].is_null());
        assert_eq!(
            report["candidate_brackets_checked"],
            doc["candidates"].as_array().unwrap().len()
        );
    }
}

#[test]
fn rejects_tampered_anchors_even_with_correct_archive_digest() {
    let (_original, archive) = fixture();
    let mut doc = generate(&archive, None);
    assert!(!doc["sparse_spans"].as_array().unwrap().is_empty());
    doc["sparse_spans"][0]["end_us"] = serde_json::json!(100_000_000_000_i64);
    let output = review(&archive, &doc, SHA);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("does not match original record anchors")
    );
}

#[test]
fn rejects_forged_candidate_id_and_bad_bracket_precision() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate(&archive, Some(&mock));
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 1);

    let mut injected = doc.clone();
    injected["candidates"][0]["source_id"] = serde_json::json!(123);
    assert!(!review(&archive, &injected, SHA).status.success());

    let mut oversized = doc.clone();
    oversized["candidates"][0]["bracket_end_us"] = serde_json::json!(2 * DAY);
    assert!(!review(&archive, &oversized, SHA).status.success());

    let mut wrong_family = doc.clone();
    wrong_family["candidates"][0]["family"] = serde_json::json!("motions");
    assert!(!review(&archive, &wrong_family, SHA).status.success());

    let mut spoofed_provider = doc;
    spoofed_provider["provider"]["frame"] = serde_json::json!("heliocentric");
    assert!(!review(&archive, &spoofed_provider, SHA).status.success());
}

#[test]
fn rejects_self_asserted_dataset_digest_and_untrusted_input_archive() {
    let (_original, archive) = fixture();
    let mut doc = generate(&archive, None);
    doc["dataset"]["sha256"] = serde_json::json!("0".repeat(64));
    assert!(!review(&archive, &doc, SHA).status.success());
    let untrusted_digest = "0".repeat(64);
    assert!(
        !review(&archive, &generate(&archive, None), &untrusted_digest)
            .status
            .success()
    );
}

#[test]
fn independent_numeric_crossing_check_accepts_matching_synthetic_ingress() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate(&archive, Some(&mock));
    let reviewed = review_with_reference(
        &archive,
        &doc,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
    );
    assert!(
        reviewed.status.success(),
        "{}",
        String::from_utf8_lossy(&reviewed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&reviewed.stdout).unwrap();
    assert_eq!(report["reference_crossings_consistent"], true);
    assert_eq!(report["numerical_crossings_checked"], 1);
    assert_eq!(
        report["reference_provider"]["provider"],
        "synthetic-reference"
    );
    assert_eq!(report["astronomical_accuracy_verified"], false);
    assert_eq!(
        report["reference_independence_scientifically_established"],
        false
    );
    assert_eq!(report["safe_to_import"], false);
}

#[test]
fn separately_pinned_reference_executable_is_hashed_before_numerical_rescan() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let trusted_sha = format!("{:x}", Sha256::digest(fs::read(&mock).unwrap()));
    let passed = review_with_rescan_and_digest(
        &archive,
        &stage,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
        Some(&(DAY / 2).to_string()),
        None,
        Some(&trusted_sha),
    );
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&passed.stdout).unwrap();
    assert_eq!(report["reference_provider_digest_authenticated"], true);
    assert_eq!(report["reference_provider_executable_sha256"], trusted_sha);
    assert_eq!(report["reference_rescan_performed"], true);
    assert_eq!(
        report["reference_independence_scientifically_established"],
        false
    );
    assert_eq!(report["safe_to_import"], false);

    let without_pin = review_with_reference(
        &archive,
        &stage,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
    );
    assert!(without_pin.status.success());
    let unpinned: serde_json::Value = serde_json::from_slice(&without_pin.stdout).unwrap();
    assert_eq!(unpinned["reference_provider_digest_authenticated"], false);
    assert!(unpinned["reference_provider_executable_sha256"].is_null());
}

#[test]
fn reference_executable_pinning_rejects_wrong_digest_invalid_inputs_and_missing_provider() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let trusted_sha = format!("{:x}", Sha256::digest(fs::read(&mock).unwrap()));
    for bad in ["0".repeat(64), "bad".into(), trusted_sha.to_uppercase()] {
        let rejected = review_with_rescan_and_digest(
            &archive,
            &stage,
            SHA,
            Some(&mock),
            Some("synthetic-reference"),
            None,
            None,
            None,
            Some(&bad),
        );
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
    }
    let no_program = review_with_rescan_and_digest(
        &archive,
        &stage,
        SHA,
        None,
        None,
        None,
        None,
        None,
        Some(&trusted_sha),
    );
    assert!(!no_program.status.success());
    assert!(no_program.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn reference_executable_pin_rejects_symlink_programs() {
    use std::os::unix::fs::symlink;
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    let directory = tempfile::tempdir().unwrap();
    let link = directory.path().join("reference-link");
    symlink(&mock, &link).unwrap();
    let trusted_sha = format!("{:x}", Sha256::digest(fs::read(&mock).unwrap()));
    let rejected = review_with_rescan_and_digest(
        &archive,
        &stage,
        SHA,
        Some(&link),
        Some("synthetic-reference"),
        None,
        None,
        None,
        Some(&trusted_sha),
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("non-symlinked"));
}

#[test]
fn second_provider_full_span_rescan_matches_ingresses_and_lunar_phases() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    for (stage, step, expected) in [
        (generate(&archive, Some(&mock)), DAY / 2, 1),
        (generate_lunar(&archive, &mock), DAY / 8, 5),
    ] {
        let reviewed = review_with_rescan(
            &archive,
            &stage,
            SHA,
            Some(&mock),
            Some("synthetic-reference"),
            None,
            Some(&step.to_string()),
            None,
        );
        assert!(
            reviewed.status.success(),
            "{}",
            String::from_utf8_lossy(&reviewed.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&reviewed.stdout).unwrap();
        assert_eq!(report["reference_rescan_performed"], true);
        assert_eq!(report["reference_rescan_correspondence_consistent"], true);
        assert_eq!(report["reference_rescan_step_us"], step);
        assert_eq!(report["reference_rescan_candidates_matched"], expected);
        assert!(report["reference_rescan_grid_steps"].as_i64().unwrap() > 0);
        assert_eq!(report["safe_to_import"], false);
        assert_eq!(report["coverage_certified"], false);
        assert_eq!(report["astronomical_accuracy_verified"], false);
    }
}

#[test]
fn full_span_rescan_finds_hidden_station_and_ingress_pairs_in_empty_staging() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    for (family, flag) in [
        ("motions", "MOCK_DOUBLE_STATION"),
        ("ingresses", "MOCK_DOUBLE_INGRESS"),
    ] {
        let stage = generate_hidden_pair(&archive, &mock, family);
        let failed = review_with_rescan(
            &archive,
            &stage,
            SHA,
            Some(&mock),
            Some("synthetic-reference"),
            None,
            Some(&(DAY / 8).to_string()),
            Some(flag),
        );
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failed.stderr)
            .contains("reference full-span rescan found 2 candidates, staging has 0"));
    }
}

#[test]
fn full_span_rescan_detects_omitted_brackets_in_otherwise_valid_staging() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let mut stage = generate(&archive, Some(&mock));
    stage["candidates"] = serde_json::json!([]);
    // An empty candidate list is structurally allowed: only the independent
    // per-span rescan can detect the missing event.
    assert!(review(&archive, &stage, SHA).status.success());
    let failed = review_with_rescan(
        &archive,
        &stage,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
        Some(&(DAY / 2).to_string()),
        None,
    );
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr)
        .contains("reference full-span rescan found 1 candidates, staging has 0"));
}

#[test]
fn full_span_rescan_can_check_empty_intervals_without_inventing_events() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate_hidden_pair(&archive, &mock, "motions");
    let checked = review_with_rescan(
        &archive,
        &stage,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
        Some(&(DAY / 8).to_string()),
        None,
    );
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["reference_rescan_candidates_matched"], 0);
    assert_eq!(report["reference_rescan_performed"], true);
    assert_eq!(report["coverage_certified"], false);
}

#[test]
fn full_span_rescan_limits_budget_and_rejects_unsupported_arguments() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let stage = generate(&archive, Some(&mock));
    for step in ["1", "0", "-1", "invalid", &DAY.to_string()] {
        let failed = review_with_rescan(
            &archive,
            &stage,
            SHA,
            Some(&mock),
            Some("synthetic-reference"),
            None,
            Some(step),
            None,
        );
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }
    let no_provider = review_with_rescan(
        &archive,
        &stage,
        SHA,
        None,
        None,
        None,
        Some(&(DAY / 2).to_string()),
        None,
    );
    assert!(!no_provider.status.success());
    assert!(no_provider.stdout.is_empty());
    let planned = generate(&archive, None);
    let plan_rescan = review_with_rescan(
        &archive,
        &planned,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
        Some(&(DAY / 2).to_string()),
        None,
    );
    assert!(!plan_rescan.status.success());
    assert!(plan_rescan.stdout.is_empty());
}

#[test]
fn numerical_crossing_review_rejects_wrong_sign_or_reference_positions() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate(&archive, Some(&mock));
    let mut forged = doc.clone();
    forged["candidates"][0]["sign_after_index"] = serde_json::json!(2);
    let mismatched = review_with_reference(
        &archive,
        &forged,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
    );
    assert!(!mismatched.status.success());
    assert!(mismatched.stdout.is_empty());

    let shifted = review_with_reference(
        &archive,
        &doc,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        Some("10"),
    );
    assert!(!shifted.status.success());
    assert!(String::from_utf8_lossy(&shifted.stderr).contains("reference numerical check failed"));
    assert!(shifted.stdout.is_empty());
}

#[test]
fn self_declared_same_provider_is_rejected_as_independent_reference() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate(&archive, Some(&mock));
    let output = review_with_reference(&archive, &doc, SHA, Some(&mock), None, None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SAME identity"));

    let planned = generate(&archive, None);
    assert!(
        !review_with_reference(&archive, &planned, SHA, Some(&mock), None, None)
            .status
            .success()
    );
}

#[test]
fn numerical_reference_checks_all_four_lunar_phase_types() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate_lunar(&archive, &mock);
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 5);
    let checked = review_with_reference(
        &archive,
        &doc,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
    );
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["numerical_crossings_checked"], 5);

    let mut false_phase = doc;
    false_phase["candidates"][0]["phase"] = serde_json::json!("new_moon");
    let rejected = review_with_reference(
        &archive,
        &false_phase,
        SHA,
        Some(&mock),
        Some("synthetic-reference"),
        None,
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn reviewer_checks_half_step_metadata_without_claiming_to_recompute_cadence() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let doc = generate_with_cadence(&archive, Some(&mock), true);
    assert_eq!(doc["cadence_check"]["mode"], "half_step");
    assert_eq!(doc["cadence_check"]["matched_candidates"], 1);
    let checked = review(&archive, &doc, SHA);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["producer_claimed_half_step_check"], true);
    assert_eq!(report["cadence_check_independently_recomputed"], false);
    assert_eq!(report["safe_to_import"], false);

    let mut forged = doc;
    forged["cadence_check"]["matched_candidates"] = serde_json::json!(77);
    let rejected = review(&archive, &forged, SHA);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn reviewer_validates_adaptive_station_claim_without_promoting_it_to_coverage() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let end = DAY.to_string();
    let generated = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            "motions",
            "--body",
            "Mars",
            "--start-us",
            "0",
            "--end-us",
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &end,
            "--precision-us",
            "1000000",
            "--provider-program",
            mock.to_str().unwrap(),
            "--adaptive-station-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    let checked = review(&archive, &doc, SHA);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["producer_claimed_adaptive_station_check"], true);
    assert_eq!(
        report["adaptive_station_check_independently_recomputed"],
        false
    );
    assert_eq!(report["coverage_certified"], false);
    assert_eq!(report["safe_to_import"], false);

    let mut forged = doc;
    forged["adaptive_station_check"]["matched_candidates"] = serde_json::json!(999);
    let rejected = review(&archive, &forged, SHA);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn reviewer_checks_ingress_adaptive_claim_and_rejects_forged_metadata() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let end = (2 * DAY).to_string();
    let step = DAY.to_string();
    let generated = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            "ingresses",
            "--body",
            "Mars",
            "--start-us",
            "0",
            "--end-us",
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &step,
            "--precision-us",
            "1000000",
            "--provider-program",
            mock.to_str().unwrap(),
            "--adaptive-ingress-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    let reviewed = review(&archive, &doc, SHA);
    assert!(
        reviewed.status.success(),
        "{}",
        String::from_utf8_lossy(&reviewed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&reviewed.stdout).unwrap();
    assert_eq!(report["producer_claimed_adaptive_ingress_check"], true);
    assert_eq!(
        report["adaptive_ingress_check_independently_recomputed"],
        false
    );
    assert_eq!(report["coverage_certified"], false);
    assert_eq!(report["safe_to_import"], false);

    let mut forged = doc.clone();
    forged["adaptive_ingress_check"]["max_depth"] = serde_json::json!(9);
    let rejected = review(&archive, &forged, SHA);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());

    let mut wrong_mode = doc;
    wrong_mode["adaptive_ingress_check"]["mode"] = serde_json::json!("risk_midpoint");
    let rejected = review(&archive, &wrong_mode, SHA);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}

#[test]
fn reviewer_checks_lunar_adaptive_claim_without_claiming_scientific_completeness() {
    let (_original, archive) = fixture();
    let (_mock_dir, mock) = provider();
    let end = (40 * DAY).to_string();
    let step = (DAY / 4).to_string();
    let generated = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args([
            "--mode",
            "calculate",
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            SHA,
            "--family",
            "synods",
            "--body",
            "Moon",
            "--start-us",
            "0",
            "--end-us",
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &step,
            "--precision-us",
            "1000000",
            "--provider-program",
            mock.to_str().unwrap(),
            "--adaptive-lunar-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&generated.stdout).unwrap();
    let reviewed = review(&archive, &doc, SHA);
    assert!(
        reviewed.status.success(),
        "{}",
        String::from_utf8_lossy(&reviewed.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&reviewed.stdout).unwrap();
    assert_eq!(report["producer_claimed_adaptive_lunar_check"], true);
    assert_eq!(
        report["adaptive_lunar_check_independently_recomputed"],
        false
    );
    assert_eq!(report["coverage_certified"], false);
    assert_eq!(report["safe_to_import"], false);

    let mut forged = doc;
    forged["adaptive_lunar_check"]["matched_candidates"] = serde_json::json!(123);
    let rejected = review(&archive, &forged, SHA);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
}
