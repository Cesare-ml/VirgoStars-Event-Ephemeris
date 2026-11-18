/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output};

const DAY: i64 = 86_400_000_000;

fn compiled_mock() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join(if cfg!(windows) {
        "mock_latitude.exe"
    } else {
        "mock_latitude"
    });
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_latitude_provider.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let built = Command::new(rustc)
        .arg(source)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "mock rustc: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    (dir, exe)
}

fn mode_exe(exe: &PathBuf, mode: &str) -> PathBuf {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let dest = exe.with_file_name(format!("mock_latitude_{mode}{extension}"));
    std::fs::copy(exe, &dest).unwrap();
    dest
}

fn run(mode: &str, primary: &PathBuf, reference: &PathBuf) -> Output {
    run_with_primary_args(mode, primary, reference, &[])
}

fn run_with_primary_args(
    mode: &str,
    primary: &PathBuf,
    reference: &PathBuf,
    primary_args: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vseph-compare-latitudes"));
    command.args([
        "--mode",
        mode,
        "--body",
        "Mars",
        "--start-us",
        "0",
        "--end-us",
        "172800000000",
        "--step-us",
        "86400000000",
        "--precision-us",
        "1000000",
        "--primary-program",
    ]);
    command.arg(primary);
    command.arg("--reference-program");
    command.arg(reference);
    for value in primary_args {
        command.args(["--primary-arg", value]);
    }
    command.args([
        "--latitude-tolerance-deg",
        "0.01",
        "--speed-tolerance-deg-per-day",
        "0.01",
        "--event-time-tolerance-us",
        "600000000",
        "--peak-speed-tolerance-deg-per-day",
        "0.1",
        "--adaptive-depth",
        "1",
        "--max-probes",
        "3000",
    ]);
    command.output().unwrap()
}

#[test]
fn provider_program_receives_literal_args_without_shell_reinterpretation() {
    let (_directory, exe) = compiled_mock();
    let requires = mode_exe(&exe, "args_required");
    let rejected = run("samples", &requires, &exe);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    let accepted = run_with_primary_args(
        "samples",
        &requires,
        &exe,
        &["--source", "path with spaces"],
    );
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(report["samples"]["samples_checked"], 2);
}

#[test]
fn samples_mode_produces_explicit_uncertified_clean_json() {
    let (_directory, exe) = compiled_mock();
    let output = run("samples", &exe, &exe);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["kind"], "experimental_latitude_provider_comparison");
    assert_eq!(report["samples"]["samples_checked"], 2);
    assert_eq!(
        report["samples"]["disagreements"].as_array().unwrap().len(),
        0
    );
    assert!(report["events"].is_null());
    assert_eq!(report["observed_disagreements"], false);
    assert_eq!(report["identities_equal"], true);
    assert_eq!(report["provider_independence_verified"], false);
    assert_eq!(report["astronomical_accuracy_verified"], false);
    assert_eq!(report["coverage_certified"], false);
    assert_eq!(report["safe_to_import"], false);
}

#[test]
fn out_of_tolerance_samples_print_report_and_exit_one() {
    let (_directory, exe) = compiled_mock();
    let shifted = mode_exe(&exe, "shifted");
    let output = run("samples", &exe, &shifted);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["observed_disagreements"], true);
    assert_eq!(
        report["samples"]["disagreements"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        report["samples"]["maximum_absolute_latitude_offset_degrees"],
        0.5
    );
    assert_eq!(report["samples"]["disagreements"][0]["at_us"], "0");
    assert_eq!(
        report["samples"]["disagreements"][0]["speed_within_tolerance"],
        true
    );
    assert_eq!(report["safe_to_import"], false);
}

#[test]
fn incompatible_frames_and_source_errors_never_print_partial_json() {
    let (_directory, exe) = compiled_mock();
    for mode in ["different_plane", "invalid_latitude", "disconnect"] {
        let bad = mode_exe(&exe, mode);
        let output = run("samples", &exe, &bad);
        assert_eq!(output.status.code(), Some(2), "{mode}");
        assert!(output.stdout.is_empty(), "{mode}: no partial JSON");
    }
}

#[test]
fn both_mode_compares_real_subprocess_samples_and_synthetic_events() {
    let (_directory, exe) = compiled_mock();
    let analytic = mode_exe(&exe, "analytic");
    let second = mode_exe(&exe, "analytic_reference");
    let output = run("both", &analytic, &second);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["samples"]["samples_checked"], 2);
    assert_eq!(report["events"]["pairs"].as_array().unwrap().len(), 12);
    assert_eq!(
        report["events"]["unmatched_primary"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(report["observed_disagreements"], false);
    assert_eq!(report["window"]["end_us"], (2 * DAY).to_string());
    assert_eq!(report["coverage_certified"], false);
}

#[test]
fn input_validation_fails_before_launching_nonexistent_programs() {
    let missing = PathBuf::from("/no-such-latitude-provider");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vseph-compare-latitudes"));
    cmd.args([
        "--mode",
        "samples",
        "--body",
        "Mars",
        "--start-us",
        "0",
        "--end-us",
        "172800000000",
        "--step-us",
        "86400000000",
        "--precision-us",
        "1000000",
        "--primary-program",
    ]);
    cmd.arg(&missing);
    cmd.arg("--reference-program");
    cmd.arg(&missing);
    cmd.args([
        "--latitude-tolerance-deg",
        "NaN",
        "--speed-tolerance-deg-per-day",
        "0",
        "--event-time-tolerance-us",
        "0",
        "--peak-speed-tolerance-deg-per-day",
        "0",
        "--adaptive-depth",
        "1",
        "--max-probes",
        "5",
    ]);
    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("finite nonnegative"));
}
