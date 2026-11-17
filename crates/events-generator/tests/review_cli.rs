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
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn review(archive: &Path, stage: &serde_json::Value, digest: &str) -> Output {
    let staging = tempfile::NamedTempFile::new().unwrap();
    fs::write(staging.path(), serde_json::to_vec(stage).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_vseph-review-candidates"))
        .args([
            "--archive",
            archive.to_str().unwrap(),
            "--sha256",
            digest,
            "--staging",
            staging.path().to_str().unwrap(),
        ])
        .output()
        .unwrap()
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
