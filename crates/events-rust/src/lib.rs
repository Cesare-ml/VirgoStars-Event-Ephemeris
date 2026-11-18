/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Rust-only, offline-first consumer API for desktop and mobile clients.
//!
//! This is an experimental **recorded-event** reader for VSEPH-P0. It does
//! not calculate astronomy, download a dataset or claim complete coverage.
//! The caller must supply a locally stored archive and its separately trusted
//! whole-file SHA-256. The Rust core is shared by both application targets.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub use virgostars_events_core::{
    BodyId, Event, EventFamily, MotionDirection, SynodStageAssessment, TimestampMicros, ZodiacSign,
};
use virgostars_events_p0::P0Archive;
pub use virgostars_events_p0::{P0AuditSummary, P0Error, P0Record, P0SynodSnapshot};

/// Validated, read-only experimental event archive for a desktop/mobile app.
///
/// This struct owns its reader. Query methods require only `&self`, so UI
/// integration can share a read-only instance under an application lock.
pub struct VirgoStarsEventEphemeris {
    archive: P0Archive,
    sha256: String,
    audit: P0AuditSummary,
    // Keep the private, digest-authenticated file alive for all subsequent
    // queries and delete it when the final Rust owner is dropped.
    _snapshot_dir: tempfile::TempDir,
}

impl VirgoStarsEventEphemeris {
    /// Open a private snapshot and independently verify its SHA-256 and *all* blocks.
    ///
    /// The expected digest must come from a trusted manifest, never from the
    /// same untrusted file. The original file can be replaced or deleted
    /// after opening: all subsequent queries use the authenticated copy.
    /// Uses the OS default temporary directory; on Android, prefer
    /// `open_verified_in` with a writable app-private cache directory.
    pub fn open_verified(path: impl AsRef<Path>, expected_sha256: &str) -> Result<Self, P0Error> {
        Self::open_snapshot(path.as_ref(), expected_sha256, None)
    }

    /// As `open_verified`, but place the authenticated copy in `cache_root`.
    ///
    /// The caller must choose an existing writable and private directory.
    /// It must not be an untrusted shared folder: the archive reader uses its
    /// stable private path without rehashing all bytes on every query.
    pub fn open_verified_in(
        path: impl AsRef<Path>,
        expected_sha256: &str,
        cache_root: impl AsRef<Path>,
    ) -> Result<Self, P0Error> {
        Self::open_snapshot(path.as_ref(), expected_sha256, Some(cache_root.as_ref()))
    }

    fn open_snapshot(
        source: &Path,
        expected_sha256: &str,
        cache_root: Option<&Path>,
    ) -> Result<Self, P0Error> {
        if expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(P0Error::Invalid(
                "expected archive SHA-256 must be 64 lower-case hex characters".into(),
            ));
        }
        // Reject malformed framing before allocating cache space. A mutated
        // source between this probe and the copy cannot pass the copied-byte
        // SHA gate unless it reproduces the separately trusted exact bytes.
        P0Archive::open(source)?;
        let snapshot_dir = match cache_root {
            Some(root) => tempfile::tempdir_in(root)?,
            None => tempfile::tempdir()?,
        };
        let snapshot = snapshot_dir.path().join("verified-archive.vseph-p0");
        fs::copy(source, &snapshot)?;

        let archive = P0Archive::open(&snapshot)?;
        let sha256 = archive.verify_archive_sha256(expected_sha256)?;
        let audit = archive.verify_all()?;
        Ok(Self {
            archive,
            sha256,
            audit,
            _snapshot_dir: snapshot_dir,
        })
    }

    /// The trusted whole-file digest verified at open time.
    pub fn verified_sha256(&self) -> &str {
        &self.sha256
    }

    /// A byte/record integrity audit, NOT an astronomical coverage certificate.
    pub fn audit(&self) -> &P0AuditSummary {
        &self.audit
    }

    /// Return the last recorded synod per body and any source-derived phase.
    pub fn synods_at(
        &self,
        utc_microseconds: i64,
    ) -> Result<BTreeMap<BodyId, P0SynodSnapshot>, P0Error> {
        self.archive
            .synods_at(TimestampMicros::new(utc_microseconds))
    }

    /// Latest recorded event at/before `utc_microseconds` (inclusive).
    pub fn at(
        &self,
        family: EventFamily,
        body: &str,
        utc_microseconds: i64,
    ) -> Result<Option<P0Record>, P0Error> {
        let body = validated_body(body)?;
        self.archive
            .at(family, &body, TimestampMicros::new(utc_microseconds))
    }

    /// First event strictly after the given UTC/POSIX microsecond instant.
    pub fn next(
        &self,
        family: EventFamily,
        body: &str,
        utc_microseconds: i64,
    ) -> Result<Option<P0Record>, P0Error> {
        let body = validated_body(body)?;
        self.archive
            .next(family, &body, TimestampMicros::new(utc_microseconds))
    }

    /// Last event strictly before the given UTC/POSIX microsecond instant.
    pub fn previous(
        &self,
        family: EventFamily,
        body: &str,
        utc_microseconds: i64,
    ) -> Result<Option<P0Record>, P0Error> {
        let body = validated_body(body)?;
        self.archive
            .previous(family, &body, TimestampMicros::new(utc_microseconds))
    }

    /// Recorded events in `[start_us, end_us)`; an empty result is not
    /// evidence that no astronomical events occurred in that interval.
    pub fn between(
        &self,
        family: EventFamily,
        body: &str,
        start_us: i64,
        end_us: i64,
    ) -> Result<Vec<P0Record>, P0Error> {
        let body = validated_body(body)?;
        self.archive.between(
            family,
            &body,
            TimestampMicros::new(start_us),
            TimestampMicros::new(end_us),
        )
    }

    /// Escape hatch for advanced native operations, including coverage-aware
    /// state queries and bounded chronological pagination.
    pub fn raw_archive(&self) -> &P0Archive {
        &self.archive
    }
}

fn validated_body(name: &str) -> Result<BodyId, P0Error> {
    BodyId::try_from(name).map_err(|_| P0Error::Invalid("invalid celestial body name".into()))
}
