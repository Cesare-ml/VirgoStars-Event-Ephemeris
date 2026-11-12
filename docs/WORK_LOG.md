# Agent work log and handoffs

This is the shared state for agents. Add a dated brief entry **in each working draft PR**, link issues and PRs, and update the roadmap only when completed. Avoid duplicate PRs by checking GitHub before starting.

## 2026-11-12 — bounded policy JSON and Python/Rust parser alignment (draft PR #6)

- **Existing PR / branch:** [#6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), `data/2026-11-10-snapshot`. Same Python reference prototype; no overlapping branch, merge, or dataset edits.
- **Code:** `prototype/coverage_policy.py` now reads at most 1,000,001 policy bytes (maximum accepted: 1,000,000), caps reviewed windows at 4,096 as in provisional Rust P0, rejects Unicode/non-ASCII decimal timestamps, and fails on duplicate JSON object keys at any nesting depth.
- **Tests:** six synthetic standard-library cases added to `prototype/test_coverage_identity.py`: policy over-size, exact boundary, excess windows, non-ASCII microseconds, duplicate top-level key and duplicate nested key. Existing six identity tests are unchanged.
- **Execution:** local `python -m py_compile coverage_policy.py test_coverage_identity.py` → **PASS**; `python -m unittest discover -s . -p 'test_*.py' -v` from the isolated two-file directory on Python 3.13.5 → **12/12 PASS**. Local `git hash-object` returned `76451e886b9f0ffb249c7130910629587beaa068` and `b90cb6f507d0431d0e6c0b61db1443ad2845e2cd`, exactly matching the GitHub commit blobs after upload.
- **Not run:** the full P0 packer/reader integration suite or Rust Cargo tests: direct GitHub checkout failed due DNS resolution in this execution container; the 12 tests use a synthetic fake file-backed reader, not a real dataset decoder.
- **Known parity gap:** Rust provisional parser already has the size/window caps and ASCII timestamp parsing, but uses `serde_json::Value` which does not reject duplicate keys like the new Python parser. Carry this into Rust parity tests before merge.
- **Next:** enforce strict duplicate-key handling in Rust; run full Python prototype/real archive integrity and Rust gate tests on a machine with repo checkout and Cargo. Original 2026-11-10 coverage policy still has **zero reviewed intervals**. No actions/deployment/production changes.

## 2026-11-12 — bind coverage reader to immutable archive identity (draft PR #6)

- **Scope / existing PR:** [#6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), branch `data/2026-11-10-snapshot`; focused extension to the experimental `prototype/coverage_policy.py`, not a competing PR.
- **Observed defect:** `CoveragePolicy.load(policy, archive_A)` verified archive A once, but `CoverageAwareReader(reader_B, policy)` did not attest the reader file. A different or modified P0 archive could thus be paired with reviewed windows.
- **Fix:** retain the reviewed archive SHA-256 in the policy; verify the reader's real file at attachment and both before and after each state query to catch swapped or modified bytes and guard cached blocks. Missing file-backed reader path fails closed. The underlying recorded-event reader remains unchanged.
- **Tests:** new `prototype/test_coverage_identity.py` with six fully synthetic standard-library tests: matching archive and window boundary, wrong attached archive, modified archive after attach, modification during `at`, unreviewed lookup mutation, missing reader path. **Executed** isolated Python 3.13.5 reproduction of exactly these source/test contents: `python -m unittest discover -s /mnt/data/vseph-coverage-check -p 'test_*.py' -v` → **6/6 PASS**; `python -m py_compile ...` → **PASS**. These run with a fake file-backed reader, not the actual P0 decoder.
- **Not run:** complete repo `prototype/test_coverage_policy.py` and binary P0 integration from a cloned checkout; GitHub checkout cannot be cloned from this environment. No Cargo/Rust or scientific completeness validation claimed. **GitHub readback verified:** the committed `coverage_policy.py` and `test_coverage_identity.py` Git blob SHA-1 values (`e30f29c4cd3be4e2f63d521a7a007b4636569d28` and `e4c94756f250b37e8009ebb9ea894a2b32eef2a4`) exactly match `git hash-object` of the two tested local files.
- **Cost / risk:** streaming archive SHA twice per state query; this is deliberately conservative and not a substitute for immutable file-descriptor-backed snapshots or genuine scientific coverage audits. Zero human-reviewed windows remain. No dataset files, manifest, live database, release or production code changed.
- **Next:** run the three Python prototype suites on a fresh checkout and integrate the equivalent snapshot-identity contract in the authoritative Rust/PyO3 reader when Cargo gates pass. No GitHub Actions, merge, deploy or publication.

## 2026-11-10 — fail-closed coverage safety on dataset draft PR #6

- **Existing PR / branch:** [#6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), `data/2026-11-10-snapshot`; no duplicate PR.
- **Change:** new `prototype/coverage_policy.py` with `CoveragePolicy` and `CoverageAwareReader`; `prototype/coverage-policy-20261110.json` bound to real P0 SHA-256, deliberately **zero approved intervals**; `prototype/test_coverage_policy.py` and `docs/COVERAGE_SAFETY.md`.
- **Validated:** 7/7 synthetic tests passed with Python 3.13 (policy hash mismatch, empty intervals, boundary inclusivity, unknown initial state, body separation, invalid review evidence/overlap). Existing 6/6 P0 tests passed. Also loaded the actual 2026-11-10 4.14MB archive, verified policy SHA and showed Moon 3000, Moon 2099, Mercury 2026 return `unknown_coverage` instead of false state. GitHub copies of the policy source and test file matched local Git blob SHA-1 exactly.
- **Not claimed:** proof of scientific completeness, valid windows for real dates, Rust VSEPH v1 support, or a fresh GitHub checkout execution.
- **Next:** independently review coverage intervals (issues #7/#8); add Rust and cross-language parity for fail-closed states after P0.2/P0.3; decide data license (#9). No merge, package publication, deploy or GitHub Actions.

## 2026-11-10 — owner-authorized scientific dataset import (draft)

- **Scope:** 2026-11-10 pgAdmin export supplied by maintainer; public release of scientific fields explicitly requested; 70,554 synods, 266,516 motion events, 43,774 sign ingresses.
- **Prepared:** normalized headered CSVs, experimental indexed VSEPH-P0 binary, SHA-256 manifests, per-year event counts and 3 heuristic long-interval candidates (not proof of missing events). Original application-only fields are excluded.
- **Integrity:** all 380,844 normalized event records reproduce exactly the canonical scientific digests of the original exports. Compressed P0 SHA-256: `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`.
- **Storage:** some large files need ordered `*.parts/part-NNN` blobs because the GitHub connector rejects single multi-megabyte transfers. `tools/assemble_dataset.py` must verify byte length, SHA-256 and record counts on checkout.
- **Scientific gaps:** sparse historical coverage depends on body/family; Moon synods have candidate 1918–1920 and 2017–2019 long intervals; Moon ingresses 2099–4836 disjoint, then Aries-only records.
- **Licensing:** code CPAL-1.0; **separate scientific data license still pending selection**, no invented unrestricted rights.
- **Verified:** 13 Git blob SHA-1 identifiers recomputed from local source partitions matched GitHub responses; branch commit [6333232](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/commit/63332329fc0161d0d747328630c859dcd789c0f9) contains all 13 data paths. A **local simulation of the exact multipart checkout layout** passed reconstruction of all six tracked files with byte sizes, SHA-256, CSV headers and row counts; the Python P0 reader successfully queried Mercury from the reassembled archive. The **exact published assembler and unit test files** have Git blob SHA-1 hashes equal to the locally executed source; the three new assembler unit tests passed using a locally simulated Git checkout layout, and all six prototype synthetic tests passed. The P0 Python packer, indexed reader and test files were also uploaded to `prototype/` as exact Git SHA-1 matches of their tested local originals. **Not run on a fresh GitHub checkout**, so PR #6 stays draft.
- **Coverage/metadata issues:** [#7 lunar synod long intervals](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/7), [#8 lunar ingress discontinuity](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/8), [#9 data license](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9).
- **Next tasks:** rerun assembler and Python tests after fresh GitHub checkout, select data license, then design Rust VSEPH v1 and reproducible gap-completion workflow.
- **No merge, package publication, production integration or GitHub Actions**.

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
