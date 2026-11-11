# Delivery milestones — Python first, VirgoStars compatibility second

**Updated 2026-11-11.** Product delivery milestones for the standalone, offline VirgoStars Event Ephemeris. Detailed code-level P0/P1/P2/P3 tasks remain in [ROADMAP.md](ROADMAP.md). The user's first required deliverable is [PYTHON_FIRST_MVP.md](PYTHON_FIRST_MVP.md), **not** a full release of the VSEPH v1 format or a complete astronomical ephemeris. Neither historical completeness nor stable packaging may be inferred from 380,844 recorded rows.

## Delivery order and dependency graph

| Milestone | User-visible outcome | Dependencies / implementation work | Acceptance gate | Status |
| --- | --- | --- | --- | --- |
| **M0 — Python synod MVP** | Install a local wheel with pip; query an instant X; receive latest recorded synod per available body, last recorded Moon new moon and current cycle phase or explicit unknown | Rust P0 integrity and indexed queries; Rust aggregation/phase logic; PyO3/maturin module; Python API, wheel, tests, examples | [PYTHON_FIRST_MVP.md](PYTHON_FIRST_MVP.md) acceptance tests, actual fresh Windows install and owner-run smoke test | **FIRST PRIORITY — not implemented** |
| **M1 — Query parity for VirgoStars** | Python and backend can query current synods/batches, phases, direction/ingress states and lunar events for month/day without PostgreSQL for those reads | `atAll`, `stateAll`, `phaseAt`; Moon month/cycle functions; UTC and Europe/Rome calendar windows; explicit unknown coverage; adapter for legacy aliases; test against existing backend SQL | Controlled fixtures + read-only comparisons to backend at identical instants, bounds, DST transitions and known gaps; no production swap | Planned after M0 |
| **M2 — Scientific coverage and snapshot qualification** | Consumers know which *specific* family/body/date windows are reviewed, uncertain or missing | Issue [#7](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/7) lunar synod gaps; [#8](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/8) ingress gap / Aries-only tail; source algorithm/provenance; reviewed, SHA-bound coverage manifests; validated versioned data patches if authorized | Evidence-backed, independently reviewed intervals; unknown results remain unknown elsewhere; immutable original snapshot | Parallel track; gate for scientifically verified state, not necessarily for M0 recorded-event preview |
| **M3 — Stable VSEPH v1 and performance** | Deterministic versioned format, robust reader/exporter, supported snapshot migration | Byte layout and golden bytes ([issue #3](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/3)); streaming exporter and manifest; corruption/fuzz/overflow and cross-reader tests; performance benchmarks | Locked spec, reproducible authorized dataset roundtrip, full Rust/Python data parity, bounded memory and regression suite | Not yet started as stable format; experimental P0 exists |
| **M4 — Portable integrations** | Production-oriented Python wheels and optional JS/TS browser + Node API sharing one Rust core | Platform wheel matrix, versioned Python types, wasm-bindgen JS/TS, consistent error semantics, stable API and docs, backend adapter | Cross-language golden tests and real-date parity, compatibility checks, no duplicated astronomy algorithms | Python part begins in M0; JS/TS follows M1 |
| **M5 — Approved open-source distribution / migration** | Installable public package(s), versioned dataset, optional migration with fallback | Separate dataset license approval ([issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9)); NOTICE/credits; PyPI/npm/crates.io approvals; signed checksums, maintainer review, rollback and shadow mode in VirgoStars | Explicit author approval, all CI-equivalent gates **run locally** (no GitHub Actions), package installs, documented source/coverage status, production migration only after explicit authorization | Not approved |

## Concrete first five small work packages (execute in order)

1. **Cargo and real snapshot gate** — synchronize tested Windows checkout (preserve local rustfmt changes), compile the current Rust branch and run `py -3 tools/validate_rust.py --archive ... --sha256 ...` against a disposable, combined PR #5 + #6 snapshot. Diagnose any mismatch without touching either PR's historic data.
2. **Synod batch and phase kernel in Rust** — return a record keyed by *all* bodies present in the synod index, with anchor `<= X`, next anchor if available, original source ID and phase boundaries. Implement D1/R2/R3/D4 and Moon CRESC1/CRESC2/CAL1/CAL2 classifications on verified input ordering. Missing/NULL extras and suspicious gaps return explicit unknown data, not D1 by default.
3. **Minimal PyO3 bridge** — create a separate Python-binding crate/path; make `VirgoStarsEvents.open(path, expected_sha256=...)` and `get_synods_at(instant)` call **the same Rust engine**. Return JSON-compatible dictionaries; aware datetime / explicit-offset ISO supported; reject naive timestamps.
4. **Real local pip wheel** — add `pyproject.toml` (maturin), importable `virgostars_events`, a simple script, build Windows wheel and prove both `pip install .` (with Rust) and `pip install <wheel>` (without compiler at install time). Keep the dataset as an explicit local input; no auto-download and no public PyPI yet.
5. **Golden + backend parity acceptance** — synthetic and owner-approved real record tests for every phase threshold equality, latest new moon, before-first/no-next, null vs empty arrays, lunar 1919/2018 gaps, exact microseconds, UTC offset normalization and JSON output; obtain comparison to the legacy `sinodi` query. Write the real test log and review before marking M0 complete.

Each work package includes code, tests, documentation and an updated draft PR; work may be subdivided but not declared finished from static inspection alone. Users should be able to try the local Python MVP **before** all historical astronomical gaps and the official VSEPH v1 design are solved, with clear coverage warnings.

## Backend parity list for M1 (order by user impact)

- `TabellaSinodo.getSinodoPerDataoraUtc` and `getSinodiPerDataoraUtc`: latest per body and `atAll`. First addressed in M0.
- `getMotiSinodiciPianetiPerDataoraUtc` and `legacySynodicMotionRepository.findSynodicMotionsAt`: current D1/R2/R3/D4 or Moon quarter-stage code from extra timestamps. First addressed in M0.
- `getSinodoLunaPrecCurrSucc`: correctly select **Moon only**, return all phases/cycles overlapping the month plus neighbors rather than assuming exactly three results; next M1.
- `TabellaMotoPianeti.getMotoPianetiPerData`, `getDirezioneMotoPianetaPerData`, `getDirezioneMotoPianetiPerData`: UTC instant state, batch state and civil-day Europe/Rome windows; M1 with explicit coverage.
- `TabellaIngressoSegni.getIngressiPerData`, `getIngressiLunaPerAnnoMese`: day/month event intervals and signAt with explicit coverage; M1.
- Calendar and chart controllers: unified chronological pages + separately emitted stored synod extras; integrate using read-only shadow comparisons, preserve Telegram metadata and interpretation in VirgoStars DB; M1–M4.

The complete source-to-target list and legacy bugs are documented in [BACKEND_COMPATIBILITY.md](BACKEND_COMPATIBILITY.md).

## Quality gates and release controls

- **Recorded event vs verified state:** the latest recorded synod or Moon new moon at/before X may be shown as a historical record, but not proof that no later event is missing. Coverage review status and known gap warnings must travel with the result. Scientific accuracy is not established merely by a matching file hash.
- **Single authoritative Rust implementation:** Python and JS/TS never independently recompute events or duplicate the binary decoder. The Rust source remains CPAL-1.0 with required **Powered by VirgoStars — https://www.virgostars.it/** attribution.
- **Install scope:** a private/developer `pip install .` or local wheel is a near-term milestone; a public `pip install virgostars-event-ephemeris` from PyPI is **not available until** an explicit approved public release. The scientific dataset has a separately undecided license and must not be bundled without clearance.
- **Git policy:** use existing draft PR #5 for Rust/Python architecture, PR #6 only for the unchanged owner-authorized data snapshot; do not trigger GitHub Actions, merge, deploy, publish or replace the backend automatically.
- **Validation:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release`, Python binding tests, fresh pip installation, independent full-dataset integrity and read-only cross-language/SQL parity. Record which commands were **actually run**.

## Current evidence / blockers

As of this update, the Rust workspace and experimental compressed P0 reader exist in **draft PR #5**, the owner snapshot (70,554 synods, 266,516 motions, 43,774 ingresses) in **draft PR #6**, and a basic user-reported Windows Cargo run passed on an earlier revision. The newest Rust changes and Python MVP are **not yet independently validated**; no PyO3 binding or pip wheel exists yet. Scientific coverage issues #7/#8 remain open and the dataset reuse license #9 remains undecided.

Do not use an elapsed-time estimate as acceptance evidence: milestone completion depends on the tested, installable functionality described above.
