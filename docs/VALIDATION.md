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

## Gate D supplement: synthetic Rome local-day event membership (issue #24)

For the **proposed** `motions.onDate`, `ingresses.onDate` and `events.onDate` adapters, use [the synthetic UTC-microsecond membership fixture](../test/fixtures/local-day-event-membership.json) and its [offline Node tests](../test/local-day-event-membership.test.mjs). The fixture covers the Europe/Rome 2026 spring-forward **23-hour day** and autumn-back **25-hour day**, plus adjacent local dates. Its invented motion and ingress event IDs exercise exact inclusive start, exclusive end and ±1 µs neighboring instants; `BigInt` comparisons avoid JavaScript floating-point timestamp loss. The independent expected ID lists prevent an accidental matching implementation from silently updating the oracle.

Run `node --check test/local-day-event-membership.test.mjs` and `node --test test/local-day-event-membership.test.mjs` from a complete checkout. This validates the **synthetic acceptance fixture only**. A future real Rust UTC-range primitive and IANA-zone wrapper must return the same event IDs and boundaries in native Rust, WASM and Python, with explicit coverage failures; no production `onDate` implementation is validated by this fixture. The equal-timestamp cross-family tie policy is not assumed here: fixture event instants are all distinct. These 2026 UTC bounds were independently sourced from [the separate date-only golden fixture PR #22](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/22), whose IANA timezone data are not version-pinned; revalidate against the chosen timezone implementation and tzdb release. Reconcile overlapping documentation with #22 during integration. No real event data, astronomy calculations or dataset rights are changed.
