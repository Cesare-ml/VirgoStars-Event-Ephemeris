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

use crate::{LatitudeProvider, LatitudeSample, LongitudeProvider, LongitudeSample};

const MAX_LINE_BYTES: u64 = 8_192;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

fn spawn_session(executable: &Path) -> Result<Session, String> {
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("cannot start provider: {error}"))?;
    let input = child.stdin.take().ok_or("provider stdin unavailable")?;
    let output = child.stdout.take().ok_or("provider stdout unavailable")?;
    Ok(Session {
        child,
        input: BufWriter::new(input),
        output: BufReader::new(output),
    })
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

fn request_position(
    session: &mut Session,
    body: &BodyId,
    at: TimestampMicros,
) -> Result<String, String> {
    // Decimal strings preserve the exact i64 microsecond timestamp for
    // JavaScript and other adapters beyond 2^53 integer precision.
    let at_text = at.as_i64().to_string();
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
    read_bounded_line(&mut session.output)
}

impl ProcessLongitudeProvider {
    pub fn spawn(executable: &Path) -> Result<Self, String> {
        let mut session = spawn_session(executable)?;
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
        let at_text = at.as_i64().to_string();
        let mut session = self
            .session
            .lock()
            .map_err(|_| "provider session lock poisoned".to_string())?;
        let response = request_position(&mut session, body, at)?;
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

/// Separate, versioned protocol for geocentric ecliptic-latitude samples.
/// The identity is **self-reported** and must be independently checked by the
/// operator. Two distinct identities do not prove independent ephemerides.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LatitudeProviderIdentity {
    pub protocol: String,
    pub provider: String,
    pub version: String,
    pub frame: String,
    pub ecliptic_plane: String,
    pub position_corrections: String,
    pub dynamical_time_scale: String,
    pub input_time_scale: String,
    pub speed_definition: String,
    pub flags: String,
    pub source_license: String,
    pub kernel_digest: Option<String>,
    pub source_digest: Option<String>,
}

impl LatitudeProviderIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.protocol != "virgostars.latitude.v1"
            || self.frame != "geocentric-ecliptic-latitude-degrees"
            || self.input_time_scale != "utc-posix-microseconds"
            || self.speed_definition != "signed-d-beta-dt-degrees-per-day"
            || [
                &self.provider,
                &self.version,
                &self.ecliptic_plane,
                &self.position_corrections,
                &self.dynamical_time_scale,
                &self.flags,
                &self.source_license,
            ]
            .iter()
            .any(|field| field.trim().is_empty())
        {
            return Err("incomplete or incompatible latitude provider identity/settings".into());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LatitudePositionResponse {
    body: String,
    utc_microseconds: String,
    latitude_degrees: f64,
    latitude_speed_degrees_per_day: f64,
}

/// Optional, development-only latitude source launched from an explicit path.
/// It does not bundle or load a planetary kernel or alter the offline reader.
pub struct ProcessLatitudeProvider {
    identity: LatitudeProviderIdentity,
    session: Mutex<Session>,
}

impl ProcessLatitudeProvider {
    pub fn spawn(executable: &Path) -> Result<Self, String> {
        let mut session = spawn_session(executable)?;
        let hello = read_bounded_line(&mut session.output)?;
        let identity: LatitudeProviderIdentity = serde_json::from_str(&hello)
            .map_err(|error| format!("invalid latitude provider identity: {error}"))?;
        identity.validate()?;
        Ok(Self {
            identity,
            session: Mutex::new(session),
        })
    }

    pub fn identity(&self) -> &LatitudeProviderIdentity {
        &self.identity
    }
}

impl LatitudeProvider for ProcessLatitudeProvider {
    fn sample_latitude(
        &self,
        body: &BodyId,
        at: TimestampMicros,
    ) -> Result<LatitudeSample, String> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "latitude provider session lock poisoned".to_string())?;
        let response = request_position(&mut session, body, at)?;
        let parsed: LatitudePositionResponse = serde_json::from_str(&response)
            .map_err(|error| format!("invalid latitude provider sample: {error}"))?;
        if parsed.body != body.as_str() || parsed.utc_microseconds != at.as_i64().to_string() {
            return Err("latitude provider returned sample for another body or instant".into());
        }
        if !parsed.latitude_degrees.is_finite()
            || parsed.latitude_degrees.abs() > 90.0
            || !parsed.latitude_speed_degrees_per_day.is_finite()
        {
            return Err("latitude provider returned invalid coordinate or speed".into());
        }
        Ok(LatitudeSample {
            latitude_degrees: parsed.latitude_degrees,
            latitude_speed_degrees_per_day: parsed.latitude_speed_degrees_per_day,
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
