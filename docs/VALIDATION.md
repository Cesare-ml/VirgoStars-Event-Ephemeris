# Validation and quality gates

**No implementation/tests have been run for this repository as of initial setup.** The following gates become executable as code is added; record actual commands and outcomes in PRs and [WORK_LOG](WORK_LOG.md).

## Mandatory portable P0 byte-integrity preflight (2026-11-12, PR #5)

Run `py -3 tools\validate_p0_synthetic.py` from a complete source checkout **before Cargo**. The builder `tools/build_python_mvp.py` now runs this gate immediately after the existing 42-case source gate and before `tools/validate_rust.py`; a nonzero exit blocks wheel creation. The independent P0 audit suite has 15 authored methods: **14 fully synthetic required** and **one separately authorized real snapshot audit** (`test_real_snapshot_full_block_audit`) which is explicitly excluded rather than silently skipped. The helper fails closed if the optional method disappears, the synthetic method count changes, a test is skipped/expected-failed/failed, or execution stops early.

**Actually executed on exact GitHub source bytes:** independently audited the original 1,470-byte / 4-block / 8-event fixture (SHA-256 `4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499`). The 15-test audit module yielded **14 PASS, 1 intentional owner-data skip**; the dedicated gate yielded **14/14 PASS, zero skips**, and the gate's 4 authored unit regressions yielded **4/4 PASS**, Python 3.13.5. Local `git hash-object` matched exact GitHub blobs for the auditor `77f8f963a2a40fa41faaf9ca49a49e7193d09c7f`, original audit test `53c03f9144534f706123a0e3b1c69f490640df0e`, golden fixture `64f38ded0a336f0d2652e994bf87b8e16ce8d393`, and new gate and tests `894a22d2f645db27783a84ea527624243769e4e1` / `cc32c3e995f257371f1ace77d6dcffa9c12b709d`. The 42 source-only and 51 native installed-wheel acceptance suites and all Rust/Cargo tests **remain unexecuted here**; this gate is additive, not a replacement.

## M0 canonical unsigned varints in the experimental P0 decoder (2026-11-12, PR #5)

**Binary contract:** P0 uses unsigned LEB128 for source-row IDs, event timestamp deltas, synod extra-count markers and recorded phase deltas. Each value must have its **shortest encoding**: a multi-byte varint ending in a zero high group (for example `80 00`, `81 00` or `80 80 00`) is invalid, even if it decodes numerically to an otherwise allowed value. An encoding representing 128 as `80 01` is valid. Existing truncated/10-byte overflow checks remain mandatory. This tightens the experimental P0 parser only; it does **not** freeze the future VSEPH v1 specification or calculate astronomical events.

The Rust `crates/events-p0/src/lib.rs::take_varint` and independent Python `tools/audit_p0_bytes.py::_varint` now reject such redundant encodings. The Rust source includes **3 new unit tests** for canonical values (including `u64::MAX`), malformed/truncated/overflow values, and complete record decoding with noncanonical source IDs/deltas. The Python auditor adds **2 test methods** in `tools/test_audit_p0_bytes.py`: direct numeric cases and **four complete synthetic archives** with valid JSON TOCs, zlib blocks and SHA-256 checksums, but noncanonical original ID, timestamp delta, phase count or phase offset. A canonical archive control remains accepted. The independent byte auditor is a validation oracle only, not a second scientific runtime.

**Actually executed in this authoring environment:** Python 3 stdlib focused reproduction of the exact new shortest-varint predicate and checksum-valid P0 test construction, `python /tmp/check_p0_canonical_varint.py`: **8/8 PASS** (no skips). **Not executed here:** the exact committed `tools/test_audit_p0_bytes.py` module, the three added Rust tests, Cargo fmt/Clippy/build, the full 42 Python source tests, 51 installed-native wheel tests, or owner-authorized original dataset parity. Cargo/rustc/maturin remain unavailable. On a Rust-equipped clone, run:

```powershell
py -3 -m unittest discover -s tools -p "test_audit_p0_bytes.py" -v
cargo test --locked -p virgostars-events-p0 canonical_varint_tests
py -3 tools\validate_python_sources.py
py -3 tools\build_python_mvp.py
```

## M0 canonical P0 block digests (2026-11-12, draft PR #5)

