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

## 2026-11-15 — synthetic onDate microsecond membership around Rome DST

- **Issue / branch:** [#24](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/24) · `agent/chat-c4d91e7a/24-rome-dst-microsecond-membership` · session `chat-c4d91e7a`; draft PR linked in issue/PR once opened.
- **Source facts:** [API contract](API_CONTRACTS.md) defines `onDate` on a local IANA calendar date as a UTC half-open range. Separate [PR #22](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/22) provides independent Europe/Rome 2026 UTC bounds. No real `onDate` adapter is shipped on `main` in this work.
- **Changed:** new synthetic `test/fixtures/local-day-event-membership.json` (20 invented motion/ingress instants across 6 local dates), `test/local-day-event-membership.test.mjs` (9 Node tests), and this validation/work log. Checks include inclusive starts, exclusive ends, adjacent-day uniqueness, 23h/25h intervals, exact ±1 µs neighbors and malformed fixture rejection; timestamps are represented with integer `BigInt` microseconds.
- **Executed on Node.js v22.16.0:** `node --check test/local-day-event-membership.test.mjs` **passed**; `node --test test/local-day-event-membership.test.mjs` **9 passed, 0 failed** in an isolated local directory. Exact Git blob SHAs of fixture and test were compared against the GitHub branch. **Not run:** complete repository test suite, Cargo/Rust, WASM/Python, an actual `onDate` wrapper or any real/private dataset.
- **Open risks:** historic/future IANA timezone rules may change; tzdb version was not pinned. Same-instant cross-family event ordering remains a separate contract decision and is intentionally omitted from this fixture. Reconcile shared docs with PR #22 before any integration.
- **Next:** execute the real adapter on the fixture when its UTC-range/IANA wrapper exists, compare IDs exactly and rerun native cross-language acceptance tests. No dataset license, merge, publication, GitHub Actions or production deployment here.
