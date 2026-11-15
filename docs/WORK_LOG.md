# Agent work log and handoffs

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

## 2026-11-15 — fixed non-hour IANA and 30-minute DST local-day fixtures (agent chat-7b2d91ea)

- **Issue/branch:** [#23](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/23), [PR #26](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/26), `agent/chat-7b2d91ea/23-nonhour-local-days` → `main`; independent review pending.
- **Facts:** `docs/API_CONTRACTS.md` proposes half-open UTC intervals for local `onDate` queries, but `main` has no callable `onDate` implementation. Existing parallel PR #22 covers Rome 23/25-hour days; this work does not touch its files.
- **Added:** `test/fixtures/non-hour-zone-local-days.json` (five independent reference cases: Kathmandu, two Chatham dates, Lord Howe autumn/spring), `test/non-hour-zone-local-days.test.mjs` (nine deterministic Node golden/mutation checks), and this validation documentation.
- **Executed:** Python 3 `zoneinfo` independently checked all five specified zone/day UTC intervals on 2026-11-15. On Node.js v22.16.0, `node --check test/non-hour-zone-local-days.test.mjs` → exit 0; `node --test test/non-hour-zone-local-days.test.mjs` → **9 passed, 0 failed, exit 0** in an isolated exact-blob mirror. Exact Git blob IDs: test `ad1e23558873edbd5d550d4c56a1133f594909c3`; fixture `4e8a6b8bcef7c188e9182b0040381ec786d7130e`.
- **Not run:** full repo/npm/Cargo tests; actual runtime `onDate` (not implemented), device, database or production checks.
- **Next:** independent PR review, then compare eventual timezone wrapper outputs against these fixed references; never derive the test oracle using the same wrapper.
