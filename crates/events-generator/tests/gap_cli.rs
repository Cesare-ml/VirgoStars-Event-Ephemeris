/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 */
use base64::{engine::general_purpose::STANDARD, Engine};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SHA: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
const DAY: i64 = 86_400_000_000;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.vseph-p0");
    let bytes = STANDARD
        .decode(include_str!("../../events-p0/tests/fixtures/synthetic.p0.base64").trim())
        .unwrap();
    fs::write(&path, bytes).unwrap();
    (temp, path)
}

fn compiled_provider() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join(if cfg!(windows) {
        "mock-provider.exe"
    } else {
        "mock-provider"
    });
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_provider.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let result = Command::new(rustc)
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    (temp, executable)
}

fn call(args: &[&str]) -> serde_json::Value {
    let result = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn plan_identifies_original_moon_synod_ids_without_inventing_rows() {
    let (_dir, archive) = fixture();
    let doc = call(&[
        "--mode",
        "plan",
        "--archive",
        archive.to_str().unwrap(),
        "--sha256",
        SHA,
        "--family",
        "synods",
        "--body",
        "Moon",
        "--start-us",
        "-876544",
        "--end-us",
        "1500000",
        "--threshold-us",
        "1000",
    ]);
    assert_eq!(doc["coverage_certified"], false);
    assert_eq!(doc["dataset"]["records_verified"], 8);
    let spans = doc["sparse_spans"].as_array().unwrap();
    assert!(spans.iter().any(|span| {
        span["kind"] == "between_recorded_anchors"
            && span["previous_original_source_id"] == 10
            && span["next_original_source_id"] == 12
    }));
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 0);
}

#[test]
fn calculate_fills_staging_only_and_preserves_provenance_for_ingresses() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = (2 * DAY).to_string();
    let step = DAY.to_string();
    let doc = call(&[
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
        program.to_str().unwrap(),
    ]);
    assert_eq!(doc["generation_complete"], false);
    assert_eq!(
        doc["sparse_spans"][0]["kind"],
        "no_recorded_anchors_in_window"
    );
    assert_eq!(doc["provider"]["provider"], "synthetic-test");
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(doc["candidates"][0]["sign_after_index"], 1);
    assert!(doc["candidates"][0]["source_id"].is_null());
}

#[test]
fn moon_phase_staging_produces_four_review_only_phase_brackets() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = (40 * DAY).to_string();
    let step = (DAY / 4).to_string();
    let doc = call(&[
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
        program.to_str().unwrap(),
    ]);
    let phases: Vec<_> = doc["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["phase"].as_str().unwrap())
        .collect();
    assert!(phases.contains(&"new_moon"));
    assert!(phases.contains(&"first_quarter"));
    assert!(phases.contains(&"full_moon"));
    assert!(phases.contains(&"last_quarter"));
    assert!(!phases.is_empty());
    assert_eq!(doc["generation_complete"], false);
}

#[test]
fn half_step_rescan_detects_two_stations_hidden_between_daily_samples() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = DAY.to_string();
    let base = [
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
        program.to_str().unwrap(),
    ];
    let coarse = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .env("MOCK_DOUBLE_STATION", "1")
        .output()
        .unwrap();
    assert!(
        coarse.status.success(),
        "{}",
        String::from_utf8_lossy(&coarse.stderr)
    );
    let stage: serde_json::Value = serde_json::from_slice(&coarse.stdout).unwrap();
    assert_eq!(stage["candidates"].as_array().unwrap().len(), 0);

    let checked = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .args(["--cadence-check", "half"])
        .env("MOCK_DOUBLE_STATION", "1")
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(checked.stdout.is_empty());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("possible missed crossings"));
}

#[test]
fn half_step_calculation_budget_fails_before_provider_and_emits_no_partial_json() {
    let (_dir, archive) = fixture();
    let end = DAY.to_string();
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
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &end,
            "--precision-us",
            "1000000",
            "--max-steps",
            "2",
            "--provider-program",
            "nonexistent-mock-provider",
            "--cadence-check",
            "half",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs 3 position steps"));
}

#[test]
fn adaptive_station_rescan_detects_two_hidden_stations_and_emits_no_partial_json() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = DAY.to_string();
    let checked = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
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
            program.to_str().unwrap(),
            "--adaptive-station-depth",
            "6",
        ])
        .env("MOCK_DOUBLE_STATION", "1")
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(checked.stdout.is_empty());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("possible missed crossings"));
}

#[test]
fn adaptive_station_rescan_stages_only_with_explicit_uncertified_metadata() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = DAY.to_string();
    let doc = call(&[
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
        program.to_str().unwrap(),
        "--adaptive-station-depth",
        "6",
    ]);
    assert_eq!(doc["adaptive_station_check"]["mode"], "risk_midpoint");
    assert_eq!(doc["adaptive_station_check"]["max_depth"], 6);
    assert_eq!(doc["adaptive_station_check"]["refined_segments"], 0);
    assert_eq!(doc["adaptive_station_check"]["coverage_certified"], false);
    assert_eq!(doc["coverage_certified"], false);
    assert_eq!(doc["generation_complete"], false);
}

