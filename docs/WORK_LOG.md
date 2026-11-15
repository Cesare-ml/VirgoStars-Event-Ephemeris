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

## 2026-11-15 — P1 local-day UTC/DST golden fixtures (issue #20)

- **Issue/branch:** [#20](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/20), `agent/chat-8f2d9c4a/20-dst-local-day-goldens`, base `main`; agent session `chat-8f2d9c4a`.
- **Scope:** `test/fixtures/local-day-utc-boundaries.json` (seven independently sourced UTC intervals), `test/local-day-utc-boundaries.test.mjs` (eight offline deterministic tests), and concise updates to the proposed API/validation contracts.
- **Evidence:** Independently calculated each bound with Python 3 `zoneinfo` and the environment's system IANA timezone database: Europe/Rome spring-forward 2026-03-29 is 82,800 seconds; fall-back 2026-10-25 is 90,000 seconds. Source data are *calendar-time fixtures only*, not released astronomical events or a new timezone algorithm.
- **Executed:** Node.js v22.16.0 local mirror: `node --check test/local-day-utc-boundaries.test.mjs` **PASS**; `node --test test/local-day-utc-boundaries.test.mjs` **8 passed, 0 failed**. Test logic and seven fixture records were copied into the GitHub branch via the GitHub connector. A full checkout command was **not** run; independent review remains required.
- **Risk/blocker:** UTC bounds reflect current 2026 IANA rules and may need refresh if jurisdictional policies change. No `onDate` implementation, VSEPH dataset, native Rust engine, event query parity or production integration is claimed.
- **Next:** independent exact-head reviewer validates source/fixture readback; a future actual `onDate` adapter must be tested against these golden values; owner decides integration.

## Template for future entries

### YYYY-MM-DD — <short task>
- **Issue/PR/branch:**
- **Facts observed / exact source:**
- **Code/docs/fixtures changed:**
- **Commands run and results (or not run):**
- **Unresolved risks / blockers:**
- **Next smallest safe task:**