The experimental Rust `P0Archive::open()` now rejects a noncanonical `raw_sha256` value in **each TOC block at header parsing time**, rather than accepting uppercase hex and only failing later when comparing against the lowercase computed SHA. A shared `canonical_lowercase_sha256` helper requires **exactly 64 lowercase ASCII hexadecimal characters** both for block digests in the header and the caller's separately trusted archive digest. Valid archived block bytes, decoded records and checksum scope are unchanged.

The Rust source includes two new `#[test]` functions: lower-case digest syntax acceptance/rejection, and an adversarial test based on the repository's independent `synthetic.p0.base64` fixture, changing **only the first TOC digest's letter case**. It requires `P0Archive::open()` itself to return `Invalid` for this file (rather than silently accepting the header). No dataset is published or edited.

**Executed now:** Python 3.13.5 standalone reproduction `python /tmp/vseph_p0_sha_case.py`: **4/4 PASS** on the exact synthetic base64 fixture; original SHA-256 `4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499`, original **1,470 bytes**, original **1,361-byte header**. Confirmed that the adversarial file retains the identical compressed payload and file length, and all four original block digests are canonical lowercase. This reference reproduction **does not execute Rust**. The two new Rust tests, Cargo validation, compiled PyO3 wheel, 42 source Python cases and 51 native installed-wheel cases remain unexecuted from this authoring environment. On a Cargo-equipped clean checkout (after reviewed Cargo.lock regeneration), run:

```powershell
cargo test --locked -p virgostars-events-p0 sha256_syntax_tests
cargo test --locked -p virgostars-events-p0
py -3 tools\validate_python_sources.py
py -3 tools\build_python_mvp.py
```

## M0 independent P0 audit: physical byte length and Rust format (2026-11-12, draft PR #5)

The Rust `p0-verify` CLI now includes the source archive's **actual on-disk byte length** as `"bytes"` in its JSON output, measured via filesystem metadata. The independent Python `tools/audit_p0_bytes.py` already emitted `"bytes"`; `tools/validate_rust.py --archive ... --sha256 ...` now requires matching positive **integer** lengths in both reports (never accepting absent/matching-null, boolean, float, string or zero values). It also requires the Rust-reported `"format"` to equal `"VSEPH-P0-EXPERIMENTAL"`. Existing trusted SHA-256, decoded block/record/family comparisons and `coverage_certified: false` remain mandatory. Three new `tools/test_validate_rust.py` regression methods cover mismatched lengths, missing/malformed lengths and wrong/missing Rust format.

**Actually executed:** independent stdlib-only Python 3.13.5 reference checks on original synthetic P0 bytes recovered `bytes=1470`, `blocks=4`, `records=8`, sections `ingress=3, motion=2, synod=3` and digest `4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499`. A separate local reproduction of the new audit comparison predicate passed **9/9 tests**. **Not executed:** the 14 authored exact repository orchestration tests, Rust compilation or genuine dual-binary Python/Rust audit, the 42 Python source gate, the 51 installed-wheel native acceptance tests, or authorized original-data parity. `Cargo.lock` still needs real Cargo regeneration. No scientific completeness inference.

## M0 compiled PyO3 golden-boundary acceptance (2026-11-12, draft PR #5)

The existing native integration suite `python/tests/test_native_fixture.py` now checks **13 timestamp subcases inside one existing test**, using the independently authored `crates/events-p0/tests/fixtures/synthetic.p0.base64` fixture. The recorded Moon source rows are ID 10 at UTC microseconds `-876544` (SQL `NULL` phase thresholds) and ID 12 at `1500000` (thresholds `1700000, 1800000, 1900000`); there is **no third Moon anchor**. The test proves inclusive phase starts and exact -1/at/+1 microsecond transitions, source-ID selection before/at the new Moon, and `unknown_open_ended_cycle` with no invented `CAL2` after the last recorded phase threshold. It also asserts that the public `new_moon` is the same Python object as `synods["Moon"]`.

The original fixture file's Git blob `64f38ded0a336f0d2652e994bf87b8e16ce8d393` was verified in the local test environment, as was the decoded byte SHA-256 `4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499`. An **independent standard-library P0 decode/reference check ran 13/13 PASS** on those original bytes. This validates the fixture and expected outputs, **not** the compiled Rust binding; the edited native test still needs to run on a real wheel. The required acceptance count remains **51** (18 Python API/time + 8 CLI + 16 source CSV oracle + 9 compiled/native P0 fixture). Do not count the independent reference check as PyO3 acceptance.

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


