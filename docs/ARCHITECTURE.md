# Architecture — standalone event ephemeris

**Status:** proposed design, not implemented. Date: 2026-11-10.

## Purpose and boundaries

Provide random access to precomputed astronomical/astrological events via an indexed portable file. This is a new **standalone** open-source library; runtime use must require **only the VSEPH dataset**, with no database, remote API, astronomical calculation engine, or sibling VirgoStars project.

The source PostgreSQL database remains owned/operated by the VirgoStars website. A separate **offline export workflow** reads a consistent snapshot to produce VSEPH. No back-write, migration or production DB changes are in scope.

## Layering

1. `events-format`: pure Rust VSEPH header/index/record parsing; explicit error and bounds checks; file validation; streaming for optional decode.
2. `events-core`: immutable event data model, per-family/by-body indices, search primitives, cyclic-navigation handles; no DB, networking or astronomical computation.
3. `events-exporter` (development-only feature/tool): PostgreSQL read-only ingestion, UTC normalization, validation, canonical sort/dedup rules and deterministic serialization plus manifest; no dependency from core to exporter.
4. `events-cli`: inspect, validate, list and benchmark datasets; tests rely on synthetic fixture samples.
5. `bindings/javascript`: Rust WASM shared core with JS/TS-friendly loading, TypeScript definitions, ISO timestamps and structured error types.
6. `bindings/python`: Rust PyO3/maturin binding, Python datetimes (aware UTC), iterator-friendly page/range APIs.
7. `adapters/virgostars` (later, optional): integration shim for old backend signatures and presentation enrichment; **not** part of the reader core.

## Core event model

- `Synod`: body ID, start instant, ordered zero-to-many named phases, end = next cycle start where known; for Moon, new/first-quarter/full/last-quarter; for supported planets d1/r2/r3/d4; no assumption that all families have exactly three extras.
- `MotionEvent`: body ID, instant, direction after station ('direct' or 'retrograde'), zodiac sign index 0..11 at event.
- `IngressEvent`: body ID, instant, entered sign index 0..11; identical sign can recur due to retrograde re-entry.
- Optional provenance fields belong to the dataset catalog/manifest; exclude Telegram IDs, descriptions, interpretations, messages and application-specific foreign keys.

A navigable `SynodHandle` contains an immutable reader reference plus (section, body, ordinal) locator; `next()/previous()` use index traversal rather than file scans. Cross-language handles must have the same semantics and not assume that a cycle ends at its final phase.

## Query semantics

- `at(body,t)`: last cycle start <= t for that body, subject to **confirmed coverage**; report out-of-range instead of fabricating a cycle.
- `next(body,t)`: first event strictly after t. `previous(body,t)`: last event strictly before t. An explicit `atOrBefore`/`atOrAfter` can cover inclusive boundaries.
- `between(start,end)` uses [start,end), chronological ordering plus stable tie breaks (family, body, record ordinal). Avoid unbounded implicit collections: provide iterators/chunking and optional limit.
- `onDate(date,timeZone)` maps a named local calendar day to real UTC bounds using an IANA time zone, including DST; time-zone support should be provided by a documented wrapper/dependency, not silent system locale behavior in core.
- State lookup relies on prior event and known valid coverage. `stateAt` of a planet returns **unknown/out-of-range** when the preceding station or coverage proof is absent. More than one lunar new moon may fall in a month; never index an arbitrary three-record selection as prev/current/next.

## Time and versioning

Persist absolute **signed 64-bit microseconds since Unix epoch**, in POSIX UTC notation, with explicit quality/time-scale metadata. PostgreSQL TIMESTAMPTZ and TIMESTAMP WITHOUT TIME ZONE must be normalized with a declared assumption and tested. Leap-second encoding cannot be inferred from PostgreSQL time alone. Do not claim TT/TDB positional accuracy or strict leap-second support from an event timestamp.

Version the **file format**, **source dataset snapshot**, **core API**, and **bindings** separately. File version major changes must fail with a typed error; append optional metadata in a backward-compatible way. Preserve dataset provenance and creation tool version in the external manifest.

## Memory/performance strategy

Index by event family + stable body ID + time chunk, with per-block start/end offsets and count. Decode only the requested block(s), optionally cache with bounded LRU. Start with delta-encoded integers and explicit bit flags; add Zstd only if benchmarks show net benefit on Node, browsers, Python and native Rust.

## Data and release responsibilities

Read [DATA_PROVENANCE.md](DATA_PROVENANCE.md), [VALIDATION.md](VALIDATION.md) and [LICENSING_AND_CREDITS.md](LICENSING_AND_CREDITS.md). The data author's ownership statement is recorded, but actual numerical fidelity, coverage and redistribution terms are not yet independently verified or set.
