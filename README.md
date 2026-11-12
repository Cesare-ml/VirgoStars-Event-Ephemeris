# VirgoStars Event Ephemeris

**Independent, free/open-source, offline-first event ephemerides for astrology.**

**Status: EXPERIMENTAL RUST IMPLEMENTATION (draft PR #5).** This branch has an in-memory Rust event index and a separate experimental reader for the VSEPH-P0 archive. It does not yet contain a verified Rust VSEPH v1 reader, JS/TS bindings, a tested/installed Python wheel, release packages or production integration. Experimental PyO3 and Python binding **source** exists but is not yet proven to compile end-to-end. The historical dataset, binary and experimental Python reference are staged separately in [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), not merged into this branch. API examples for unimplemented methods remain design proposals.

**Rust/Python transport integrity (2026-11-12, draft PR #5):** The Python facade now refuses a native response unless it is a JSON object with an actual integer `requested_at_us` exactly matching the caller's requested UTC microsecond count. A wrong, missing, boolean, float or string timestamp raises `ValueError` rather than silently relabeling events from another instant. One Python API regression covers these cases and non-object JSON. The source-only acceptance threshold is now **41** tests, and the compiled PyO3 wheel requires **50 non-skipped** tests. Only an isolated 14-case integrity reproduction ran locally; a complete installed/native build has **not** yet been verified.

**P0 byte-audit parity (2026-11-12, draft PR #5):** The local `p0-verify` Rust CLI now emits the physical archive `bytes` count. When a separately trusted local snapshot is supplied, `tools/validate_rust.py` rejects a malformed/missing byte count, Python/Rust size mismatch or a Rust response claiming a format other than `VSEPH-P0-EXPERIMENTAL`, alongside existing SHA/block/event-count checks. Three new Python orchestration tests cover these failures. The original independent synthetic P0 reference was checked as **1470 bytes, 4 blocks, 8 events**, and **9/9 local predicate-reproduction tests passed**. This does **not** replace actual Cargo/native tests, which remain blocked on `Cargo.lock` regeneration.

## First product milestone: Python synods, Moon new moon and current cycle phase

**The first usable MVP is a locally pip-installable Python library**, ahead of WASM, a stable VSEPH v1 format, and production replacement. The planned API is:

```python
from virgostars_events import VirgoStarsEvents

with VirgoStarsEvents.open(
    "local-events.vseph-p0", expected_sha256="<trusted dataset SHA-256>"
) as events:
    result = events.get_synods_at("2026-11-11T12:00:00Z")
    print(result["synods"]["Mercury"]["phase"]["code"])
    print(result["new_moon"]["cycle_start_utc"])
```

**The Python package source is now on draft PR #5, but this example is NOT YET VALIDATED against a compiled Rust wheel.** Before running it, build/install the local package using [PYTHON_MVP_BUILD.md](docs/PYTHON_MVP_BUILD.md) and supply an independently trusted local P0 archive. An earlier set of 8 pure-Python wrapper tests passed; newly expanded timestamp and CLI tests still require execution. Rust native compilation and pip installation remain unverified. Implement the same Rust reader via PyO3/maturin, package as a locally installable wheel (`pip install .` after a build backend is added), and require explicit caller-supplied local data plus a trusted digest. The result is a JSON-compatible dictionary of latest **recorded** cycle anchors `<= X`, Moon's latest recorded new moon and source-threshold-derived D1/R2/R3/D4 or Moon quarter-stage code, with transparent unknowns/coverage warnings. No interpolation or scientific completeness guarantees.

A local command-line entry point has also been written (but is not yet tested with a built wheel):

```powershell
# After local wheel installation:
python -m virgostars_events "local-events.vseph-p0" "2026-11-11T12:00:00Z" --sha256 "<trusted SHA-256>"
# The identical command is registered as "virgostars-events" by pip.
```

Both commands return JSON from the same Rust-backed API, require a user-provided local archive and trusted checksum, and never download or bundle the dataset. Input timestamps must contain an explicit UTC offset with **at most six fractional-second digits**; submicrosecond timestamps are rejected rather than silently truncated.

Input validation now fails early on a malformed SHA-256, an impossible local DST datetime or a malformed CLI instant, without opening the archive on invalid input. The expanded **48-test** acceptance suite still requires a compiled PyO3 wheel; its native checks have **NOT RUN** on this branch.

A clean **offline wheel-installation gate** is now available after local `maturin build --release --out dist`: `py -3 tools\verify_python_wheel.py --wheel path\to\matching.whl`. It creates an isolated venv, installs only that wheel without an index, checks installed package/native imports and the offline CLI, then invokes the mandatory fail-closed Python MVP suite. Optional `--archive`, `--sha256`, and `--synods-csv` add authorized original-data comparisons. The gate's **8 orchestrator unit tests passed**, and an actual offline installation of a deliberately invalid wheel failed as expected because it contained a Python stub instead of a compiled Rust extension. It also checks the installed `virgostars-events` command with inherited Python import overrides stripped from subprocesses. **No actual compiled native wheel has yet passed this gate**; see the [Windows build guide](docs/PYTHON_MVP_BUILD.md).

To run **all required local build and installation checks together** (on a source checkout with Cargo and maturin installed), use `py -3 tools\build_python_mvp.py`. It compiles a new candidate wheel in a temporary directory, invokes Rust validation and the isolated Python/native acceptance gate, and only retains the verified wheel in `dist/`; it never releases to PyPI, bundles the dataset or overwrites a different wheel. Its **7 orchestration tests passed**, but the actual Rust/Windows wheel has **not** been built or installed in this agent environment. See the [M0 build guide](docs/PYTHON_MVP_BUILD.md) before treating it as a usable wheel.

**Explicit resource cleanup:** `VirgoStarsEvents.close()` releases the Rust-owned private archive snapshot as soon as the wrapper no longer has other native owners; `with VirgoStarsEvents.open(...) as events:` automatically closes it even on exceptions. `events.closed` reports the state, and querying or re-entering a closed wrapper raises `RuntimeError`. The three new lifecycle tests pass in an isolated Python reproduction; the compiled PyO3 wheel has **not** been tested. The baseline is **40 required non-skipped tests** (15 API/time + 6 CLI + 12 source oracle + 9 native golden fixture). Context management helps avoid retaining temporary disk copies longer than needed.

**Lockfile gate before the first Windows wheel:** the tracked `Cargo.lock` currently omits the PyO3 binding crate and `tempfile` and has **not** been regenerated here. On your Cargo-enabled development checkout run `cargo generate-lockfile`, inspect/commit the reviewed `Cargo.lock`, then `py -3 tools\build_python_mvp.py`. Rust validation starts with `cargo metadata --locked`; Cargo compile/test commands, maturin wheel builds and PEP 517 packaging all require a current lockfile and refuse implicit updates. This is source reproducibility, not successful native compilation or an offline crates.io guarantee. See [Rust validation](docs/LOCAL_RUST_CHECKS.md).

**CLI snapshot cleanup and executed tests (2026-11-12):** The `virgostars-events` CLI now uses the Python context manager, releasing the verified private archive copy on normal output and query/JSON errors. The owner-data CSV parity validator and native owner smoke tests also close their readers explicitly. **8/8 CLI plus 15/15 Python API/time tests passed** on Python 3.13.5 with exact GitHub-committed files (confirmed by Git blob hashes), using mocked native readers. The full acceptance gate is now **44 non-skipped cases**: 15 API/time + 8 CLI + 12 independent source oracle + 9 compiled native P0. Compiled Rust/PyO3, owner-data parity and wheel installation **remain NOT RUN**.

See [Python-first MVP specification and acceptance tests](docs/PYTHON_FIRST_MVP.md), [delivery milestones M0–M5](docs/DELIVERY_MILESTONES.md), [roadmap](docs/ROADMAP.md) and [backend compatibility](docs/BACKEND_COMPATIBILITY.md). Public PyPI publication and redistribution of the separately licensed dataset are **not approved yet**.

## Scope

One portable, indexed, versioned dataset format (**VSEPH**, proposed) with three astronomical event families:

| Family | Source table in the current VirgoStars backend | Baseline rows* |
| --- | --- | ---: |
| Synodic cycles, including lunar new/full/quarter phases and supported planetary cycle phases | `sinodi` | 70,554 |
| Planetary direction changes (direct/retrograde stations) | `moto_pianeti` | 266,516 |
| Zodiac-sign ingresses, including retrograde re-entries | `ingresso_segni` | 43,774 |
| **Total** | | **380,844** |

*Counts reported by the dataset author on 2026-11-10, **not independently verified from an export**. Historical coverage, completeness per body, numerical precision and resulting file size are not yet established.

This is a **standalone library**, not a module of `VirgoStars-Ephemeris` or `astrological-functions`. The runtime must not require PostgreSQL, network access, Swiss Ephemeris, NASA/JPL kernels, a license server, or an astronomical calculation engine. An **optional development-time PostgreSQL exporter** may consume the maintainer's existing tables; the shipped reader must not.

## Technical direction

- **Rust** is the authoritative binary parser, validator and indexed query engine, with no runtime DB dependency.
- **JavaScript/TypeScript** uses the same Rust engine via WebAssembly, with TypeScript typings and browser/Node support.
- **Python** uses the same Rust engine via PyO3/maturin wheels. Native Rust consumers use the crate directly.
- Dataset, binary file format and language bindings have independent versioning and compatibility rules.
- All astronomical moments use explicit UTC/POSIX-time semantics; date-only queries must specify a time zone or use an explicitly documented UTC default. Do not silently convert `timestamp without time zone` values.
- Stable API targets: `synods.at`, `synods.atAll`, `synods.phaseAt`, `synod.next()` / `previous()`, `motions.onDate` / `stateAt` / `stateAll`, `ingresses.onDate` / `next` / `signAt`, and unified `events.between`.

A major goal is **behavioral parity** with the astronomical queries actually used by the [VirgoStars backend](https://github.com/Cesare-ml/virgo-stars), including its date-range, month, timezone, phase and cycle-navigation use cases, **without reproducing known legacy mistakes**. See [backend mapping](docs/BACKEND_COMPATIBILITY.md).

## Read before contributing

1. [AGENTS.md](AGENTS.md) — mandatory instructions, work selection, safety, quality gates and handoff for human/AI contributors.
2. [Architecture](docs/ARCHITECTURE.md) and [ADR-0001](docs/ADR-0001-RUST-BINDINGS.md).
3. [Proposed VSEPH binary format](docs/VSEPH_FORMAT.md) and [public API contracts](docs/API_CONTRACTS.md).
4. [Backend compatibility inventory](docs/BACKEND_COMPATIBILITY.md) and [validation plan](docs/VALIDATION.md).
5. [Data provenance / ownership](docs/DATA_PROVENANCE.md), [licensing & credits](docs/LICENSING_AND_CREDITS.md), [NOTICE](NOTICE) and [CPAL-1.0 LICENSE](LICENSE).
6. [Roadmap and task queue](docs/ROADMAP.md), [contributing](CONTRIBUTING.md), [release policy](docs/RELEASE_PROCESS.md) and [Italian overview](docs/README.it.md).

## Ownership, license and attribution

The project maintainer states that the source dataset is **entirely their original work generated using their own algorithms**. This is a recorded authorship statement, not an independent audit. **No dataset files have been published here and no dataset redistribution license has yet been selected**; the source-code license must not be presumed to license later binary datasets. Document the exact dataset license before the first data release.

The **original source code** is licensed under [CPAL-1.0](LICENSE), following [VirgoStars-Ephemeris](https://github.com/Cesare-ml/VirgoStars-Ephemeris): completed Exhibits A/B include the original developer's attribution:
 
> Copyright (c) 2026 Cesare-ml (VirgoStars project)  
> Powered by VirgoStars  
> https://www.virgostars.it/

See the full license, especially `14 (GUI launch/session attribution, including Larger Works as specified) and `15 (network use). An About/Credits link is good practice but does not automatically satisfy every applicable display condition. Code remains free to use, study, modify and redistribute **subject to the license**; do not interpret “free” as a waiver of legal notices.

## What does not belong here

No secret credentials, exported private DB copies, telemetry, interpretive horoscope text, Telegram metadata, notification queues, GUI artwork, non-approved third-party data, or astronomical calculation algorithms. Keep optional PostgreSQL access isolated to an exporter used **only during dataset generation**.

## Development status and first deliverable

**Next milestone:** approve public API, freeze a first **draft** format specification, build tiny synthetic fixtures, write an executable Rust reader prototype with strict error handling and tests, then implement a read-only exporter/validator that can prove exact parity on an *authorized* snapshot. This repository contains **specifications only** until these gates pass.

For work opportunities and acceptance criteria, use [the roadmap](docs/ROADMAP.md); do not claim feature completion until code, tests and documentation are merged.

### Rust prototype: recorded events and reviewed coverage

The draft Rust branch provides `EventIndex` and `virgostars-events-p0`, including a SHA-256-bound `CoverageAwareP0` adapter. It distinguishes the last recorded ingress/station from a scientifically reviewed state; **no time intervals are approved for the real 2026-11-10 dataset**. See [the Rust coverage contract](docs/REVIEWED_COVERAGE_RUST.md) and [developer guide](docs/DEVELOPMENT.md). These changes are in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) and are not merged into main.

### Reproducible experimental byte integrity audit

An independent Python test tool in draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) validates **all** P0 compressed blocks and encoded records, including file/block SHA-256 and corruption checks. The actual 2026-11-10 P0 file passed **761 block checks / 380,844 record decodes**; see [auditor instructions](docs/P0_BYTE_AUDIT.md). This is a check of source **bytes**, not missing astronomical events or the still-unverified Rust decoder; `cargo test` is not yet run.

### Verify the entire experimental compressed archive

On the Rust development branch (draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)), the new `P0Archive::verify_all()` validates **every** block, not only blocks accessed by an individual query. A provisional offline command is available after installing Rust and reconstructing the dataset from [PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6):

```sh
cargo run -p virgostars-events-p0 --bin p0-verify -- datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0
```

It returns JSON event counts, full-file `sha256` and a nonzero exit status for invalid blocks, with `coverage_certified: false`. Add `--sha256 <trusted-manifest-digest>` to reject a mismatched or corrupt **entire** archive; do not trust an unauthenticated digest printed by the file itself. The executable is **not yet compiled/tested with Cargo** in this authoring environment. The independent Python verifier has executed successfully across 761 blocks and 380,844 records, with 8/8 tests passing. See [P0 byte-audit instructions](docs/P0_BYTE_AUDIT.md) for steps and checksum verification. Neither program fills astronomical coverage gaps.

### One-command local Rust gates (Windows, macOS, Linux)

For contributors who need to validate a Rust change without GitHub Actions, draft PR #5 now includes a cross-platform Python wrapper:

```sh
python tools/validate_rust.py
```

It runs `cargo fmt`, `cargo clippy`, workspace tests and release build in sequence, stopping at the first failure. To additionally validate the experimental VSEPH-P0 archive, pass `--archive <path> --sha256 <trusted-manifest-digest>`; the wrapper independently checks the complete archive using Python and the Rust CLI and **requires the two JSON summaries to match**. On Windows use `py -3 tools\\validate_rust.py`.

See [local Rust validation instructions](docs/LOCAL_RUST_CHECKS.md). The helper's own **9 Python tests passed**; this does **not** establish that the Rust compiler/build has run in the current environment. The experimental adapter and original dataset are still staged on separate draft PRs.


### Verify public CSV fields against the compressed archive

The experimental development tool `tools/verify_csv_p0.py` (draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)) compares **every scientific field** in the three normalized public CSV tables (draft PR #6) with the compressed VSEPH-P0 file by original PostgreSQL row ID and celestial body. This detects missing or modified events, changed signs/directions and loss of synod phases (including NULL vs an explicit empty list). The 2026-11-10 snapshot passed **380,844/380,844** individual comparisons, with **11/11** synthetic regression tests passing. No missing dates are inferred or generated.

After assembling the archived data in a temporary combined PR checkout:

```sh
python tools/verify_csv_p0.py datasets/2026-11-10 datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

See [CSV ↔ P0 field parity](docs/CSV_P0_FIELD_PARITY.md). This is Python 3.10+ standard-library tooling, **not** evidence that the Rust parser has compiled or that historical coverage is complete.

### PostgreSQL row identity checks

The experimental P0 reader and independent Python byte auditor now reject **zero or reused original source row IDs in one event family**, even when duplicates occur in different compressed blocks or celestial bodies. IDs are allowed to coincide **across the three independent source tables**. The original 2026-11-10 archive passed all identity checks: **380,844 valid rows**, **761 indexed blocks**. See [P0 byte-audit guide](docs/P0_BYTE_AUDIT.md); the Python suite reports **13/13 PASS**, while Rust tests/compilation remain **NOT RUN** in the authoring environment.

### Chronological merged event pages — experimental Rust core

Draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) now provides `EventIndex::chronological_between(start, end, limit, after)` for **recorded events** from all three families. It uses a stable timestamp/family/body tie order, cursor pagination, a 10,000-record maximum page size and half-open UTC microsecond ranges, without an unbounded materialization of the selected interval. Missing astronomical events are never interpolated or declared complete.

An **experimental file-backed counterpart** now exists: `P0Archive::chronological_between(start, end, limit, after)` returns owned `P0Record` values with original source IDs while decoding at most one 512-event compressed block per active series at a time. Both readers bind cursor continuation to the same exact time interval **and process-local reader instance**, so a reopened archive cannot reuse a stale cursor even when its bytes are identical. Seven in-memory plus eight compressed-reader Rust integration tests have been written but **have not yet been run** for these changes. The public VSEPH v1 API remains pending. See [chronological paging guide](docs/P0_CHRONOLOGICAL_PAGING.md) and [developer notes](docs/DEVELOPMENT.md). Do not mistake recorded events for certified complete historical coverage.

**Verified private snapshot (M0, 2026-11-12):** `NativeReader::new` now checks source P0 framing, copies the requested archive into a securely created, per-reader temporary directory, and authenticates the **copied bytes** using the independently supplied trusted SHA-256. All subsequent Rust indexed queries read only that private copy, even when the original is edited, replaced or removed. The temporary copy is deleted when the native reader is dropped. This avoids two whole-archive SHA scans on **every query**, at the cost of a one-time disk copy, SHA scan and temporary disk space. The new synthetic native source-change tests are written but **NOT RUN** with PyO3 here. Mandatory suite: **37 tests** (9 native + 12 API/time + 6 CLI + 10 independent CSV oracle). Not an adversary-resistant file lock, and the actual Rust wheel remains uncompiled in this environment.

**Independent CSV parity performance (2026-11-12):** `tools/verify_python_synods.py::expected_row` now uses Python 3.10+ `bisect_right(series, instant, key=attrgetter("at"))` on sorted events, rather than allocating a whole new timestamp array for *every probe*. A reader with 37,054 synthetic Moon anchors and 570 repeated lookups produced the same results without allocation (local illustrative timings: ~0.24 s old vs ~0.0003 s new; not a production benchmark). Two new tests require no iteration or O(N) copying even for a virtual million-event input; 2/2 passed in an isolated Python 3.13.5 reproduction. The standalone CSV parity CLI now context-closes its verified native P0 copy on errors and completion. The full M0 installed-wheel minimum is **46** (15 Python API/time, 8 CLI, 14 CSV oracle, 9 PyO3 native fixture); compiled wheel/native tests remain pending.

**Source-parity CLI preflight (2026-11-12):** `tools/verify_python_synods.py` now verifies a 64-hex SHA-256 and an exact UTC instant **before** reading a potentially large source CSV. It canonicalizes an uppercase SHA-256 to lowercase for both the verified native reader and the parity comparison (the same digest, no data change), and reports missing-native-import errors without unhandled tracebacks. Two source-oracle tests cover invalid-input early rejection and uppercase-digest parity with a context-managed synthetic reader; their behavior passed a 2/2 **isolated Python reproduction**, not the full repository suite. The stricter `tools/validate_python_mvp.py` release gate still explicitly requires lowercase owner SHA-256. Its required suite is **48 non-skipped tests** = 15 API/time + 8 CLI + 16 independent source oracle + 9 compiled native.

**CLI error handling (2026-11-12):** The offline `virgostars-events` command now reports a missing expected native-output field (`KeyError`) as a clean nonzero CLI error rather than emitting a Python traceback. The existing CLI regression was extended with three failure subcases: query `ValueError`, malformed native field `KeyError`, and failed JSON serialization `TypeError`. All must leave stdout empty and close the verified per-reader temporary P0 snapshot. The **exact GitHub CLI module**, verified by Git blob SHA-1 `a438eeb3738fc2396568af050456f60dff8f6a09`, passed **4/4 focused local Python 3.13 tests** against lightweight stub dependencies (three errors plus successful JSON). This is not compiled Rust/PyO3 acceptance; the baseline remains **48 mandatory non-skipped tests**.

**Independent Python source-parity identity gate (2026-11-12):** `tools/verify_python_synods.py` now rejects native responses that have a missing/incorrect `dataset.format` (must be `VSEPH-P0`) or a missing/non-null `dataset.id` (P0 has no separately assigned dataset ID), even if their source IDs and SHA-256 superficially match. It also requires `new_moon` to be **the same Python dictionary object** as `synods["Moon"]` whenever an earlier recorded Moon anchor exists; a value-equal detached copy is no longer accepted as an alias. Existing source-oracle test methods gained mutation subcases for these regressions. **Eight isolated Python 3.13.5 predicate checks passed**, but these are not a full run of the GitHub suite or any compiled Rust/PyO3 tests. The required installed-wheel acceptance remains **49 non-skipped tests** (16 API + 8 CLI + 16 independent CSV oracle + 9 native).

**Raw native Moon alias verification (2026-11-12, PR #5):** Before converting Rust-native microseconds into UTC strings, `VirgoStarsEvents.get_synods_at` now checks that `synods` is a dictionary of records and that the native `new_moon` field is present and **value-equal to the unprojected `synods["Moon"]` record** when there is a previous Moon anchor (otherwise it must be `null`). Previously the Python wrapper silently reconstructed `new_moon`, concealing contradictory native transport results. The returned Python convenience alias remains the **same object** as the projected `synods["Moon"]`. The test-only native response fixture and API regression now cover missing, null, wrong-ID, unrelated Moon aliases and malformed `synods` containers. **9/9 focused tests passed with Python 3.13.5 on the exact committed Python wrapper and UTC conversion source (Git blob SHA checks)**; Rust was replaced by a test double, and the actual complete in-repository 50-test installed-wheel gate remains NOT RUN. Current minima: 17 API/time + 8 CLI + 16 CSV oracle + 9 compiled native = **50 mandatory non-skipped**, plus authorized real-data parity.

**Rust/Python dataset identity consistency (2026-11-12, PR #5):** `VirgoStarsEvents.open(..., expected_sha256=trusted_digest)` now retains the **canonical trusted digest** in its Python wrapper. Before projecting any native event rows, `get_synods_at` rejects a malformed native dataset envelope (missing/non-null P0 ID, format other than `VSEPH-P0`, non-false coverage certification or invalid/non-lowercase SHA-256), and rejects a well-formed SHA that does not match the trusted digest supplied to `open`. This prevents exposing plausible-looking events under a contradictory claimed archive identity; it does **not** replace the Rust SHA verification of the copied P0 bytes. Existing API regression methods include 12 additional negative subcases and assert canonical digest forwarding, with no new test methods. **12/12 focused Python 3.13.5 tests passed** using the exact GitHub Python wrapper blob `f985507f5a122d2bd48e72a35e7f6cbb4d5f7472`, the exact UTC helper and a simulated native reader. The full **50-case** installed-wheel/Rust acceptance and original owner CSV/P0 parity remain **NOT RUN**.


**M0 build gate ordering (2026-11-12, PR #5):** `py -3 tools\\build_python_mvp.py` now runs `tools/validate_python_sources.py` first (**42 non-skipped source-only tests**: 18 API/time + 8 CLI + 16 synthetic CSV oracle), **before** the locked Rust fmt/Clippy/test/release build. It only runs maturin and creates a wheel after both early gates pass, and only retains a wheel after isolated PyO3/installed-wheel acceptance (**50 non-skipped tests**, including 9 compiled native). Added two build-orchestration regression methods to prove Python-source/Cargo failures never reach wheel publication, while updating the happy-path and late-failure assertions for the new four-step order. Fixed the synthetic Python boundary fixture to set raw `new_moon: null` when it removes a preceding Moon anchor, matching the recently added fail-closed Rust-to-Python alias check. **4/4 isolated local sequencing tests passed** under Python 3.13.5; these reproduce gate order with stubs, **not** the exact GitHub test file or a real compiled wheel. Rust/Cargo/maturin are still unavailable here; no native build or original-data parity has run.

**Strict unittest outcomes (2026-11-12, PR #5):** The 42-case Python source gate and 51-case installed Rust/PyO3 wheel gate now both reject `unittest.expectedFailures` and `unexpectedSuccesses` in addition to skips, ordinary failures and incomplete test execution. Standard `unittest` misleadingly reports `OK (expected failures=1)` with `wasSuccessful() == True`, which is not sufficient for native acceptance. Four new test methods across the two separate orchestration test files cover these outcomes, including **real** temporary `@unittest.expectedFailure` unittest modules. A standalone Python 3.13.5 reproduction of the acceptance predicates against real unittest results passed **8/8 focused checks**, not the exact checked-out orchestration tests or a compiled wheel. The 41/50 mandatory counts remain unchanged.

**M0 per-body coverage integrity (2026-11-12, draft PR #5):** `VirgoStarsEvents.get_synods_at` now requires each native synod body to retain `coverage.status == "unreviewed"` and a list-valued `coverage.warnings` containing `no_reviewed_coverage_policy`. It also rejects non-object `phase` payloads before projecting UTC times. This blocks a mislabeled native response from silently asserting scientifically certified event coverage; additional factual gap warnings remain allowed. Existing Python API regression test now exercises six malformed payloads plus one permitted extra-warning case. **8/8 focused isolated Python 3.13.5 predicate tests passed**, not the full GitHub suite or compiled Rust. Concurrent work expanded authored API tests to 18, so the *current* acceptance minimum is **42 source-only** (18 API + 8 CLI + 16 independent oracle) and **51 real installed-wheel/native** (42 + 9 compiled P0 fixture). No owner data, M0 wheel or Rust compilation verified here.

**CSV parity probes at Python's UTC limits (2026-11-12, PR #5):** `tools/verify_python_synods.py::select_probes` now retains only representable UTC instants (inclusive years **0001–9999**). Valid original source anchors at `0001-01-01T00:00:00Z` or `9999-12-31T23:59:59.999999Z` must be checked exactly, without generating synthetic before/after instants in year 0000 or 10000 that `iso_utc` and the Python API cannot express. Existing CSV-oracle regression method now checks boundary preservation, pruning and exact microsecond UTC round-trip. **9/9 focused local Python 3.13.5 reproduction tests passed**, not a full checkout run; required acceptance remains **42 source-only + 9 compiled native = 51 non-skipped**, with independently authorized owner-data parity still required.

**Fail-closed nullable P0 fields in independent source parity (2026-11-12, PR #5):** `tools/verify_python_synods.py::assert_parity` now distinguishes **missing mandatory JSON fields** from explicit `null` values in every synod record and its phase, including `source_id`, current/next cycle anchors, phase thresholds, phase code and phase-start time. Earlier `dict.get(key) != expected` comparisons silently accepted omitted keys when the source oracle legitimately expected `None`, especially before the first recorded event. The public `new_moon` key is now mandatory even when its value must be null (no earlier Moon or no Moon family). Existing independent-oracle test methods include missing-key and valid-null cases; total stays **16 CSV oracle**, **42 Python source-only** and **51 installed native acceptance tests**. **12/12 focused Python 3.13.5 tests passed on an isolated reproduction** of the new comparator, NOT the complete checked-out test suite or compiled wheel. Scientific coverage remains unreviewed.
