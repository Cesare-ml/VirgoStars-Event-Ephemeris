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
