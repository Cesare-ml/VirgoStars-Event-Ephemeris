/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Optional out-of-process adapter for a separately licensed ephemeris source.
//! It is a development-only JSON-lines protocol, not an in-process Swiss link.

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use virgostars_events_core::{BodyId, TimestampMicros};

use crate::{LongitudeProvider, LongitudeSample};

const MAX_LINE_BYTES: u64 = 8_192;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentity {
    pub protocol: String,
    pub provider: String,
    pub version: String,
    pub frame: String,
    pub input_time_scale: String,
    pub flags: String,
    pub kernel_digest: Option<String>,
    pub source_digest: Option<String>,
}

impl ProviderIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.protocol != "virgostars.longitude.v2"
            || self.frame != "geocentric-tropical-ecliptic-degrees"
            || self.input_time_scale != "utc-posix-microseconds"
            || self.provider.trim().is_empty()
            || self.version.trim().is_empty()
            || self.flags.trim().is_empty()
        {
            return Err("incomplete or incompatible provider identity/settings".into());
        }
        Ok(())
    }
}

struct Session {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // Process lifetime is owned by the staging invocation, not a Tauri app.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// JSON-lines process adapter: initial identity line, then request/response pairs.
pub struct ProcessLongitudeProvider {
    identity: ProviderIdentity,
    session: Mutex<Session>,
}

#[derive(Serialize)]
struct PositionRequest<'a> {
    body: &'a str,
    utc_microseconds: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PositionResponse {
    body: String,
    utc_microseconds: String,
    longitude_degrees: f64,
    longitude_speed_degrees_per_day: f64,
}

fn read_bounded_line(reader: &mut BufReader<ChildStdout>) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut limited = reader.take(MAX_LINE_BYTES + 1);
    limited
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("provider read failed: {error}"))?;
    if bytes.len() as u64 > MAX_LINE_BYTES || !bytes.ends_with(b"\n") {
        return Err("provider line exceeds limit or terminated unexpectedly".into());
    }
    String::from_utf8(bytes).map_err(|_| "provider output is not UTF-8".into())
}

impl ProcessLongitudeProvider {
    pub fn spawn(executable: &Path) -> Result<Self, String> {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("cannot start provider: {error}"))?;
        let input = child.stdin.take().ok_or("provider stdin unavailable")?;
        let output = child.stdout.take().ok_or("provider stdout unavailable")?;
        let mut session = Session {
            child,
            input: BufWriter::new(input),
            output: BufReader::new(output),
        };
        let hello = read_bounded_line(&mut session.output)?;
        let identity: ProviderIdentity = serde_json::from_str(&hello)
            .map_err(|error| format!("invalid provider identity: {error}"))?;
        identity.validate()?;
        Ok(Self {
            identity,
            session: Mutex::new(session),
        })
    }

    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }
}

impl LongitudeProvider for ProcessLongitudeProvider {
    fn sample(&self, body: &BodyId, at: TimestampMicros) -> Result<LongitudeSample, String> {
        // Decimal strings preserve the exact i64 microsecond input for Node
        // adapters even outside JavaScript's 2^53 integer-safe range.
        let at_text = at.as_i64().to_string();
        let mut session = self
            .session
            .lock()
            .map_err(|_| "provider session lock poisoned".to_string())?;
        serde_json::to_writer(
            &mut session.input,
            &PositionRequest {
                body: body.as_str(),
                utc_microseconds: &at_text,
            },
        )
        .map_err(|error| format!("provider request encoding failed: {error}"))?;
        session
            .input
            .write_all(b"\n")
            .and_then(|_| session.input.flush())
            .map_err(|error| format!("provider request write failed: {error}"))?;
        let response = read_bounded_line(&mut session.output)?;
        let parsed: PositionResponse = serde_json::from_str(&response)
            .map_err(|error| format!("invalid provider position: {error}"))?;
        if parsed.body != body.as_str() || parsed.utc_microseconds != at_text {
            return Err("provider returned position for a different body or instant".into());
        }
        Ok(LongitudeSample {
            longitude_degrees: parsed.longitude_degrees,
            longitude_speed_degrees_per_day: parsed.longitude_speed_degrees_per_day,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mismatched_coordinate_system() {
        let identity = ProviderIdentity {
            protocol: "virgostars.longitude.v2".into(),
            provider: "test".into(),
            version: "1".into(),
            frame: "sidereal".into(),
            input_time_scale: "utc-posix-microseconds".into(),
            flags: "test".into(),
            kernel_digest: None,
            source_digest: None,
        };
        assert!(identity.validate().is_err());
    }
}
