# Roadmap and agent task queue

**Updated:** 2026-11-11. **Status:** development in draft PRs #4/#5/#6; no changes have been merged to main. Rust/Python experimental code and an owner-authorized dataset exist on separate branches. This is the authoritative work queue. Tasks are intentionally small, ordered and verifiable.

## Working conventions

- `[ ]` not done; `[x]` only if merged and verified; `[~]` only if in progress with PR link documented in WORK_LOG.
- Agent: read AGENTS.md and **all open PRs/issues** before selecting a task; avoid concurrent duplicate changes.
- Every deliverable includes relevant code, tests, docs, validation outcomes and a *draft PR*; do not deploy/merge without explicit authorization.
- No GitHub Actions automation. Run explicit checks locally or through available tools.

## M0 — FIRST user-facing deliverable: installable Python synod MVP

**Deliver this BEFORE JS/TS WASM, production backend migration or frozen VSEPH v1.** Full acceptance criteria and proposed dictionary API: [Python-first MVP](PYTHON_FIRST_MVP.md). End-to-end delivery order including post-MVP features: [Delivery milestones M0–M5](DELIVERY_MILESTONES.md).

User story: from a Python installation, pass a timezone-aware date/instant X to `VirgoStarsEvents.get_synods_at(X)`; receive the **latest recorded synod start <= X for each available celestial body**, the **last recorded new moon** as a convenience field, and the **current Moon/planetary cycle phase** only when original three phase boundaries justify it. Include source IDs, cycle anchor timestamps, dataset identity and missing/coverage-warning status. **Do not** infer missing cycles or misrepresent the latest recorded event as scientific completeness.

