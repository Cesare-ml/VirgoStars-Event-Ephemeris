# VirgoStars Event Ephemeris

**Independent, free/open-source, offline-first event ephemerides for astrology.**

**Status: INITIAL IMPLEMENTATION / PUBLIC DATA SNAPSHOT DRAFT.** The dataset owner has authorized [a public snapshot of three scientific event tables](datasets/2026-11-10/README.md); its large files may be stored as exact, SHA-256 verified parts (see [reassembly instructions](docs/DATASET_UPLOAD.md)). The compressed archive is **VSEPH-P0 experimental**, not the still-unimplemented VSEPH v1 Rust reader. [Draft PR #4](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/4) and [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) contain prototype fixtures and a Rust workspace, still subject to validation. JS/TS and Python bindings have not been released.

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

The project maintainer states that the source dataset is **entirely their original work generated using their own algorithms**. This is a recorded authorship statement, not an independent audit. **The owner has requested publication of an immutable scientific event snapshot, but a separate dataset redistribution/reuse license has not yet been selected**; the source-code license must not be presumed to license the published binary or CSV datasets. Document the exact dataset license before the first data release.

The **original source code** is licensed under [CPAL-1.0](LICENSE), following [VirgoStars-Ephemeris](https://github.com/Cesare-ml/VirgoStars-Ephemeris): completed Exhibits A/B include the original developer's attribution:
 
> Copyright (c) 2026 Cesare-ml (VirgoStars project)  
> Powered by VirgoStars  
> https://www.virgostars.it/

See the full license, especially `14 (GUI launch/session attribution, including Larger Works as specified) and `15 (network use). An About/Credits link is good practice but does not automatically satisfy every applicable display condition. Code remains free to use, study, modify and redistribute **subject to the license**; do not interpret “free” as a waiver of legal notices.

## What does not belong here

No secret credentials, exported private DB copies, telemetry, interpretive horoscope text, Telegram metadata, notification queues, GUI artwork, non-approved third-party data, or astronomical calculation algorithms. Keep optional PostgreSQL access isolated to an exporter used **only during dataset generation**.

## Development status and first deliverable

**Next milestone:** approve public API, freeze a first **draft** format specification, build tiny synthetic fixtures, write an executable Rust reader prototype with strict error handling and tests, then implement a read-only exporter/validator that can prove exact parity on an *authorized* snapshot. Development code and a source-data snapshot may be available in draft PRs; the official VSEPH v1 parser and cross-language bindings are not yet implemented.

For work opportunities and acceptance criteria, use [the roadmap](docs/ROADMAP.md); do not claim feature completion until code, tests and documentation are merged.

## Data snapshot and coverage limitations

See [snapshot 2026-11-10](datasets/2026-11-10/README.md), its [manifest](datasets/2026-11-10/manifest-publication.json), [candidate gaps](datasets/2026-11-10/candidate_large_intervals.csv), and [reassembly/verification guide](docs/DATASET_UPLOAD.md). The compressed P0 reader does **not** extrapolate missing events and its latest-sign/state lookups cannot establish continuity across unknown gaps. In particular the Moon ingress series ends in 2099 and resumes with Aries-only events in 4836. Existing data can still be queried for well-covered bodies and dates, subject to coverage checks.
