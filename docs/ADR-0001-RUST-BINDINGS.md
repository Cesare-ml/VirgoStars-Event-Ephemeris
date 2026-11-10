# ADR-0001 — Rust reader and thin cross-language bindings

**Status:** Accepted architectural direction; implementation pending.  
**Date:** 2026-11-10.

## Decision

Use Rust as the **only source of truth** for parsing, validating, indexing and querying VSEPH. The same algorithm must underpin:
- native Rust (crate API);
- JavaScript and TypeScript (WASM via wasm-bindgen, declarations and ergonomics layer);
- Python (PyO3 extension, maturin wheels, typed Python facade).

Do not build three competing decoders. WASM avoids shipping a Node-specific binary initially and allows browser use; benchmark its loading cost and memory before deciding on an optional napi-rs Node addon. For Python, ship wheels for supported platforms; source builds may require a Rust toolchain.

## Important boundaries

This is **not** the VirgoStars-Ephemeris astronomical Rust engine. It performs **no astronomical interpolation or recalculation** and does not need NASA/JPL or Swiss files. The only runtime input is a versioned precomputed event dataset. Optional PostgreSQL export is a separate **development tool**, not in the runtime dependency graph.

## Alternatives considered

- TypeScript-only: easiest for Node/Angular but creates a separate Python implementation, JS Date range/precision hazards, and makes portable low-level parsing harder to unify.
- Python-only: excellent data tooling but not a natural browser engine.
- SQLite as shipped dataset: portable and capable, but adds another query engine/possible WASM weight and is less tailored to a tiny immutable 3-family file.
- Flat CSV/JSON: human readable, but larger, slower to load, expensive to index and more ambiguous about time precision.

These are not prohibited export/debug formats. VSEPH is the canonical **distribution format** once experimentally validated.

## Consequences

Rust compiler and FFI packaging complexity; require cross-language identical fixtures and typed errors. Shared core eliminates decoder drift. Future languages (C ABI / Java) remain possible without altering the file format.

## Revisit criteria

Revisit if measured browser bundle/load size, memory or performance fails published acceptance targets. Do not optimize prematurely; document benchmarks and alternative trade-offs in a new ADR.
