# Agent working agreement — read before every task

Applies to all human contributors and AI agents in the entire repository. The goal is **concrete, small, verified progress** on a standalone open-source event-ephemeris library. An agent should always be able to find the current state and next safe task without private chat history.

## Required reading, in order

1. This file, `README.md`, `LICENSE` and `NOTICE`.
2. `docs/ARCHITECTURE.md`, `docs/ADR-0001-RUST-BINDINGS.md`, `docs/VSEPH_FORMAT.md`, `docs/API_CONTRACTS.md`.
3. `docs/BACKEND_COMPATIBILITY.md`, `docs/DATA_PROVENANCE.md`, `docs/VALIDATION.md`.
4. `docs/ROADMAP.md`, `docs/WORK_LOG.md` and any relevant issue/PR already in progress.
5. Recent commits, open issues, open draft PRs and project configuration; check whether another agent is working on the same deliverable.

**Current status (2026-11-10):** the main branch initially contains docs and license only; [draft PR #4](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/4) contains synthetic fixture tooling and [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) contains the initial typed Rust workspace. Neither is merged. PR #5 now contains an **in-memory recorded-event index** (not a binary reader); dataset PR #6 separately contains the published-on-branch 2026-11-10 scientific snapshot, experimental Python reader and draft coverage policy. No file-backed **Rust** VSEPH reader, exporter or bindings exist. Never infer functionality from design examples or an unmerged PR.

## Task selection for each run

1. Inspect the repo, issues, PRs, branches, tests, and roadmap status first.
2. Prefer an unblocked `P0` item in `docs/ROADMAP.md` whose scope is not already claimed. If a PR exists, extend that branch rather than create a competing PR.
3. State a bounded deliverable with acceptance criteria (e.g., parser of a specific header field plus failure tests). Do not take on a broad untestable rewrite.
4. Implement code **with tests, fixtures and docs** as relevant. If only docs exist, select the smallest preparatory work necessary for the first executable milestone; do not keep drafting plans indefinitely.
5. Validate locally or with available tools. Log exact commands and results; if tools cannot run tests, explicitly say **not run**, never “passed”.
6. Push to a dedicated branch; open/update one **draft PR** with changed files, tests, blockers and next step. Update `docs/WORK_LOG.md` and the applicable roadmap checkbox **only when the acceptance criteria are actually met**. Avoid duplicate PRs.
7. Do not deploy, publish packages, tag production releases, mutate live DB tables, merge to main, or enable/trigger GitHub Actions unless explicitly approved. The project intentionally avoids GitHub Actions as a substitute for locally verified work.

## Non-negotiable boundaries

- **Standalone runtime:** no PostgreSQL, HTTP service, license server, astronomy kernel or dependence on another VirgoStars repository for the reader.
- **Authoritative Rust core:** same decoder/query semantics exposed to Rust, JS/TS (WASM) and Python (PyO3); no parallel astronomical or index algorithms per language.
- **Reader vs exporter:** read-only PostgreSQL export is an optional development tool. Use parameterized queries, repeatable snapshot/transaction, limited credentials and streaming. Never write to the source database.
- **No astronomic recomputation** of stations, synods or ingresses in the reader. No copying Swiss Ephemeris or third-party proprietary source.
- **Dataset ownership:** maintainer reports the source data were generated independently with their algorithms; this statement is not independent verification. Do not invent provenance, licenses, ranges or precision. No full dataset, secrets, private dumps, connection strings, personal information, Telegram content, copyrighted material or snapshots in commits without explicit release clearance.
- **Licensing:** code is CPAL-1.0 including completed attribution Exhibits A/B. Add the Exhibit A notice to each new source file (or clear, discoverable equivalent when syntax prevents it). Maintain NOTICE and `docs/LICENSING_AND_CREDITS.md`. Dataset license is **not selected**; do not apply CPAL to exported binary data by assumption.
- **Integrity first:** never silently round timestamps, invent missing events, interpolate an unverified event, silently replace an out-of-range result, or treat a missing phase as a successful full cycle. `EventIndex.at`/navigation returns **recorded evidence only**, never proof of a verified sign/direction state across data gaps. Check SHA-bound coverage and reviewer evidence before any state is labeled verified.
- **Compatibility:** preserve legacy requirements deliberately, not legacy bugs. The concrete use-case map is in `docs/BACKEND_COMPATIBILITY.md`.
- **Time:** UTC/POSIX timestamps, explicit offsets for ISO instants; document PostgreSQL timestamp semantics and local-date DST behavior. No host-local-time defaults in core.
- **Performance:** profile before adding a compression/dependency layer; only decode requested indexed chunks and cap memory on corrupt inputs.

## Definition of done

- Behavior documented in the public API specification and tested on synthetic fixtures (past/future boundary, same-instant equality, duplicate input, empty dataset, null extras, broken checksum/truncation).
- Deterministic output and stable ordering across calls and languages.
- No source DB mutation; no imported secrets, sensitive data or unrelated product code.
- Tests/lints/builds run and results disclosed; doc links remain valid.
- Changes are reviewable, small, accessible, traceable through an open draft PR and `docs/WORK_LOG.md`.

## Progress reporting template

Each agent handoff/PR must include: `task / issue`, `base commit and branch`, `scope and files`, `facts observed`, `implementation`, `validation commands / outcomes`, `known risks or data gaps`, `next smallest task`, `PR URL`. Mark assertions as confirmed, planned or blocked. Never invent a record count beyond the explicitly dated baseline.

## Priority queue

Use `docs/ROADMAP.md` for exact acceptance criteria and `docs/WORK_LOG.md` for latest handoffs. Start with P0 fixture schema + strict Rust query model, followed by minimal file parsing and edge-case tests. Data export requires authorized database access; without it, work on synthetic fixtures and validation tooling, not fake real data.
