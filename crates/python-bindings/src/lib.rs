/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! Minimal Python boundary for recorded Rust P0 synods. No scientific
//! reimplementation exists on the Python side; only time/JSON conversion.

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;
use serde_json::{json, Map, Value};
use virgostars_events_core::{
    Event, TimestampMicros, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_MICROS,
};
use virgostars_events_p0::{P0Archive, P0Error};

fn as_py_error(err: P0Error) -> PyErr {
    match err {
        P0Error::Io(io) => PyOSError::new_err(io.to_string()),
        P0Error::Invalid(message) | P0Error::Unsupported(message) => {
            PyValueError::new_err(message)
        }
    }
}

/// Python-visible, read-only, precomputed event archive.
#[pyclass]
struct NativeReader {
    archive: P0Archive,
    sha256: String,
}

#[pymethods]
impl NativeReader {
    #[new]
    fn new(path: &str, expected_sha256: &str) -> PyResult<Self> {
        let archive = P0Archive::open(path).map_err(as_py_error)?;
        let sha256 = archive
            .verify_archive_sha256(expected_sha256)
            .map_err(as_py_error)?;
        Ok(Self { archive, sha256 })
    }

    /// Internal JSON transport; public Python wrapper returns a native dict.
    fn get_synods_at_us(&self, requested_at_us: i64) -> PyResult<String> {
        let t = TimestampMicros::new(requested_at_us);
        let snapshots = self.archive.synods_at(t).map_err(as_py_error)?;
        let mut entries = Map::new();

        for (body, snapshot) in snapshots {
            // The P0 archive is immutable for pagination but may contain
            // scientifically missing lunar cycles. Retain its last recorded
            // new moon without claiming CAL2 across a year-long gap.
            let has_moon_gap = body.as_str() == "Moon"
                && snapshot.record.as_ref().is_some_and(|record| {
                    snapshot.next_recorded_start.is_some_and(|next| {
                        next.as_i64().saturating_sub(record.at().as_i64())
                            > MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_MICROS
                    })
                });
            let mut warnings = vec!["no_reviewed_coverage_policy"];
            if has_moon_gap {
                warnings.push("moon_recorded_cycle_gap_over_45_days");
            }
            let (source_id, cycle_start_us, phase_thresholds_us) = match snapshot.record {
                Some(record) => match record.event {
                    Event::Synod(synod) => (
                        Some(record.source_id),
                        Some(synod.start().as_i64()),
                        synod.phases().map(|extras| {
                            extras.iter().map(|extra| extra.at().as_i64()).collect::<Vec<_>>()
                        }),
                    ),
                    _ => return Err(PyValueError::new_err("invalid source synod family")),
                },
                None => (None, None, None),
            };

            let (phase_code, phase_started_at_us, phase_status) = match snapshot.phase {
                Some(assessment) => (
                    assessment.stage.map(|stage| stage.code()),
                    assessment.started_at.map(TimestampMicros::as_i64),
                    assessment.status.code(),
                ),
                None => (None, None, "no_preceding_recorded_cycle"),
            };

            entries.insert(
                body.as_str().to_owned(),
                json!({
                    "body": body.as_str(),
                    "source_id": source_id,
                    "cycle_start_us": cycle_start_us,
                    "next_cycle_start_us": snapshot.next_recorded_start.map(TimestampMicros::as_i64),
                    "phase_thresholds_us": phase_thresholds_us,
                    "phase": {
                        "code": phase_code,
                        "started_at_us": phase_started_at_us,
                        "status": phase_status
                    },
                    "coverage": {
                        "status": "unreviewed",
                        "warnings": warnings
                    }
                }),
            );
        }

        let new_moon = entries
            .get("Moon")
            .filter(|moon| !moon["cycle_start_us"].is_null())
            .cloned()
            .unwrap_or(Value::Null);

        Ok(json!({
            "requested_at_us": requested_at_us,
            "dataset": {
                "id": Value::Null,
                "sha256": self.sha256,
                "format": "VSEPH-P0",
                "coverage_certified": false
            },
            "synods": Value::Object(entries),
            "new_moon": new_moon
        })
        .to_string())
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeReader>()?;
    Ok(())
}
