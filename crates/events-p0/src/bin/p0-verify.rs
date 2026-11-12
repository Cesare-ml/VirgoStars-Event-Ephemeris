/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0); see LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! Offline exhaustive verifier for experimental VSEPH-P0 (NOT VSEPH v1).

use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::process;

use virgostars_events_core::EventFamily;
use virgostars_events_p0::P0Archive;

fn usage() -> IoError {
    IoError::new(
        ErrorKind::InvalidInput,
        "usage: p0-verify <archive.vseph-p0> [--sha256 <64-char-lowercase-digest>]",
    )
}

fn verify_archive(
    path: &std::path::Path,
    expected: Option<&str>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    // Opening reads only the bounded header/index. When a trusted SHA was
    // supplied, verify the complete input BEFORE decompressing arbitrary
    // blocks: wrong snapshots must fail as identity errors, not codec errors.
    let archive = P0Archive::open(path)?;
    let sha256 = match expected {
        Some(expected) => archive.verify_archive_sha256(expected)?,
        None => archive.archive_sha256()?,
    };
    let result = archive.verify_all()?;
    let count = |family| result.by_family.get(&family).copied().unwrap_or(0);
    let bytes = std::fs::metadata(path)?.len();
    Ok(serde_json::json!({
        "format": "VSEPH-P0-EXPERIMENTAL",
        "sha256": sha256,
        "bytes": bytes,
        "sha256_checked_against_expected": expected.is_some(),
        "blocks": result.blocks,
        "records": result.records,
        "sections": {
            "synod": count(EventFamily::Synod),
            "motion": count(EventFamily::Motion),
            "ingress": count(EventFamily::Ingress)
        },
        "coverage_certified": false
    }))
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os();
    let _program_name = args.next();
    let path = args.next().ok_or_else(usage)?;

    let expected = match args.next() {
        None => None,
        Some(flag) if flag.to_str() == Some("--sha256") => Some(args.next().ok_or_else(usage)?),
        Some(_) => return Err(usage().into()),
    };
    if args.next().is_some() {
        return Err(usage().into());
    }

    let path = std::path::Path::new(&path);
    let digest = expected.as_ref().map(|value| value.to_str().ok_or_else(usage)).transpose()?;
    let report = verify_archive(path, digest)?;
    println!("{report}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("VSEPH-P0 verification failed: {error}");
        process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::verify_archive;
    use base64::Engine as _;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE_SHA: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";

    struct TemporaryArchive(PathBuf);

    impl Drop for TemporaryArchive {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn golden_bytes() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../tests/fixtures/synthetic.p0.base64").trim())
            .expect("independently authored synthetic P0 fixture")
    }

    fn write_archive(bytes: &[u8]) -> TemporaryArchive {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("valid system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "virgostars-p0-verify-{}-{nonce}.p0",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write temporary synthetic archive");
        TemporaryArchive(path)
    }

    #[test]
    fn trusted_golden_archive_reports_all_verified_fields() {
        let input = write_archive(&golden_bytes());
        let report = verify_archive(&input.0, Some(FIXTURE_SHA))
            .expect("the original synthetic P0 archive must pass");
        assert_eq!(report["sha256"], FIXTURE_SHA);
        assert_eq!(report["sha256_checked_against_expected"], true);
        assert_eq!(report["format"], "VSEPH-P0-EXPERIMENTAL");
        assert_eq!(report["bytes"], 1470);
        assert_eq!(report["blocks"], 4);
        assert_eq!(report["records"], 8);
        assert_eq!(report["sections"]["synod"], 3);
        assert_eq!(report["sections"]["motion"], 2);
        assert_eq!(report["sections"]["ingress"], 3);
        assert_eq!(report["coverage_certified"], false);
    }

    #[test]
    fn wrong_trusted_hash_rejected_before_decoding_corrupt_block() {
        let mut corrupted = golden_bytes();
        *corrupted.last_mut().expect("nonempty fixture") ^= 0x7f;
        let input = write_archive(&corrupted);
        // Both the file identity and compressed block are now wrong.
        // The expected SHA must be diagnosed FIRST, before zlib decoding.
        let error = verify_archive(&input.0, Some(FIXTURE_SHA))
            .expect_err("wrong trusted archive must fail");
        assert!(
            error.to_string().contains("whole-file archive SHA-256 mismatch"),
            "identity verification must precede block decode: {error}"
        );
    }

    #[test]
    fn full_block_audit_still_required_without_trusted_sha() {
        let input = write_archive(&golden_bytes());
        let report = verify_archive(&input.0, None).expect("valid untrusted fixture still audited");
        assert_eq!(report["sha256_checked_against_expected"], false);
        let mut corrupted = golden_bytes();
        *corrupted.last_mut().unwrap() ^= 0x7f;
        let altered = write_archive(&corrupted);
        assert!(verify_archive(&altered.0, None).is_err());
    }
}
