# VirgoStars Event Ephemeris

**Independent, free/open-source, offline-first event ephemerides for astrology.**

**Status: EXPERIMENTAL RUST IMPLEMENTATION (draft PR #5).** This branch has an in-memory Rust event index and a separate experimental reader for the VSEPH-P0 archive. It does not yet contain a verified Rust VSEPH v1 reader, JS/TS or Python bindings, release packages or production integration. The historical dataset, binary and experimental Python reference are staged separately in [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), not merged into this branch. API examples for unimplemented methods remain design proposals.

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
