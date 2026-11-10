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

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os();
    let _program_name = args.next();
    let path = args.next().ok_or_else(usage)?;

    let expected = match args.next() {
        None => None,
        Some(flag) if flag.to_str() == Some("--sha256") => {
            Some(args.next().ok_or_else(usage)?)
        }
        Some(_) => return Err(usage().into()),
    };
    if args.next().is_some() {
        return Err(usage().into());
    }

    let archive = P0Archive::open(std::path::Path::new(&path))?;
    let result = archive.verify_all()?;
    let sha256 = match expected.as_ref() {
        Some(expected) => archive.verify_archive_sha256(
            expected.to_str().ok_or_else(usage)?
        )?,
        None => archive.archive_sha256()?,
    };
    let count = |family| result.by_family.get(&family).copied().unwrap_or(0);
    println!(
        "{}",
        serde_json::json!({
            "format": "VSEPH-P0-EXPERIMENTAL",
            "sha256": sha256,
            "sha256_checked_against_expected": expected.is_some(),
            "blocks": result.blocks,
            "records": result.records,
            "sections": {
                "synod": count(EventFamily::Synod),
                "motion": count(EventFamily::Motion),
                "ingress": count(EventFamily::Ingress)
            },
            "coverage_certified": false
        })
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("VSEPH-P0 verification failed: {error}");
        process::exit(2);
    }
}
