/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (the "License"); you may not use this file except in
 * compliance with the License. You may obtain a copy at
 * https://opensource.org/license/CPAL-1.0.
 *
 * The Original Code is VirgoStars-Event-Ephemeris source code.
 * The Original Developer and Initial Developer is Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * Distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY KIND.
 * See repository LICENSE (Exhibits A and B) and NOTICE.
 *
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! Minimal VSEPH candidate magic probe.
//!
//! This crate deliberately does **not** parse a VSEPH dataset yet. The wire
//! layout, version encoding, checksums and indexes remain draft in
//! `docs/VSEPH_FORMAT.md` and must be frozen in issue #3 before implementation.

use std::error::Error;
use std::fmt;

/// Five-byte file identifier proposed for future VSEPH binary files.
pub const VSEPH_MAGIC: [u8; 5] = *b"VSEPH";

/// Errors from probing only the magic bytes, not a complete file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormatProbeError {
    /// Fewer than five bytes are available.
    TruncatedMagic { actual: usize },
    /// Five or more bytes exist but the identifier does not match.
    InvalidMagic,
}

impl fmt::Display for FormatProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedMagic { actual } => {
                write!(f, "truncated VSEPH magic: {actual} bytes (requires 5)")
            }
            Self::InvalidMagic => write!(f, "invalid VSEPH magic"),
        }
    }
}

impl Error for FormatProbeError {}

/// Probe the five candidate identification bytes, without claiming the file is valid.
///
/// Extra bytes are intentionally permitted; complete layout checks will be
/// added only after the first byte-level version is specified and tested.
pub fn probe_magic(input: &[u8]) -> Result<(), FormatProbeError> {
    if input.len() < VSEPH_MAGIC.len() {
        return Err(FormatProbeError::TruncatedMagic {
            actual: input.len(),
        });
    }
    if !input.starts_with(&VSEPH_MAGIC) {
        return Err(FormatProbeError::InvalidMagic);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{probe_magic, FormatProbeError, VSEPH_MAGIC};

    #[test]
    fn accepts_only_the_magic_prefix_not_the_whole_format() {
        assert!(probe_magic(&VSEPH_MAGIC).is_ok());
        assert!(probe_magic(b"VSEPHnot-a-valid-file-yet").is_ok());
    }

    #[test]
    fn rejects_all_truncated_prefixes() {
        for n in 0..VSEPH_MAGIC.len() {
            assert_eq!(
                probe_magic(&VSEPH_MAGIC[..n]),
                Err(FormatProbeError::TruncatedMagic { actual: n })
            );
        }
    }

    #[test]
    fn rejects_wrong_magic() {
        assert_eq!(probe_magic(b"OTHER"), Err(FormatProbeError::InvalidMagic));
        assert_eq!(probe_magic(b"VSEPh"), Err(FormatProbeError::InvalidMagic));
    }
}
