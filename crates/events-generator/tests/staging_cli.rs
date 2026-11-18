/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

fn compiled_mock() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let exe = directory.path().join(if cfg!(windows) {
        "mock_provider.exe"
    } else {
        "mock_provider"
    });
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_provider.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let built = Command::new(rustc)
        .arg(source)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "fixture rustc: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    (directory, exe)
}

fn run(exe: &PathBuf, bad_echo: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vseph-generate-candidates"));
    command.args([
        "--mode",
        "ingresses",
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
        "--provider-program",
    ]);
    command.arg(exe);
    if bad_echo {
        command.env("MOCK_BAD_ECHO", "1");
    }
    command.output().unwrap()
}

#[test]
fn provider_process_round_trip_produces_unverified_staging_bracket() {
    let (_directory, exe) = compiled_mock();
    let output = run(&exe, false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["coverage_certified"], false);
    assert_eq!(doc["kind"], "unverified_candidate_brackets");
    assert_eq!(doc["provider"]["provider"], "synthetic-test");
    assert_eq!(doc["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(doc["candidates"][0]["sign_after_index"], 1);
    let start = doc["candidates"][0]["bracket_start_us"].as_i64().unwrap();
    let end = doc["candidates"][0]["bracket_end_us"].as_i64().unwrap();
    assert!(start <= 43_200_000_000 && end >= 43_200_000_000);
    assert!(end - start <= 1_000_000);
}

#[test]
fn wrong_body_echo_never_writes_partial_staging_output() {
    let (_directory, exe) = compiled_mock();
    let output = run(&exe, true);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("different body or instant"));
}
