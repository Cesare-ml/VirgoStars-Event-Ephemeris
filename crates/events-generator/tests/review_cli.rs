/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 */
use base64::{engine::general_purpose::STANDARD, Engine};
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
    if let Some(name) = name {
        command.env("MOCK_PROVIDER_NAME", name);
    }
    if let Some(offset) = offset {
        command.env("MOCK_LONGITUDE_OFFSET", offset);
    }
    command.output().unwrap()
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
