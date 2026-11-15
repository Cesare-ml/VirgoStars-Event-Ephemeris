# Validation and quality gates

**No implementation/tests have been run for this repository as of initial setup.** The following gates become executable as code is added; record actual commands and outcomes in PRs and [WORK_LOG](WORK_LOG.md).

## Gate A: documentation/specification

- README, AGENTS, license, NOTICE, ADR, API and format docs are consistent and discoverable.
- New methods map to concrete backend queries in [BACKEND_COMPATIBILITY.md](BACKEND_COMPATIBILITY.md).
- Clearly mark proposed vs implemented behavior and separate code/dataset license.
- Record changed source paths and any uncertain assumptions; validate link targets.

## Gate B: Rust format parser and core

Once Cargo workspace exists, standard commands (exact feature flags may change):
~~~sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
~~~

Require unit tests for:
- header/endian/version/flags; every index offset/length/count and overflow; truncated/corrupted files; invalid checksums; excessive allocations.
- positive/negative Unix microsecond times, offsets, same-instant ordering, first/last records, empty body and missing coverage.
- absent, 0, 3 or variable `dataora_extra`, strict phase ordering and phase labels by family.
- strict next/previous and inclusive current at exactly event timestamp; [start,end) ranges and deterministic tie order.
- motion direct/retrograde transitions, repeated ingresses and year/DST boundary searches.

## Gate C: exporter + dataset integrity

A production snapshot export is **not authorized in this repository by default**. When access and publication are explicitly approved:
1. Execute read-only, transactionally consistent streaming export using least privilege.
2. Compare source table/body count, min/max, duplicate keys, type constraints and every event field to decoded output.
3. Validate all `dataora_extra`, null semantics and per-body gap indicators.
4. Report discrepancies **without silently repairing or dropping rows**.
5. Publish checksum and versioned manifest only with dataset-owner approval and an explicit dataset license.

As of 2026-11-10, the count baseline is 70,554 + 266,516 + 43,774 = 380,844 reported rows; this is an **expectation to audit**, not proven ground truth.

## Gate D: cross-language parity

For a shared synthetic golden fixture, compare exact serialized results and error categories from Rust, JS/TS WASM and Python PyO3. Include UTC and Europe/Rome on both DST transitions, out-of-coverage dates, month containing multiple new moons, negative timestamps, missing phases and invalid input. Test browser and Node loading separately, and Python wheels on supported target platforms.

## Gate E: backend behavior parity

Use the [compatibility matrix](BACKEND_COMPATIBILITY.md), comparing at least:
- latest synod by body/all bodies and handle `next()`;
- Moon quarter/full/new events in month and neighbors;
- sinodic D1/R2/R3/D4 + Moon phases where present;
- motion on date, state at instant, state at *end of local day*;
- sign ingresses on a local date, Moon's month ingresses;
- unified events calendar sorted and filtered.
Record deliberate legacy bug fixes separately rather than forcing the new engine to reproduce them.

## Gate F: performance and release

Measure dataset bytes, memory use, random lookup p50/p95, cold open, repeated queries, interval iteration, Rust crate size, WASM gzip bytes and Python wheel sizes on specified hardware/runtime. No claimed throughput without executed benchmarks.

**No releases, deployments, backend replacement, production DB write, automatic GitHub Actions or merge to main without maintainer's explicit decision and completed applicable gates.**

## Supplemental local-day golden oracle — non-hour IANA offsets (2026-11-15)

Issue [#23](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/23) adds offline, **fixed** reference intervals for proposed `onDate(localDate, IANAZone)` behavior. The source files are `test/fixtures/non-hour-zone-local-days.json` and `test/non-hour-zone-local-days.test.mjs`. The test command is `node --test test/non-hour-zone-local-days.test.mjs`; optional syntax preflight is `node --check test/non-hour-zone-local-days.test.mjs`.

- `Asia/Kathmandu` (UTC+05:45) and `Pacific/Chatham` (UTC+12:45 or +13:45) detect mistaken whole-hour conversion. The two Chatham fixtures cover summer/winter offset differences.
- `Australia/Lord_Howe` changes its clock by **30 minutes**: the fixed 2026-04-05 local day lasts **88,200 s (24.5 h)** and 2026-10-04 lasts **84,600 s (23.5 h)**, using half-open UTC intervals.
- Reference instants were independently checked with Python 3 `zoneinfo` and installed IANA tzdb on **2026-11-15**. Runtime tests compare the checked-in JSON with separately transcribed fixed UTC strings, validate elapsed seconds, and reject duplicate zone/day keys, missing zones, reversed times and inappropriate whole-hour/23h/25h assumptions.
- This is **only a test of the fixed fixture/oracle's integrity**. There is no implemented `onDate` wrapper on `main`; the eventual Rust/WASM/Python wrapper must be tested directly against these constants, **never** against expected values derived using the same function. Future tzdb changes can alter some local-day mappings: record the tzdb version and review differences before revising the oracle.

**Verification:** Node.js **v22.16.0**, isolated local checkout containing **exact GitHub blobs** of the new test and fixture: `node --check test/non-hour-zone-local-days.test.mjs` → **PASS (exit 0)**; `node --test test/non-hour-zone-local-days.test.mjs` → **9/9 PASS (exit 0)**. The first draft's duplicate-key diagnostic test failed 1/9, was fixed, and the complete suite was rerun successfully. Tested file blob IDs: test `ad1e23558873edbd5d550d4c56a1133f594909c3`, fixture `4e8a6b8bcef7c188e9182b0040381ec786d7130e`.

**Not run:** full-repository Node/Rust test suites, any real VSEPH dataset, native cross-language `onDate` parity, browser/mobile integrations, or source database export. Do not treat fixed example checks as a timezone API implementation, source-data validation, or accuracy certification.
