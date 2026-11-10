/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0); see LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Distributed AS IS, without warranty.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! SHA-256-bound, fail-closed state queries for the EXPERIMENTAL VSEPH-P0 file.
//! Compatible in intent with prototype/coverage_policy.py in dataset PR #6.
//! Source records alone never certify missing intervals are complete.

use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};
use virgostars_events_core::{
    BodyId, CoverageAssessment, CoveragePolicy, EventFamily, ReviewedWindow, TimestampMicros,
};

use crate::{invalid, P0Archive, P0Error, P0Record};

const SCHEMA: &str = "virgostars-reviewed-coverage/v0";
const MAX_POLICY_BYTES: u64 = 1_000_000;

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, P0Error> {
    value.get(key).and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("missing or non-string coverage policy field {key}")))
}

fn instant(value: &Value, key: &str) -> Result<TimestampMicros, P0Error> {
    let text = required(value, key)?;
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0')) || text.starts_with('+')
    {
        return Err(invalid(format!("invalid policy instant {key}")));
    }
    text.parse::<i64>().map(TimestampMicros::new)
        .map_err(|_| invalid(format!("policy instant {key} out of i64 range")))
}

fn file_hash(path: &Path) -> Result<String, P0Error> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        hash.update(&chunk[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn parse_window(row: &Value) -> Result<ReviewedWindow, P0Error> {
    let family = match required(row, "family")? {
        "ingress" => EventFamily::Ingress,
        "motion" => EventFamily::Motion,
        _ => return Err(invalid("unsupported coverage family")),
    };
    let body = BodyId::try_from(required(row, "body")?)
        .map_err(|error| invalid(error.to_string()))?;
    let reviewer = required(row, "reviewer")?;
    let evidence = required(row, "evidence")?;
    ReviewedWindow::new(
        family,
        body,
        instant(row, "start_us")?,
        instant(row, "end_us_exclusive")?,
        reviewer.to_owned(),
        evidence.to_owned(),
    ).map_err(|error| invalid(error.to_string()))
}

/// A state query outcome. The last recorded event is useful diagnostic
/// evidence even when coverage is not approved, but cannot certify a sign.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoveredState {
    UnknownCoverage { last_recorded: Option<P0Record> },
    UnknownInitialState { last_recorded: Option<P0Record> },
    Reviewed {
        record: P0Record,
        reviewer: String,
        evidence: String,
        window_start: TimestampMicros,
        window_end_exclusive: TimestampMicros,
    },
}

/// Data-bound coverage wrapper; the policy is verified against the entire
/// archive SHA-256 before use, unlike a raw event lookup.
///
/// This is a scientific review *record*, not mathematical proof that the
/// named reviewer independently confirmed completeness.
pub struct CoverageAwareP0<'a> {
    archive: &'a P0Archive,
    policy: CoveragePolicy,
    sha256: String,
}

impl P0Archive {
    /// Attach a v0 review policy to the exact file, rejecting other dataset
    /// versions, malformed windows and unreviewed policy metadata.
    pub fn with_coverage_policy(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<CoverageAwareP0<'_>, P0Error> {
        let policy_path = path.as_ref();
        if fs::metadata(policy_path)?.len() > MAX_POLICY_BYTES {
            return Err(invalid("coverage policy exceeds maximum size"));
        }
        let data = fs::read(policy_path)?;
        let document: Value = serde_json::from_slice(&data)
            .map_err(|error| invalid(format!("malformed coverage policy: {error}")))?;
        if required(&document, "schema")? != SCHEMA
            || required(&document, "verification")? != "human-reviewed-only"
        {
            return Err(invalid("unsupported coverage policy schema or verification mode"));
        }
        let sha256 = required(&document, "archive_sha256")?;
        if sha256.len() != 64
            || !sha256.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid("invalid lowercase archive SHA-256"));
        }
        let actual = file_hash(&self.path)?;
        if sha256 != actual {
            return Err(invalid("coverage policy archive SHA-256 mismatch"));
        }
        let rows = document.get("reviewed_windows").and_then(Value::as_array)
            .ok_or_else(|| invalid("missing coverage reviewed_windows array"))?;
        // Bound the number of entries even though file size is capped.
        if rows.len() > 4096 {
            return Err(invalid("too many coverage windows"));
        }
        let windows = rows.iter().map(parse_window)
            .collect::<Result<Vec<_>, _>>()?;
        let policy = CoveragePolicy::new(windows)
            .map_err(|error| invalid(error.to_string()))?;
        Ok(CoverageAwareP0 {
            archive: self,
            policy,
            sha256: actual,
        })
    }
}

impl CoverageAwareP0<'_> {
    /// The SHA-256 of the dataset verified when attaching this policy.
    pub fn archive_sha256(&self) -> &str {
        &self.sha256
    }

    pub fn reviewed_window_count(&self) -> usize {
        self.policy.windows().len()
    }

    fn state(
        &self,
        family: EventFamily,
        body: &BodyId,
        instant: TimestampMicros,
    ) -> Result<CoveredState, P0Error> {
        // Do not let a swapped/mutated file inherit the verification made at
        // construction. This costs a streamed hash for each state query in P0.
        if file_hash(&self.archive.path)? != self.sha256 {
            return Err(invalid("archive bytes changed since coverage policy verification"));
        }
        let last = self.archive.at(family, body, instant)?;
        match self.policy.assess(family, body, instant, last.as_ref().map(P0Record::at)) {
            CoverageAssessment::UnknownCoverage =>
                Ok(CoveredState::UnknownCoverage { last_recorded: last }),
            CoverageAssessment::UnknownInitialState =>
                Ok(CoveredState::UnknownInitialState { last_recorded: last }),
            CoverageAssessment::Reviewed(window) =>
                Ok(CoveredState::Reviewed {
                    record: last.expect("reviewed window requires initial recorded event"),
                    reviewer: window.reviewer().to_owned(),
                    evidence: window.evidence().to_owned(),
                    window_start: window.start(),
                    window_end_exclusive: window.end_exclusive(),
                }),
        }
    }

    /// Reviewed sign state from the ingress family only.
    pub fn sign_at(&self, body: &BodyId, instant: TimestampMicros) -> Result<CoveredState, P0Error> {
        self.state(EventFamily::Ingress, body, instant)
    }

    /// Reviewed direct/retrograde state from the motion family only.
    pub fn direction_at(&self, body: &BodyId, instant: TimestampMicros) -> Result<CoveredState, P0Error> {
        self.state(EventFamily::Motion, body, instant)
    }
}
