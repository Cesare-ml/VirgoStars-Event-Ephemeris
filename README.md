# VirgoStars Event Ephemeris

**Independent, free/open-source, offline-first event ephemerides for astrology.**

**Status: EXPERIMENTAL RUST IMPLEMENTATION (draft PR #5).** This branch has an in-memory Rust event index and a separate experimental reader for the VSEPH-P0 archive. It does not yet contain a verified Rust VSEPH v1 reader, JS/TS bindings, a tested/installed Python wheel, release packages or production integration. Experimental PyO3 and Python binding **source** exists but is not yet proven to compile end-to-end. The historical dataset, binary and experimental Python reference are staged separately in [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), not merged into this branch. API examples for unimplemented methods remain design proposals.

## First product milestone: Python synods, Moon new moon and current cycle phase

**The first usable MVP is a locally pip-installable Python library**, ahead of WASM, a stable VSEPH v1 format, and production replacement. The planned API is:

```python
from virgostars_events import VirgoStarsEvents

events = VirgoStarsEvents.open("local-events.vseph-p0", expected_sha256="<trusted dataset SHA-256>")
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

Input validation now fails early on a malformed SHA-256, an impossible local DST datetime or a malformed CLI instant, without opening the archive on invalid input. The expanded **34-test** acceptance suite still requires a compiled PyO3 wheel; its native checks have **NOT RUN** on this branch.

A clean **offline wheel-installation gate** is now available after local `maturin build --release --out dist`: `py -3 tools\verify_python_wheel.py --wheel path\to\matching.whl`. It creates an isolated venv, installs only that wheel without an index, checks installed package/native imports and the offline CLI, then invokes the mandatory fail-closed Python MVP suite. Optional `--archive`, `--sha256`, and `--synods-csv` add authorized original-data comparisons. The gate's **7 orchestrator unit tests passed**, and an actual offline installation of a deliberately invalid wheel failed as expected because it contained a Python stub instead of a compiled Rust extension. It also checks the installed `virgostars-events` command. **No actual compiled native wheel has yet passed this gate**; see the [Windows build guide](docs/PYTHON_MVP_BUILD.md).

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
