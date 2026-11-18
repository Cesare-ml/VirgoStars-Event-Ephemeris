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

use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_generator::latitude_provider_compare::{
    compare_latitude_provider_samples, LatitudeSampleComparisonOptions, LatitudeSampleTolerance,
};
use virgostars_events_generator::provider_process::ProcessLatitudeProvider;
use virgostars_events_generator::{LatitudeProvider, SearchWindow};

fn compiled_mock() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let executable = dir.path().join(if cfg!(windows) {
        "mock_latitude.exe"
    } else {
        "mock_latitude"
    });
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_latitude_provider.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let result = Command::new(rustc)
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "mock rustc: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    (dir, executable)
}

fn mode_exe(executable: &PathBuf, mode: &str) -> PathBuf {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let dest = executable.with_file_name(format!("mock_latitude_{mode}{extension}"));
    std::fs::copy(executable, &dest).unwrap();
    dest
}

fn mars() -> BodyId {
    BodyId::try_from("Mars").unwrap()
}

#[test]
fn latitude_adapter_echoes_exact_microseconds_and_signed_velocity() {
    let (_directory, executable) = compiled_mock();
    let provider = ProcessLatitudeProvider::spawn(&executable).unwrap();
    assert_eq!(provider.identity().protocol, "virgostars.latitude.v1");
    assert_eq!(provider.identity().ecliptic_plane, "mean-of-date");
    for at in [i64::MIN, -1, 0, i64::MAX] {
        let sample = provider
            .sample_latitude(&mars(), TimestampMicros::new(at))
            .unwrap();
        assert_eq!(sample.latitude_degrees, 1.25);
        assert_eq!(sample.latitude_speed_degrees_per_day, -0.125);
    }
}

#[test]
fn mismatched_or_incomplete_latitude_identity_fails_before_sampling() {
    let (_directory, executable) = compiled_mock();
    for mode in ["bad_protocol", "missing_plane"] {
        let bad = mode_exe(&executable, mode);
        assert!(ProcessLatitudeProvider::spawn(&bad).is_err(), "{mode}");
    }
}

#[test]
fn wrong_echo_invalid_samples_and_malformed_output_fail_closed() {
    let (_directory, executable) = compiled_mock();
    for mode in [
        "wrong_body",
        "wrong_time",
        "invalid_latitude",
        "invalid_speed",
        "oversize",
        "disconnect",
    ] {
        let bad = mode_exe(&executable, mode);
        let provider = ProcessLatitudeProvider::spawn(&bad).unwrap();
        assert!(
            provider
                .sample_latitude(&mars(), TimestampMicros::new(123))
                .is_err(),
            "{mode} should fail"
        );
    }
}

#[test]
fn separate_external_processes_work_with_existing_sample_comparator() {
    let (_directory, executable) = compiled_mock();
    let first = ProcessLatitudeProvider::spawn(&executable).unwrap();
    let second = ProcessLatitudeProvider::spawn(&executable).unwrap();
    let (mut a_budget, mut b_budget) = (3, 3);
    let report = compare_latitude_provider_samples(
        &first,
        &second,
        &mars(),
        LatitudeSampleComparisonOptions {
            window: SearchWindow {
                start_us: 0,
                end_us: 3_000_000,
                step_us: 1_000_000,
                precision_us: 1_000_000,
            },
            tolerance: LatitudeSampleTolerance {
                latitude_degrees: 0.0,
                signed_speed_degrees_per_day: 0.0,
            },
        },
        &mut a_budget,
        &mut b_budget,
    )
    .unwrap();
    assert_eq!(report.samples_checked, 3);
    assert!(!report.has_sample_disagreements());
    assert_eq!((a_budget, b_budget), (0, 0));
}