#[test]
fn adaptive_station_budget_exhaustion_rejects_before_provider_startup() {
    let (_dir, archive) = fixture();
    let end = DAY.to_string();
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
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &end,
            "--precision-us",
            "1000000",
            "--max-steps",
            "1",
            "--provider-program",
            "nonexistent-mock-provider",
            "--adaptive-station-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("budget exhausted"));
}

#[test]
fn adaptive_ingress_scan_detects_forward_and_retrograde_reentry_hidden_by_daily_sampling() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = DAY.to_string();
    let base = [
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
        &end,
        "--precision-us",
        "1000000",
        "--provider-program",
        program.to_str().unwrap(),
    ];
    let coarse = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .env("MOCK_DOUBLE_INGRESS", "1")
        .output()
        .unwrap();
    assert!(
        coarse.status.success(),
        "{}",
        String::from_utf8_lossy(&coarse.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&coarse.stdout).unwrap();
    assert!(doc["candidates"].as_array().unwrap().is_empty());

    let checked = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .args(["--adaptive-ingress-depth", "6"])
        .env("MOCK_DOUBLE_INGRESS", "1")
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(checked.stdout.is_empty());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("possible missed crossings"));
}

#[test]
fn normal_ingress_adaptive_check_emits_uncertified_evidence_only() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = (2 * DAY).to_string();
    let step = DAY.to_string();
    let doc = call(&[
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
        program.to_str().unwrap(),
        "--adaptive-ingress-depth",
        "6",
    ]);
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(
        doc["adaptive_ingress_check"]["mode"],
        "zodiac_boundary_midpoint"
    );
    assert_eq!(doc["adaptive_ingress_check"]["max_depth"], 6);
    assert_eq!(doc["adaptive_ingress_check"]["matched_candidates"], 1);
    assert_eq!(doc["adaptive_ingress_check"]["coverage_certified"], false);
    assert_eq!(doc["generation_complete"], false);
}

#[test]
fn adaptive_ingress_budget_fails_before_provider_launch() {
    let (_dir, archive) = fixture();
    let end = DAY.to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
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
            &end,
            "--precision-us",
            "1000000",
            "--max-steps",
            "1",
            "--provider-program",
            "nonexistent-mock-provider",
            "--adaptive-ingress-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("budget exhausted"));
}

#[test]
fn adaptive_ingress_option_rejects_motion_family_without_starting_provider() {
    let (_dir, archive) = fixture();
    let end = DAY.to_string();
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
            &end,
            "--threshold-us",
            "1000000000",
            "--step-us",
            &end,
            "--precision-us",
            "1000000",
            "--provider-program",
            "nonexistent-mock-provider",
            "--adaptive-ingress-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires family ingresses"));
}

#[test]
fn adaptive_lunar_crosscheck_rejects_full_turn_alias_hidden_between_daily_samples() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = DAY.to_string();
    let base = [
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
        &end,
        "--precision-us",
        "1000000",
        "--provider-program",
        program.to_str().unwrap(),
    ];
    let coarse = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .env("MOCK_LUNAR_ALIAS", "1")
        .output()
        .unwrap();
    assert!(
        coarse.status.success(),
        "{}",
        String::from_utf8_lossy(&coarse.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&coarse.stdout).unwrap();
    assert!(doc["candidates"].as_array().unwrap().is_empty());

    let checked = Command::new(env!("CARGO_BIN_EXE_vseph-fill-gaps"))
        .args(base)
        .args(["--adaptive-lunar-depth", "6"])
        .env("MOCK_LUNAR_ALIAS", "1")
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(checked.stdout.is_empty());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("ambiguous lunar phase"));
}

#[test]
fn ordinary_moon_phases_pass_adaptive_lunar_staging_without_coverage_claim() {
    let (_dir, archive) = fixture();
    let (_program_dir, program) = compiled_provider();
    let end = (40 * DAY).to_string();
    let step = (DAY / 4).to_string();
    let doc = call(&[
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
        program.to_str().unwrap(),
        "--adaptive-lunar-depth",
        "6",
    ]);
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 5);
    assert_eq!(doc["adaptive_lunar_check"]["mode"], "phase_angle_midpoint");
    assert_eq!(doc["adaptive_lunar_check"]["max_depth"], 6);
    assert_eq!(doc["adaptive_lunar_check"]["matched_candidates"], 5);
    assert_eq!(doc["adaptive_lunar_check"]["coverage_certified"], false);
    assert_eq!(doc["generation_complete"], false);
}

#[test]
fn adaptive_lunar_probe_budget_rejects_before_provider_launch() {
    let (_dir, archive) = fixture();
    let end = DAY.to_string();
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
            &end,
            "--precision-us",
            "1000000",
            "--max-steps",
            "1",
            "--provider-program",
            "nonexistent-mock-provider",
            "--adaptive-lunar-depth",
            "6",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("budget exhausted"));
}
