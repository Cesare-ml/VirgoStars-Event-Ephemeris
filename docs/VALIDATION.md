# Validation and quality gates

**No implementation/tests have been run for this repository as of initial setup.** The following gates become executable as code is added; record actual commands and outcomes in PRs and [WORK_LOG](WORK_LOG.md).

## Gate A: documentation/specification

- README, AGENTS, license, NOTICE, ADR, API and format docs are consistent and discoverable.
- New methods map to concrete backend queries in [BACKEND_COMPATIBILITY.md](BACKEND_COMPATIBILITY.md).
- Clearly mark proposed vs implemented behavior and separate code/dataset license.
- Record changed source paths and any uncertain assumptions; validate link targets.

### Reproducible dependency lock preflight

Mandatory before Rust gates and PyO3 builds: `cargo metadata --locked --no-deps --format-version 1`; this must fail if dependency resolution would change the tracked `Cargo.lock`. If it fails, on a reviewed local development branch run `cargo generate-lockfile`, inspect `git diff -- Cargo.lock`, verify MSRV, and commit the generated lock. Do not hand-edit registry checksums. Cargo fmt may run normally; Clippy/tests/build and Rust P0 archive audit must use `--locked`. The maturin wheel workflow also uses `--locked` via CLI and `pyproject.toml`. First compilation and 40-test PyO3 gate still require a developer machine with Cargo; no claim that the current lock passes.

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

## Python-first M0 native acceptance (2026-11-12 update)

The draft PyO3 `NativeReader::new` must authenticate a **private on-disk copy** of the requested VSEPH-P0 archive against the caller's trusted SHA-256 and keep it alive until the reader is dropped. A reader's query must be stable after the original file is edited, replaced or removed; a *new* reader from the altered original must reject the digest. The owned copy removes the former two O(archive-size) SHA scans per query, trading them for one startup copy, one digest check and additional temporary disk space. The private temporary directory is not protection against a malicious process with the same access rights.

Run `cargo generate-lockfile` after updating the `tempfile` Rust dependency (existing lockfile also lacked PyO3); commit the regenerated lockfile, not manually invented crate checksums. On Windows run `py -3 tools\build_python_mvp.py` for the complete Cargo and PyO3 acceptance. Minimum **40 non-skipped Python tests** (15 API/time, 6 CLI, 10 independent CSV oracle, 9 compiled golden native fixtures). Add original P0/CSV parity for authorized source data. **These newly changed native tests, library build and lock regeneration are NOT RUN** in this authoring environment.

**Private archive lifetime:** `VirgoStarsEvents.close()` and `with ... as reader` now release the Python wrapper's native P0 snapshot reference promptly. Regression tests must check normal exit, exceptional exit, idempotent close, closed-state use rejection and native-reader deallocation. Three Python tests added; a standalone Python reproduction passed 3/3, but actual repository/native integration not yet compiled or validated. Require the updated 40-test gate before release.

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

**Python CLI resource gate (2026-11-12):** `virgostars-events` must close its privately copied verified archive on successful JSON output, lookup error and JSON error. The independently executed GitHub-source **8 CLI + 15 API/time tests PASS** (Python 3.13.5, native mocked). The owner source CSV comparator and native smoke tests also close their readers; their scientific/native execution is still pending. The full mandatory acceptance is **44 cases**, not proof of native readiness.

**CSV-oracle indexed lookup gate (2026-11-12):** For repeated original-source parity samples, `expected_row` must not allocate a full timestamp array each call. Require Python >=3.10 `bisect_right(..., key=attrgetter("at"))` and the new **two** tests proving no linear series iteration and bounded O(log N) index access. The standalone oracle CLI releases its Rust private P0 copy through context management. **Only the two new focused cases ran (2/2 PASS in isolated Python reproduction)**; the real original source comparison and native Python wheel remain unexecuted. Full installed M0 requirement: **46 non-skipped tests**.

