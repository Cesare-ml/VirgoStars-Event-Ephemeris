# VSEPH file format — design specification

**STATUS: DRAFT v0, NOT FROZEN AND NOT IMPLEMENTED.** This document defines invariants and a candidate layout. It is **not** a claim that any file was produced.

## Goals

A deterministic immutable compact file for ~380,844 baseline events in three families; constant-time index lookup by family/body and logarithmic time-block selection; no DB or astronomy computation at runtime; language-neutral parsing; truncated/corrupt input rejected without arbitrary allocations.

## Candidate logical layout

| Region | Contents |
| --- | --- |
| Header | 5-byte ASCII magic `VSEPH`, format major/minor, byte order (little endian), flags, header length, offsets, total size |
| Catalog | Stable body IDs/names and family IDs; event schema version; coverage ranges; time scale and source snapshot identifiers |
| Index | Sorted by (family, body, time-block first timestamp); record count, first/last timestamp, byte offset, compressed/uncompressed length, checksum |
| Blocks | timestamp base + delta-coded times; body implied by index; packed direction/sign for motions and ingresses; synod phase count + ordered phase offsets |
| Trailer / manifest reference | Dataset digest, optional checksum tree and metadata location; sidecar JSON for human-readable provenance and release checks |

**Encoding details to prototype/freeze before v1:** concrete header byte offsets, checksum algorithm and scope, block-length maximum, varint encoding, zigzag for signed deltas, handling of unknown optional fields, locale-independent body-ID registry, compression codec ID, and version/feature negotiation. No normative byte-level parser may ship until these are fixed and golden fixtures committed.

## Types and precision

- Timestamp: signed `i64` number of **microseconds since 1970-01-01T00:00:00Z** in Unix/POSIX UTC convention. Historical instants before 1970 are negative. No floating-point storage, no loss of precision from JavaScript `Number`; expose ISO UTC text / BigInt or safe date conversions as appropriate.
- Synod records: one anchor timestamp plus 0..N ordered named phase offsets or absolute instants. For Moon 3 standard extras when present, but do not hardcode count for all bodies. Preserve null vs no phases as metadata/contract until source export rules are decided.
- Motion records: direction code + zodiac sign (0 Aries ... 11 Pisces) + timestamp, tied to a body.
- Ingress records: entered zodiac sign (0..11) + timestamp and body; retrograde re-entries are valid.
- Interpretation, Telegram notification IDs and source DB surrogate IDs are excluded from core scientific records. Optional legacy ID map belongs in a separately versioned adapter artifact if required.
- Scientific/temporal provenance: dataset catalog must declare time scale, assumption for timestamp-without-time-zone source, algorithm/method ID, and confidence/precision not greater than justified.

## Indexing, navigation and compatibility

Canonical ordering by family, body ID, time ascending, with deterministic same-instant tie behavior and no silent data loss. Group into bounded independently readable blocks (initial experiment: around 256 records/block); include direct body range table to avoid global scans. Handles may refer to body ordinal and decode adjacent blocks for `next/previous`.

Decoder MUST validate header, version, count, offsets, overflow, bounds, ordering, checksum and allowed values before exposing records. Unknown mandatory features fail closed; unknown optional metadata may be skipped by declared length.

Do not serialize file-system paths or live PostgreSQL identifiers into the file.

## Date and completeness caveats

`sinodi.dataora` is TIMESTAMPTZ (actual instant); `moto_pianeti.dataora` and `ingresso_segni.dataora` are TIMESTAMP WITHOUT TIME ZONE (conventionally UTC in current backend). Export must label and validate this assumption explicitly, with near-DST regression samples. Neither DB type proves subsecond precision or astronomical time scale. An index range alone does not prove there are no gaps; manifest must carry coverage quality/known gaps by body/family.

## Compression experiment

Benchmark at least:
1. Fixed-width timestamps (simple baseline).
2. Per-body delta + unsigned varints and packed enums.
3. Same delta encoding with optional per-block Zstd (only if WASM/wheel costs are acceptable).

Report compressed file bytes, peak memory, load/startup cost, random lookups (cold/warm), interval iteration and binary build sizes. Do not promise a specific dataset size before measuring a real authorized export.

## Manifest (proposed)

A standalone human-readable `dataset.manifest.json` should include: dataset ID/version, author statement, generator version, SHA-256 of VSEPH, source snapshot reference (non-secret), per-section/body count/min/max UTC, missing/gap reports, timestamp convention, known scientific method description, license identifier/URL and release approval. No author statement or placeholder licenses should be silently promoted to release-certified facts.

## First implementation checkpoint

Create a documented **tiny synthetic** file with all three families, one pre-1970 time, one null/empty phase list, one body with sparse coverage, multiple entries on the same date and one corrupted/truncated variant. Implement a reader and exact round-trip tests **before exporting real records**.