- [ ] **M0.0 — Current Rust and authorized snapshot validated.** Finish current Cargo fmt/Clippy/tests/release on Windows, independent `p0-verify` / Python audit using exact SHA-256 and all 380,844 records on a disposable combined PR checkout.
- [~] **M0.1 — Rust aggregated synod query and phase classification ([PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Batch last-anchor `<= X` by all indexed synod bodies; Moon new moon and D1/R2/R3/D4 / CRESC1/CRESC2/CAL1/CAL2 threshold handling, NULL vs empty/partial extras, exact equality, missing coverage.
- [~] **M0.2 — Rust-backed Python interface ([PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Add dedicated PyO3/maturin binding; planned import `virgostars_events` and methods `VirgoStarsEvents.open(local_archive, expected_sha256=...)`, `get_synods_at(offset_aware_instant)`, returning a JSON-serializable dictionary. Rust stays the only data reader.
- [~] **M0.3 — Local pip installation and Windows wheel ([PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Implement `pyproject.toml`, prove `pip install .`, `pip install -e .`, create/install a local compiled wheel in a fresh venv, no compiler needed to install the wheel itself. Archive supplied separately, no automatic download.
- [~] **M0.4 — Python acceptance and SQL parity ([PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Unit, boundary/phase and real-source golden tests; compare `TabellaSinodo.getSinodiPerDataoraUtc` and `getMotiSinodiciPianetiPerDataoraUtc`; show an executable 10-line example; obtain maintainer smoke-test on Windows. **First usable MVP DONE only when all boxes pass.**

**Progress 2026-11-11:** implemented a Rust recorded synod phase module (5 unrun Rust tests), `P0Archive::synods_at` (4 unrun Rust tests), PyO3 extension source, `pyproject.toml`, UTC-safe Python wrapper and an example. **8/8 actual pure-Python unit tests PASS** against bytes matching committed GitHub blobs. **No Rust build, pip/wheel installation, native Python smoke or SQL parity has run on these new commits**. See [Windows Python MVP build guide](PYTHON_MVP_BUILD.md). New Rust chrono tests need a fresh Cargo run; the separate dataset reuse license is unresolved. Local developer pip installs are permitted without pretending that a public PyPI release is approved. Keep M0 in progress only once corresponding implementation/tests exist; mark boxes completed only after real validation.

## P0 — first executable and reproducible core

- [ ] **P0.1 ([issue #1](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/1)) — Define synthetic fixture contract.** Add schema and six tiny text fixtures for three families (pre-1970, sparse, null and variable extra phases, duplicate/malformed, DST-relevant UTC instants). Review date/id/phase semantics. **Done when:** deterministic fixture generator and tests for fixture parsing/validation exist.
- [~] **P0.2 ([issue #2](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/2), [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)) — Initialize Rust workspace.** Minimal `events-core` / `events-format` with library-only dependencies, explicit CPAL notices, no DB or astronomy engine. **Done when:** `cargo fmt --check`, `cargo clippy` and `cargo test` pass.
- [ ] **P0.3 ([issue #3](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/3)) — Freeze VSEPH v1 byte layout** following `VSEPH_FORMAT.md`. Define offsets, endian, varint, limits, checksums, format version, stable body registry and golden bytes. **Done when:** a byte-by-byte spec and hand-authored golden header/invalid-header tests exist.
- [ ] **P0.4 — Reader header/index MVP (experimental VSEPH-P0 Rust reader in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); v1 reader still pending).** Open synthetic VSEPH bytes/files with overflow and corruption tests and bounded memory. **Done when:** tests cover correct/unknown/truncated/corrupt header/index.
- [~] **P0.5 — Synod body queries/navigation (in-memory foundation in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Implement `at`, `next`, `previous`, `between`, optional phases with strict bounds and handle navigation. **Done when:** fixture parity and phase boundary tests pass.
- [~] **P0.6 — Motion + ingress queries (draft Rust coverage policy in [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); still unverified).** Implement date/range, latest-state, `signAt`, retrograde re-entries and no-coverage errors. **Done when:** fixture boundary/empty/gap tests pass.
- [~] **P0.7 — Unified event iterator ([draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5)).** Initial bounded chronological, cursor-paginated merge is implemented both for the immutable in-memory `EventIndex` **and the file-backed experimental `P0Archive`** (up to one 512-record decoded block per series; never materialize all 380,844 records). Cursors are bound to exact half-open range and process-local reader identity; all families share stable timestamp/family/body tie order. Seven in-memory + eight P0 Rust tests written **but not run on these new commits**. **Done when:** Cargo fmt, Clippy, all new tests and release build pass on the updated branch, and file-backed parity against a verified owner-approved snapshot is established.

## P1 — exporter, manifest and real-data validation

- [ ] **P1.1 — Source audit SQL (read-only).** Document queries producing counts per body, timestamp ranges, phase cardinality, duplicates, suspicious gaps and zone assumptions; test on synthetic PostgreSQL fixture. **Done when:** audit produces structured report without mutations.
- [ ] **P1.2 — Streaming export tool.** Explicit read-only connection, repeatable snapshot, canonical normalization, deterministic serialization and manifest digest. **Done when:** synthetic DB -> VSEPH -> decoded equality holds.
- [~] **P1.3 — Independent validator / CLI (experimental full-byte audit, source-ID uniqueness and CSV↔P0 field parity in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); v1 validator and Rust compile gate still pending).** Validate VSEPH headers/chunks/checksums, extract counts and sample events, report file and manifest integrity. **Done when:** bad file tests and golden round-trip pass.
- [ ] **P1.4 — Authenticated owner dataset audit.** With explicit access, audit reported 380,844-row baseline, bodies, coverage/holes and ownership terms. **Done when:** owner-reviewed report and zero unexplained loss. **Blocked:** authorized source export and dataset licensing decision.
- [ ] **P1.5 — Compression benchmarks.** Compare fixed-width vs varints vs block Zstd. **Done when:** size/latency/memory tradeoff documented with reproducible commands.

## P2 — language bindings and ecosystem (Python takes priority)

- [ ] **P2.1 — PyO3/maturin Python MVP first (M0).** Implement the minimum Rust-backed Python reader, latest synod/new-moon query and phases, local `pip install .` / editable build / Windows wheel before starting browser bindings. Later expand Python to full API with UTC date and time zones.
- [ ] **P2.2 — wasm-bindgen JS/TS after initial Python usage** with Node/browser loading and types; test shared Rust fixtures.
- [ ] **P2.3 — Cross-language parity harness** for every required API/query and error category.
- [ ] **P2.4 — Optional VirgoStars backend adapter**, read-only, with exact query compatibility and documented intentional fixes; **no production replacement** without review.
- [ ] **P2.5 — Documentation, CLI examples, benchmarks, memory/bundle-size limits** and release gates.

## P3 — publication (requires approval)

- [ ] **P3.1 — Decide and document separate dataset open-data license**, verify rights and author approval; update manifest/release docs.
- [ ] **P3.2 — Approved dataset release asset**, checksums, visible credits, version/tag and signed manifest.
- [ ] **P3.3 — Packages to crates.io, npm, PyPI**, only after full tests and maintainer approval.
- [ ] **P3.4 — Stable v1 public contracts** with compatibility guarantees and migration notes.

## Agent assignment suggestions

- Rust format/index agent: P0.2–P0.4.
- Rust query/API agent: P0.5–P0.7.
- Data quality/export agent: P1.1–P1.3, synthetic tests only until authorized.
- JS/WASM agent: P2.2 after successful installable Python MVP and shared Rust types settle.
- Python agent: M0.1–M0.4 / P2.1 immediately after Rust snapshot gate (first user-facing priority).
- Backend parity agent: P2.3–P2.4, no production changes.

**Never mark a task done from a draft, speculation or a successful generation of docs alone.** Link PR and tests in WORK_LOG.