**Source CSV verifier preflight (2026-11-12):** The standalone `tools/verify_python_synods.py` must reject malformed SHA-256/UTC probes before any large CSV reads. Case-insensitive digest is canonicalized to lowercase before native open and independent JSON dataset comparison; the separate `tools/validate_python_mvp.py` release gate intentionally requires lowercase input. Two synthetic regression cases were added, with **2/2 focused isolated reproduction PASS**, not full checked-out test execution. The installed Python/Rust M0 acceptance minimum is **48 non-skipped** cases including **16** independent source-oracle checks; this does not certify source-data completeness or a compiled wheel.

**CLI malformed-native-output boundary (2026-11-12):** A missing required field during Python projection may raise `KeyError`; the CLI must catch and display it cleanly, without a traceback or partially emitted JSON. Query `ValueError` and JSON encoding `TypeError` must have the same fail-closed behavior and release the `VirgoStarsEvents` context-managed temporary copy. Extended the existing eight-case CLI suite with three error subcases, leaving the mandatory **48-test** count unchanged. Executed **4/4** focused tests using the exact SHA-verified CLI source plus test-only dependency stubs; full compiled Rust/native gate still NOT RUN.

**Direct NativeReader trust-material gate (2026-11-12):** Both the facade `VirgoStarsEvents.open()` and directly imported PyO3 `_native.NativeReader()` must reject malformed SHA-256 *before* touching the original P0 file or temporary storage. Direct constructor enforces exactly 64 lower-case ASCII hex bytes. Test invalid digests against a deliberately missing source path to assert `ValueError` first; a valid SHA on the same missing path must instead raise `OSError`. This extends an existing 9-case native suite without raising the native test count; a concurrent API change sets the current M0 baseline to **49 tests** (16+8+16+9). **Native/Rust compilation and tests NOT RUN** in this environment.

**Independent source oracle identity checks (2026-11-12):** The recorded synod CSV/PyO3 comparison must validate the full response envelope, not just events and SHA: `dataset.format == "VSEPH-P0"`, `dataset.id is None` with the key present, `dataset.sha256` equal to the independently trusted digest, and `coverage_certified is False`. A recorded Moon's `new_moon` must be the same object as `synods["Moon"]`; before any Moon anchor, `new_moon` must be `None`. Mutated dataset identity and detached Moon alias now fail source parity. Eight **isolated reproductions** of the exact predicates passed on Python 3.13.5; the full 16-source-oracle + 9-native/49-M0 test gates are NOT RUN in this intervention.

**Raw Rust-to-Python Moon alias transport integrity (2026-11-12):** The wrapper must reject native `new_moon` values that disagree with `synods["Moon"]` **before** projecting timestamp units or overwriting the convenience field. The source JSON contains a value-equal (not necessarily same-object) copy of the latest recorded Moon row; if there is no recorded predecessor, its native `new_moon` must be explicit JSON `null`. Reject missing alias fields, differing source IDs or phases, non-null aliases before the first Moon, and non-object `synods` or body members. The public projected Python response must still alias the same Moon dictionary object. Executed **9 focused local Python 3.13.5 tests**, with exact GitHub-file SHA verification for the wrapper and time helper; **not a compiled PyO3/native gate**. Complete installed-wheel acceptance remains **50 non-skipped** (17+8+16+9).

**Authenticated native response envelope (2026-11-12):** The public Python facade must verify a native JSON `dataset` mapping with `format == "VSEPH-P0"`, an explicit `id: null`, `coverage_certified is False` and a canonical 64-digit lowercase hexadecimal SHA-256. `VirgoStarsEvents.open` retains the independently supplied trusted digest and rejects a native response whose reported SHA-256 differs, even if syntactically valid. Reject malformed/omitted dataset metadata **before** projecting phase/UTC fields. The native Rust constructor remains responsible for authenticating the copied P0 bytes themselves. Added source-level mutation subcases to existing API tests without raising the test method count. **12/12 focused Python 3.13.5 tests PASS** against the exact committed wrapper, with a mock Rust reader; **not** proof of native compilation. Mandatory full gate remains **50 non-skipped** = 17 API/time + 8 CLI + 16 independent CSV oracle + 9 compiled native fixture.
