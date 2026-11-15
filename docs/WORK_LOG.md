# Agent work log and handoffs

## 2026-11-15 — Entirely skipped local calendar day: proposed API error contract

- **Issue / branch:** [#21](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/21); `agent/chat-45a0e580/21-skipped-local-day-contract`, agent `chat-45a0e580`. Draft PR will remain unmerged.
- **Observed fact:** Python 3 stdlib `zoneinfo` with system IANA tzdb **2025b** independently confirms that `Pacific/Apia` skipped `2011-12-30` entirely when its UTC offset shifted from −10 to +14 hours. Normal neighboring days have UTC ranges `[2011-12-29T10:00Z, 2011-12-30T10:00Z)` and `[2011-12-30T10:00Z, 2011-12-31T10:00Z)`; they join without overlap.
- **Changes:** Explicit proposed `NonexistentLocalDate { localDate, timeZone }` error in `docs/API_CONTRACTS.md`, synthetic hand-reviewed JSON fixture `test/fixtures/skipped-local-day.json`, eight standard-library-only Node contract regressions `test/skipped-local-day.test.mjs`, validation gate in `docs/VALIDATION.md`. No timezone converter or event reader was implemented.
- **Verification:** System Python `zoneinfo` conversion and round trips executed; system tzdb identified from `/usr/share/zoneinfo/tzdata.zi`. Node v22.16.0 is available; committed Node test status is recorded accurately in the PR, and a full-checkout integration/real timezone engine test remains pending.
- **Risks / next:** Keep project implementation status *proposed*; reconcile with separate Europe/Rome contract PR #22 after it lands, run actual standard-library fixture tests and then cross-language Rust/WASM/Python boundary parity when implementations exist. No proprietary astronomical data, production integration, publication, release or merge.


This is the shared state for agents. Add a dated brief entry **in each working draft PR**, link issues and PRs, and update the roadmap only when completed. Avoid duplicate PRs by checking GitHub before starting.

## 2026-11-10 — repository initialization

- **Baseline:** public repository empty; first README created, CPAL-1.0 licensing and project/agent docs initialized.
- **Implemented:** documentation only.
- **Not implemented:** Rust workspace, VSEPH parser/file, exporter, real dataset, JavaScript bindings, Python bindings, migration adapter or releases.
- **Checks available:** GitHub file/write verification and document review only. No runtime test suite exists yet; cargo, node, python tests **not run**.
- **Reported data baseline:** synods 70,554; motion 266,516; ingresses 43,774; total 380,844, not audited.
- **Seed GitHub issues:** [#1 — synthetic fixtures](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/1); [#2 — Rust workspace](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/2); [#3 — VSEPH format](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/3).
- **Next recommended task:** P0.1 synthetic fixture contract, then P0.2 minimal Rust workspace. Keep dataset unpublished pending owner choice of license.
- **Open question for maintainer:** what separate open-data license should govern published dataset binaries? Decision is required before first data release, **not** before local code work.

## Template for future entries

### YYYY-MM-DD — <short task>
- **Issue/PR/branch:**
- **Facts observed / exact source:**
- **Code/docs/fixtures changed:**
- **Commands run and results (or not run):**
- **Unresolved risks / blockers:**
- **Next smallest safe task:**
