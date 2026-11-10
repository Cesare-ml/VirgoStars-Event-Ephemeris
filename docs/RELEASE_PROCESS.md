# Release process — not yet active

No packages or datasets are currently released. Publishing a source crate, binding or dataset is a **separate explicit maintainer decision**; no GitHub Actions auto release or deployment.

## Independent version streams

- `core` Rust crate version (SemVer).
- `format` VSEPH byte format major/minor, with golden reference file and parser compatibility table.
- `dataset` date/version and unique digest, with per-body/time coverage manifest.
- `bindings` npm and Python package versions, each stating which Rust core and file formats they accept.
- `adapter` optional VirgoStars integration release, never a forced production deployment.

## Code release gates

1. Applicable `cargo fmt/clippy/test`, JS/TS tests and Python build/wheel parity tests pass, with logs and reproducible commands.
2. Security review for arbitrary-file decoding, overflow bounds, error handling, memory limits, hostile index handling.
3. Public API, format spec and golden fixtures consistent with shipped implementation.
4. License/NOTICE and per-file notices preserved; required attribution guidance included.
5. Human maintainer approves version and public release; no release inferred from a PR merge.

## Dataset release gates

1. Explicit statement and confirmation of data author/release rights and approved **separate** dataset license.
2. Read-only authenticated snapshot audit including counts, min/max coverage, complete phase schema and documented missing segments.
3. Deterministic export, full source-vs-decoder equivalence, reproducible checksums and manifest.
4. Binary size, random-query speed, cross-language compatibility and integrity tests pass.
5. Owner gives explicit permission to publish the exact snapshot/version.
6. Publish as a separate versioned asset with checksum, manifest, data license and credits. Never commit a private DB dump.

## Changes that require a new format version

Changed timestamp unit or epoch, body-ID registry semantics, mandatory phase code semantics, record ordering, meaning of an index offset, compression features, or wire representation without a backward-compatible parser all require a major-format decision and new fixtures.
