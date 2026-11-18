# Independent VSEPH-P0 byte-contract audit

**Status:** test tooling in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5). No merge or production changes. The compressed data and Python prototype are in a **separate draft PR #6**. This audit is **not** the Rust decoder, a verified astronomical coverage model, or the future normative VSEPH v1 file specification.

## Why this check exists

The current Rust P0 reader has not yet passed `cargo test` or `cargo clippy` in this environment (Rust and Cargo are unavailable). To strengthen the evidence now, `tools/audit_p0_bytes.py` independently walks **every byte of every P0 block** with the Python standard library, without calling the Python P0 reader or source PostgreSQL database. It validates the same framing and bounds the Rust adapter proposes to implement.

- Full archive SHA-256 (if supplied), 8-byte P0 magic, version, codec, header size (<=5,000,000 bytes), complete TOC coverage and canonical family/body order.
- Bounded zlib streams, no trailing compressed data, exact declared uncompressed size (<=25,000,000 bytes/block), **each** uncompressed block's SHA-256, and 1–512 records/block.
- Base timestamp as signed i64 POSIX microseconds, unsigned varint IDs/deltas, strict ascending timestamps, overflow rejection, valid motion direction and zodiac signs, optional synod phase deltas and exact consumption of bytes.
- Exact TOC min/max values and section counts; no missing or extraneous payload bytes.
- Independent test harness `tools/test_audit_p0_bytes.py` covering the small (8-event) Python-generated cross-language golden fixture, invalid magic/header, wrong archive hash, truncated/appended payload and compressed-block corruption.
- Test-only maximum input size **256,000,000 bytes** to avoid unbounded allocation in the independent auditor. This is a prototype resource constraint, not a promised stable file-format limit.

This tool checks the **encoded records**, but it does **not** calculate missing astronomical events, certify continuity or compare results with an independently computed astronomical ephemeris.

## Run it on any supported OS

Requires **Python 3.10 or newer**, no packages.

```sh
python -m unittest discover -s tools -p 'test_audit_p0_bytes.py' -v
```

Run after reconstructing the **exact** real archive from the dataset branch (PR #6) in a **combined local checkout**:

```sh
python tools/assemble_dataset.py
python tools/audit_p0_bytes.py datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

Then enable the optional real-data unit test.

macOS/Linux:

```sh
VSEPH_P0_PATH=datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 python -m unittest discover -s tools -p 'test_audit_p0_bytes.py' -v
```

Windows PowerShell:

```powershell
$env:VSEPH_P0_PATH = "datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0"
python -m unittest discover -s tools -p test_audit_p0_bytes.py -v
Remove-Item Env:VSEPH_P0_PATH
```

The current dataset is **4,143,385 bytes**, with **761** compressed blocks and **380,844** records (synod 70,554; motion 266,516; ingress 43,774). All independently scanned blocks, fields and checksums passed in the authoring environment on 2026-11-10.

## Validation evidence

- **PASS:** `python tools/audit_p0_bytes.py [P0 archive] --sha256 [expected digest]` — 761/761 blocks, 380,844/380,844 records.
- **PASS:** `VSEPH_P0_PATH=[P0 archive] python -m unittest discover -s tools -p test_audit_p0_bytes.py -v` — 7/7 tests (including the optional real-data test).
- **PASS:** separate synthetic golden fixture: four blocks and eight fabricated records, SHA-256 `4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499`.
- **NOT RUN:** all Cargo checks. Passing a Python byte-level audit is **not** evidence that the Rust crate compiles or that every Rust query matches the Python P0 reader. Those checks are still blockers for PR #5.

The dataset has gaps; see [the reviewed coverage contract](REVIEWED_COVERAGE_RUST.md). All returns from a raw P0 search are *recorded events*, not scientifically certified states. The current policy contains zero approved completeness intervals.

Attribution: **Powered by VirgoStars** — https://www.virgostars.it/. Source code CPAL-1.0; scientific dataset reuse licensing is tracked separately in [issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9).

## New equivalent Rust full-block verification (draft; uncompiled here)

The experimental Rust P0 reader on the same branch now has `P0Archive::verify_all()` and a one-command CLI. It walks every indexed block, decodes it and checks all per-block hashes and totals without retaining the whole dataset in memory.

```sh
cargo run -p virgostars-events-p0 --bin p0-verify -- datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0
```

A correctly verified 2026-11-10 archive should report **761 blocks / 380,844 records** (ingress 43,774; motion 266,516; synod 70,554), with `coverage_certified:false`. An invalid input should cause a nonzero exit status. The CLI verifies **all P0 compressed blocks**. With `--sha256 <expected>` it **also** validates the whole file against the separately trusted manifest digest; without that option it merely reports the computed SHA-256 and does not authenticate the snapshot. The Rust command has **NOT BEEN RUN** here: Rust/Cargo toolchain is unavailable. The corresponding independent Python audit is tested and passed **8/8**, including corruption of a block not accessed by a targeted Mercury query.

## Optional trusted whole-file digest (Rust CLI)

On the Rust draft branch, after Cargo is installed and the dataset is assembled, run:

```sh
cargo run -p virgostars-events-p0 --bin p0-verify -- datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

The command prints a lowercase `sha256`, `sha256_checked_against_expected: true`, counts and `coverage_certified: false`, then exits successfully only if **both** all blocks and the full-file digest match. It never declares unreviewed astronomical periods complete. The parser now uses checked arithmetic for the total record count. The Rust CLI and new tests **have not been compiled or executed** in this environment; use the already executed Python verifier for current confidence.

## Source-row identity integrity (additional 2026-11-10 hardening)

The independent Python auditor now rejects **zero and reused PostgreSQL source row IDs**. A source ID is unique within its original table/event family across **all compressed blocks and all celestial bodies**; numerical IDs may coincide between different families because the source tables have separate identity sequences.

The experimental Rust P0 decoder applies the same rule in two places:
- `decode_records` rejects ID zero or repeated IDs *within a block*, so targeted lookups fail closed on malformed indexed data.
- `P0Archive::verify_all()` additionally checks uniqueness *across all blocks and bodies within each family*. A targeted lookup cannot by itself verify a different, unqueried block.

To prove the semantic check rather than merely checksum rejection, the regression tests generate synthetic archives with **valid compressed data and updated SHA-256 digests**, but intentionally duplicate an original ID. They cover zero IDs, duplicates inside one block, duplicates across separate blocks and across bodies, and legal reuse between independent families.

```bash
python -m unittest discover -s tools -p test_audit_p0_bytes.py -v
python tools/audit_p0_bytes.py <archive.vseph-p0> --sha256 <trusted-manifest-digest>
```

**Actually executed:** **13/13 Python tests PASS**, including the real 2026-11-10 binary with **380,844 events**, **761 blocks**, no duplicate/zero IDs, and matching full-file SHA-256 `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`. The executed Python scripts and golden fixture match their committed GitHub blob SHA-1 values exactly.

**Not yet executed:** the four new Rust golden tests for these identity rules (zero ID, duplicate in block, duplicate in separate blocks, duplicate across bodies) and the existing Rust workspace tests — no `rustc`/`cargo` on the authoring environment, and the Rust distribution host could not be resolved. These tests must be compiled and run before merge.

This validates **dataset identity and serialization**, not the completeness or scientific correctness of historical astronomical events.