**Source-before-Rust release validation (2026-11-12):** `tools/build_python_mvp.py` now runs `tools/validate_python_sources.py` before Cargo and maturin. It is a **41 non-skipped Python test preflight** intended to detect a broken API/oracle/CLI without wasting a native build; the final separate wheel gate still requires **50 non-skipped tests**, a real PyO3 extension and approved owner CSV/P0 comparison when supplied. Two additional orchestrator regression methods verify that preflight and Rust failures stop before a wheel is saved. Local 4/4 isolated Python gate-order simulation passed; the exact on-branch orchestrator suite and native compilation were **NOT RUN** in this environment.

**Fail-closed expected-failure regression (2026-11-12):** For both source-only `tools/validate_python_sources.py` and native installed-wheel `tools/validate_python_mvp.py`, accept a discovered unittest suite only when every case actually executed and passed. Reject `result.expectedFailures` even when `result.wasSuccessful()` says True, and explicitly reject `result.unexpectedSuccesses`, `result.skipped`, ordinary failures, errors and partial runs. Added tests with real synthetic temporary unittest modules to reproduce the false `OK (expected failures=1)` path. **8/8** isolated Python 3.13.5 acceptance-condition checks PASSED with real unittest results. Full repository source/wheel suites not run in this intervention; original data and Rust compilation still unverified. Mandatory counts unchanged: 41 source-only and 50 installed native.

**Experimental native per-body coverage must remain unreviewed (2026-11-12):** `VirgoStarsEvents.get_synods_at` fails if any native synod record omits `coverage`, changes `coverage.status` away from `unreviewed`, removes `no_reviewed_coverage_policy` from the list-valued warnings, or returns a non-object `phase`. Additional gap warnings are valid. This enforces the absence of scientific coverage certification at the public Python boundary without duplicating astronomical calculations. Seven mutation/acceptance subcases were added to an existing 18-method API suite. An isolated reproduction of the exact coverage/phase predicates passed **8/8 Python 3.13.5 tests**. The full current source gate is **42 non-skipped** (18 API + 8 CLI + 16 oracle); compiled/native acceptance is **51 non-skipped** (42 + 9), still NOT RUN. Older dated test counts in this chronological log are historical, not today's target.

**Parity timestamp representability (2026-11-12):** Normalized CSVs can encode earliest/largest Python-representable UTC timestamps. After sampling source anchors, phase thresholds, historical Moon gaps and mandatory regression dates, `select_probes` discards *only* synthetic instants earlier than year 0001 or later than `9999-12-31T23:59:59.999999Z`. This prevents `OverflowError`/unrepresentable date failures in `iso_utc` during otherwise legitimate owner CSV parity, while retaining exact boundary source anchors and valid neighbors. Extended an existing source-oracle regression method; test count remains 16 independent CSV oracle cases. **9/9 isolated local Python 3.13.5 tests passed**, including round-trips; full on-branch 42 source tests, 51 compiled/native tests and authorized original-data parity have **NOT** run. Earlier chronological counts in this document are historical.

**Required JSON key presence during CSV/PyO3 parity (2026-11-12):** The source oracle now checks `key in record` before comparing each mandatory field, and `key in phase` before comparing phase fields. This prevents `dict.get()` from accepting missing keys whenever the expected source value is null. Even with no earlier Moon, the public `new_moon` field must explicitly exist and equal `null`; missing key is invalid. Regression cases cover omitted source ID, current/next anchor, phase thresholds, code, started-at timestamp and missing `new_moon` for both a pre-first-Moon query and a source with no Moon series. No new test methods: **16 independent oracle tests**, **42 source-only**, **51 installed PyO3**. A **12/12** isolated Python 3.13.5 test reproduced the new comparisons (not an execution of the actual GitHub modules). Compiled Rust/native and authorized owner-data parity remain **NOT RUN**.

**Strict source-CSV quoting gate (2026-11-12, draft PR #5):** `tools/verify_python_synods.py::load_synod_csv` now creates its `csv.DictReader` with `strict=True` and converts `csv.Error` into a clear `ValueError` with the physical CSV line number. The default permissive Python CSV parser can consume later scientific rows into an unterminated quoted field. Rejecting this ambiguous input is required before independent Rust/CSV parity; correctly quoted multiline JSON phase arrays remain supported. Extended the existing source-oracle test method (16 total) with malformed-quote rejection and valid multiline JSON. **5/5 Python 3.13.5 isolated tests passed** using the committed loader logic and a test-only source-normalization substitute; this was not the complete exact GitHub source suite, the 42-case source gate, a compiled 51-case PyO3 wheel or owner-authorized data parity.
