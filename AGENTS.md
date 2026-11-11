# Agent working agreement — read before every task

Applies to all human contributors and AI agents in the entire repository. The goal is **concrete, small, verified progress** on a standalone open-source event-ephemeris library. An agent should always be able to find the current state and next safe task without private chat history.

## Required reading, in order

1. This file, `README.md`, `LICENSE` and `NOTICE`.
2. `docs/ARCHITECTURE.md`, `docs/ADR-0001-RUST-BINDINGS.md`, `docs/VSEPH_FORMAT.md`, `docs/API_CONTRACTS.md`.
3. `docs/BACKEND_COMPATIBILITY.md`, `docs/DATA_PROVENANCE.md`, `docs/VALIDATION.md`.
4. `docs/ROADMAP.md`, `docs/WORK_LOG.md` and any relevant issue/PR already in progress.
5. Recent commits, open issues, open draft PRs and project configuration; check whether another agent is working on the same deliverable.

**Current status (2026-11-10):** the main branch initially contains docs and license only; [draft PR #4](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/4) contains synthetic fixture tooling and [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) contains the initial typed Rust workspace. Neither is merged. PR #5 contains an **in-memory recorded-event index**, a file-backed **experimental Rust VSEPH-P0** decoder, and a newly added SHA-256-bound Rust coverage adapter. Dataset PR #6 separately contains the 2026-11-10 scientific snapshot, experimental Python reader and reference coverage policy. The Rust VSEPH v1 format, compilation, and parity tests remain unverified. No exporter or stable bindings exist. Never infer functionality from design examples or an unmerged PR.

## User-prioritized first deliverable (2026-11-11)

**Python FIRST, not JS/WASM first.** The maintainer requires an actually usable, easily pip-installable Python MVP accepting an offset-aware instant X and returning the **most recent recorded synod for each body**, **last recorded Moon new moon** and **current cycle stage** when safely derivable from stored phases. Follow [PYTHON_FIRST_MVP.md](docs/PYTHON_FIRST_MVP.md) and [DELIVERY_MILESTONES.md](docs/DELIVERY_MILESTONES.md), with M0 first. Explicit user story and acceptance checklist are binding for future agent work.

- First validate the current Rust/P0 branch against synthetic and authorized real-file golden data. Then implement batched synod anchor/phase semantics **in Rust** (not an independent Python scientific port).
- Build a minimal PyO3/maturin package, locally test `pip install .` and `pip install -e .`, create/install a Windows wheel in a fresh venv and verify `VirgoStarsEvents.open(...).get_synods_at(...)` end-to-end. Require `python tools/validate_python_mvp.py` after installation: **5 synthetic native fixture tests must actually run**, not be skipped; optional owner-data smoke uses `--archive` and trusted `--sha256`. Do not treat `unittest OK (skipped=N)` as passing native integration. When the owner-approved normalized `sinodi.csv` and separate P0 file are available, additionally run `python tools/validate_python_mvp.py --archive <path> --sha256 <trusted-digest> --synods-csv <authorized-csv>`: check source IDs and original phase thresholds against the real Rust/Python responses. This is a **read-only validation oracle**, never an independent Python astronomy engine.
- Must preserve source row IDs, UTC microseconds, SQL `NULL` vs `[]`, exact phase threshold equality, dataset identity and known gap warnings. Do not claim scientifically verified phase/current state solely from historical recorded data. Moon synod anchors represent recorded new moon according to the source convention, not newly calculated events.
- No PyPI publication, permanent dataset bundling or automatic download until dataset distribution rights are explicitly approved. Package source follows CPAL-1.0 with visible VirgoStars credits.
- Add regression tests and update README, ROADMAP and WORK_LOG for every small milestone. Avoid competing PRs; existing draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) houses Rust/Python work. The user has NOT asked to merge or deploy.
- **Documentation of a planned API is not code.** Current PyO3 binding source and Python `get_synods_at()` wrapper have been committed, but have NOT been compiled together, installed via pip or smoke-tested against a native wheel. Eight pure-Python mock-binding tests passed; no working package may be claimed yet. Never describe their examples as executable until built and tested.

## Task selection for each run

1. Inspect the repo, issues, PRs, branches, tests, and roadmap status first.
2. Prefer an unblocked `P0` item in `docs/ROADMAP.md` whose scope is not already claimed. If a PR exists, extend that branch rather than create a competing PR.
3. State a bounded deliverable with acceptance criteria (e.g., parser of a specific header field plus failure tests). Do not take on a broad untestable rewrite.
4. Implement code **with tests, fixtures and docs** as relevant. If only docs exist, select the smallest preparatory work necessary for the first executable milestone; do not keep drafting plans indefinitely.
5. Validate locally or with available tools. Before proposing a merge, run `python tools/validate_rust.py` (or on Windows `py -3 tools\\validate_rust.py`) from the Rust branch to execute Cargo fmt, Clippy, tests and release build. For an assembled authorized VSEPH-P0 snapshot, pass `--archive <path> --sha256 <trusted-manifest-digest>` to require independent Python/Rust JSON summary parity. See `docs/LOCAL_RUST_CHECKS.md`. Log exact commands and results; if tools cannot run tests, explicitly say **not run**, never “passed”.
6. Push to a dedicated branch; open/update one **draft PR** with changed files, tests, blockers and next step. Update `docs/WORK_LOG.md` and the applicable roadmap checkbox **only when the acceptance criteria are actually met**. Avoid duplicate PRs.
7. Do not deploy, publish packages, tag production releases, mutate live DB tables, merge to main, or enable/trigger GitHub Actions unless explicitly approved. The project intentionally avoids GitHub Actions as a substitute for locally verified work.

## Non-negotiable boundaries

- **Standalone runtime:** no PostgreSQL, HTTP service, license server, astronomy kernel or dependence on another VirgoStars repository for the reader.
- **Authoritative Rust core:** same decoder/query semantics exposed to Rust, JS/TS (WASM) and Python (PyO3); no parallel astronomical or index algorithms per language.
- **Reader vs exporter:** read-only PostgreSQL export is an optional development tool. Use parameterized queries, repeatable snapshot/transaction, limited credentials and streaming. Never write to the source database.
- **No astronomic recomputation** of stations, synods or ingresses in the reader. No copying Swiss Ephemeris or third-party proprietary source.
- **Dataset ownership:** maintainer reports the source data were generated independently with their algorithms; this statement is not independent verification. Do not invent provenance, licenses, ranges or precision. No full dataset, secrets, private dumps, connection strings, personal information, Telegram content, copyrighted material or snapshots in commits without explicit release clearance.
- **Licensing:** code is CPAL-1.0 including completed attribution Exhibits A/B. Add the Exhibit A notice to each new source file (or clear, discoverable equivalent when syntax prevents it). Maintain NOTICE and `docs/LICENSING_AND_CREDITS.md`. Dataset license is **not selected**; do not apply CPAL to exported binary data by assumption.
- **Integrity first:** never silently round timestamps, invent missing events, interpolate an unverified event, silently replace an out-of-range result, or treat a missing phase as a successful full cycle. **Rust coverage safety:** `CoverageAwareP0` only treats a state as reviewed when a SHA-bound policy names an independently approved [start,end) interval and a recorded event exists inside it. The 2026-11-10 policy has ZERO approved real-data windows; do not infer signs/directions across gaps. See `docs/REVIEWED_COVERAGE_RUST.md`. `EventIndex.at`/navigation returns **recorded evidence only**, never proof of a verified sign/direction state across data gaps. Check SHA-bound coverage and reviewer evidence before any state is labeled verified.
- **Compatibility:** preserve legacy requirements deliberately, not legacy bugs. The concrete use-case map is in `docs/BACKEND_COMPATIBILITY.md`.
- **Time:** UTC/POSIX timestamps, explicit offsets for ISO instants; document PostgreSQL timestamp semantics and local-date DST behavior. No host-local-time defaults in core.
- **Chronological paging on the Rust index and P0 file reader:** experimental `EventIndex::chronological_between` (borrowed in-memory events) and `P0Archive::chronological_between` (owned compressed-record events with original source IDs) share stable timestamp/family/body ordering, half-open intervals, bounded pages and cursors bound to both interval and live reader instance (not a persistent dataset identity). Test both suites (`chronological_public_api`, `chronological_p0`), run full Cargo fmt/Clippy/tests/release build and verify real-data parity before claiming P0.7 complete; **these new commits have not yet been compiled**. Do not infer an event or a scientifically verified sign/direction across historical gaps. See `docs/P0_CHRONOLOGICAL_PAGING.md`.
- **PostgreSQL source identity:** each original row ID must be positive and globally unique within its source family/table, including across bodies and compressed blocks; IDs may repeat across different families. Require the full-block scan `P0Archive::verify_all()` before declaring an archive valid, as targeted `at/next/previous` lookups do not see duplicates in unrelated blocks. The independent Python test suite `tools/test_audit_p0_bytes.py` must also pass (13 tests). No silent deduplication of source IDs, timestamps or invented replacements.
- **CSV↔P0 scientific parity:** if the owner-approved snapshot, normalized CSV, exporter or compressed P0 encoding changes, run `python -m unittest discover -s tools -p test_verify_csv_p0.py -v` and the full `tools/verify_csv_p0.py <normalized-csv-folder> <assembled-archive> --sha256 <trusted-manifest-digest>` before requesting review. Confirm **all** IDs, timestamps, phase arrays, signs and motion direction are unchanged; report missing records as data gaps, never fabricate replacements. See `docs/CSV_P0_FIELD_PARITY.md`.
- **Exhaustive P0 validation:** when editing compressed block parsing, check both the targeted query path and the independent full-archive path. `P0Archive::verify_all()` and the `p0-verify` CLI are provisional Rust implementations; `tools/audit_p0_bytes.py` is a separately executable Python reference. Neither establishes that missing astronomical dates have been computed. Run both on a single immutable snapshot and require `p0-verify <file> --sha256 <trusted-manifest-digest>` to compare the entire file to a versioned manifest before approving data integration. The SHA-256 is not a proof that any uncovered astronomical interval is complete. **Cargo remains a required gate and has not been run in this authoring environment.**
- **Performance:** profile before adding a compression/dependency layer; only decode requested indexed chunks and cap memory on corrupt inputs. For VSEPH-P0 changes, run `python -m unittest discover -s tools -p test_audit_p0_bytes.py -v` and optionally the full-byte audit in `docs/P0_BYTE_AUDIT.md` before Cargo tests. Python success does NOT establish successful Rust compilation. The isolated P0 adapter uses bounded zlib decompression and per-block hashes, not a certified production v1 decoder.

## Definition of done

- Behavior documented in the public API specification and tested on synthetic fixtures (past/future boundary, same-instant equality, duplicate input, empty dataset, null extras, broken checksum/truncation).
- Deterministic output and stable ordering across calls and languages.
- No source DB mutation; no imported secrets, sensitive data or unrelated product code.
- Tests/lints/builds run and results disclosed; doc links remain valid.
- Changes are reviewable, small, accessible, traceable through an open draft PR and `docs/WORK_LOG.md`.

## Progress reporting template

Each agent handoff/PR must include: `task / issue`, `base commit and branch`, `scope and files`, `facts observed`, `implementation`, `validation commands / outcomes`, `known risks or data gaps`, `next smallest task`, `PR URL`. Mark assertions as confirmed, planned or blocked. Never invent a record count beyond the explicitly dated baseline.

## Priority queue

Use `docs/ROADMAP.md` for exact acceptance criteria and `docs/WORK_LOG.md` for latest handoffs. Follow the **M0 Python-first MVP** in `docs/ROADMAP.md` and `docs/PYTHON_FIRST_MVP.md`: validate the existing experimental Rust reader, implement latest synods/new-moon/phase in Rust, then a pip-installable Python PyO3/maturin binding with a real Windows smoke test. After M0, progress toward full backend query parity, scientific coverage review, VSEPH v1 and JS/WASM. Data export requires authorized database access; without it, work on synthetic fixtures and validation tooling, not fake real data.
