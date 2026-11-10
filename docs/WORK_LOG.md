# Agent work log and handoffs

This is the shared state for agents. Add a dated brief entry **in each working draft PR**, link issues and PRs, and update the roadmap only when completed. Avoid duplicate PRs by checking GitHub before starting.

## 2026-11-10 — Whole-file SHA-256 verification and record-count overflow hardening (draft PR #5)

- **PR/branch:** [draft #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), `feat/p0-2-rust-workspace`; no competing PR, no merge or changes to the dataset in [PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6).
- **Implementation:** `crates/events-p0/src/lib.rs` now uses checked addition across family totals (malformed header cannot cause unchecked `sum()` overflow); exposes `archive_sha256()` (bounded-memory streaming file digest) and `verify_archive_sha256(expected)` (strict lowercase 64-digit hex and full-file comparison). The SHA-256 helper is reused by `crates/events-p0/src/coverage.rs`; previously the policy adapter duplicated hashing logic.
- **CLI:** `crates/events-p0/src/bin/p0-verify.rs` accepts `--sha256 <trusted-manifest-digest>` and returns nonzero when it differs; JSON now includes the computed `sha256` and `sha256_checked_against_expected`. Without `--sha256`, the digest is informational only. Archive-wide block integrity checking remains `verify_all()`; neither form proves the dates have complete astronomical coverage.
- **Tests:** three **new Rust golden tests written, NOT RUN**: matching whole-file digest, CLI correct digest, wrong/malformed CLI digest. Existing Rust golden CLI test now checks JSON SHA fields. Documented in `docs/DEVELOPMENT.md`, `docs/P0_BYTE_AUDIT.md`, `docs/REVIEWED_COVERAGE_RUST.md`, `AGENTS.md`, `README.md`.
- **Executed evidence:** `VSEPH_P0_PATH=/mnt/data/virgostars-events-20261110.vseph-p0 python -m unittest discover -s /mnt/data/local_rust/tools -p test_audit_p0_bytes.py -v`: **8/8 PASS**; independent Python full-block auditor with trusted digest `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`: **761/761 blocks, 380,844/380,844 records**, matching whole-file SHA-256. These validate Python and the actual dataset, not Rust runtime behavior.
- **Blocked verification:** no `rustc`/`cargo`; `curl https://static.rust-lang.org/...` fails DNS. Cargo fmt/clippy/test/release build **NOT RUN**. The new Rust code and CLI remain experimental, PR draft; do not merge as validated.
- **Next:** execute full Cargo gates in a Rust-equipped checkout, fix compile/format issues, run CLI with `--sha256` on the reassembled PR #6 dataset, then continue byte-level VSEPH v1 format work (issue #3). No Actions, deploy, release or source data changes.

## 2026-11-10 — Rust P0 exhaustive verification and CLI (draft PR #5)

- **PR/branch:** [#5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), `feat/p0-2-rust-workspace`; no new PR or merge.
- **Code:** `crates/events-p0/src/lib.rs` adds `P0AuditSummary` and `P0Archive::verify_all()`, processing each compressed block with exact decoded per-family counts, without interpreting data gaps as completeness. `crates/events-p0/src/bin/p0-verify.rs` provides an offline JSON-producing CLI with error exit code. No stable v1 format changes.
- **Tests added (Rust, NOT RUN):** four new `golden_p0.rs` cases: full synthetic archive counts, corrupt nonqueried block, successful CLI JSON, CLI failure on corrupt archive. The ignored real-snapshot test now reads all 761 compressed blocks / 380,844 records and verifies section totals in Rust when executed in a combined PR checkout. `tools/test_audit_p0_bytes.py` adds a matching independently executable Python test for corruption outside a targeted query.
- **Actual validation:** `VSEPH_P0_PATH=/mnt/data/virgostars-events-20261110.vseph-p0 python -m unittest discover -s /mnt/data/local_rust/tools -p test_audit_p0_bytes.py -v` → **8/8 PASS**. `python .../audit_p0_bytes.py .../virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e` → **761 blocks and 380,844 events, SHA valid**. Independent Python test confirmed Mercury targeted query still succeeds with an unrelated ingress block corrupt, while full validation rejects the file.
- **Unverified:** no `cargo`/`rustc` installed. Attempted system package index refresh failed to complete in time. The new Rust methods and CLI have **NOT been compiled or executed**; no Rust/Python runtime parity claim is made. Continue to block merge until Cargo fmt, clippy, tests and release build pass.
- **Next:** run Cargo checks in a Rust-equipped checkout; fix any compile/style errors; compare Rust `verify_all()` against the Python reference and manifest; then continue VSEPH v1 and reviewed-coverage work. No CI Actions, deploy, data edits or merge.

## 2026-11-10 — Independent full-block P0 integrity audit (draft PR #5)

- **Existing PR/branch:** [#5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), `feat/p0-2-rust-workspace`; no competing PR and no changes to the separate public snapshot on PR #6.
- **Implementation:** `tools/audit_p0_bytes.py` independently decodes **all** P0 zlib blocks with strict limits and checks source ID varints, delta timestamps, phases, direction, sign, index ordering, header/section counts, block/file SHA-256; `tools/test_audit_p0_bytes.py` has 7 tests (one optional against real data). New `docs/P0_BYTE_AUDIT.md` includes reproducible commands and PowerShell instructions.
- **Verification really executed:** in a local mirror of the exact file tree, `VSEPH_P0_PATH=/mnt/data/virgostars-events-20261110.vseph-p0 python -m unittest discover -s tools -p test_audit_p0_bytes.py -v`: **7/7 PASS**, including malicious/truncated tests; `python tools/audit_p0_bytes.py /mnt/data/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`: **761 blocks / 380,844 records / all hashes PASS**; independently generated 1470-byte golden fixture **4 blocks / 8 events PASS**. Matched Git blobs of committed Python source with executed local files.
- **Toolchain limitations:** `rustc` and `cargo` were not found; GitHub clone DNS unavailable. **Cargo fmt, clippy, test, release build remain NOT RUN.** Python byte audit validates file framing, **not** the Rust reader's compilation, cross-language runtime parity, or astronomical completeness.
- **Next:** run Rust gates on a Rust-equipped checkout, compare actual Rust decoded events with Python reference against the full snapshot, improve the normative VSEPH v1 format, and preserve fail-closed reviewed coverage.
- **No merge, deploy, GitHub Actions, package publication, or dataset modification.**

## 2026-11-10 — Rust fail-closed state coverage policy (draft PR #5)

- **Task/branch/PR:** P0.6 preliminary Rust coverage semantics; `feat/p0-2-rust-workspace`, [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5). Existing PR #6 already contains the same experimental policy contract in Python and the scientific archive, so no competing PR created.
- **Implementation:** `crates/events-core/src/coverage.rs` shared `CoveragePolicy`, `ReviewedWindow`, `CoverageAssessment`; explicit family/body [start,end) windows, nonempty human reviewer/evidence, no overlap. `crates/events-p0/src/coverage.rs` loads the JSON `virgostars-reviewed-coverage/v0` policy, verifies SHA-256 of the entire archive, prevents a mutated binary from inheriting a verified policy, and exposes `CoverageAwareP0.sign_at` / `direction_at` with `UnknownCoverage`, `UnknownInitialState` and `Reviewed`.
- **Code/tests/docs:** 5 new core unit tests plus 7 P0 integration tests against the independently Python-generated P0 golden fixture; `docs/REVIEWED_COVERAGE_RUST.md` and this log. Tests cover pre-1970 moments, inclusive/exclusive bounds, previous-window state rejection, overlapping/invalid review metadata, independent motion/ingress coverage, wrong SHA, mutated archive and zero-window policy.
- **Actual validation executed:** `cd /mnt/data && python -m unittest -v test_vseph_prototype`: **6/6 PASS**. Inspected original local binary SHA-256 `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`, file size 4,143,385 bytes, sections summing to 380,844, 761 blocks. A raw Moon ingress query for year 3000 returned its last **recorded** event in 2099; this demonstrates why fail-closed state classification is necessary.
- **Not run:** Rust `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build` — the container has no Rust compiler and GitHub cannot be cloned via DNS. Tests are written, NOT executed; no Rust compilation or Rust/Python behavioral parity is claimed.
- **Known limitations:** policy contents are asserted by a reviewer, not independently proven by software. Real dataset policy on PR #6 still has zero approved intervals. Rehashing whole P0 file per covered query is correct but not optimized. Existing PR #5 is not merge-ready; PR #6 still separate; code and data licenses tracked separately.
- **Next:** run Cargo checks on a real Rust checkout, fix any compiler/clippy/format issues, exercise the 2026-11-10 zero-window policy against the actual archive, then finalize a stable VSEPH v1 reader. No deploy, merge, data modifications or GitHub Actions.

## 2026-11-10 — P0 binary decoder in Rust (draft PR #5)

- **Branch/PR:** feat/p0-2-rust-workspace / [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5). Existing PR reused; dataset and Python reference remain on separate PR #6.
- **Implementation:** new isolated crates/events-p0/ Rust crate with bounded VSEPH-P0 JSON-header/index parser, independently verified zlib blocks with SHA-256, varint decoder, exact i64 timestamps, source IDs, and recorded-event queries at/next/previous/between/synod_at.
- **Fixture:** independent Python-built 1470-byte, 8-event, four-block golden file; SHA-256 4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499; seven active Rust tests and one explicitly ignored real-snapshot integration test defined, NOT executed with Cargo.
- **Executed checks:** Python P0 reference reader verified all expected golden fixture records/query outcomes and integrity, and six existing Python synthetic tests passed. Real binary header shows 380,844 records, 761 indexed blocks, 4,143,385 bytes.
- **Not executed:** cargo fmt, cargo clippy, cargo test, cargo build; rustc/cargo unavailable. No Rust runtime parity or successful compilation is claimed.
- **Scientific boundary:** P0 reader produces recorded events only. The separate SHA-bound coverage policy in PR #6 is not yet ported to Rust; verified sign/direction state must not be inferred from the last record.
- **Next:** execute standard Cargo gates, correct compile/style issues, run ignored real-data integration test on combined checkout, then define stable VSEPH v1 format and move reviewed coverage semantics into Rust.
- **No merge, deployment, GitHub Actions, production changes or dataset modification.**

## 2026-11-10 — Indexed Rust queries and synod navigation (draft PR #5)

- **Existing PR / branch:** [#5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), `feat/p0-2-rust-workspace`; extended rather than opening another Rust PR. PR #6 covers the Python experimental binary reader and SHA-bound coverage policy and remains separate.
- **Implementation:** new `crates/events-core/src/indexed.rs`, exposed from `lib.rs`. In-memory immutable grouped index; deterministic per-family/body ordering; reject same-series/same-instant duplicate input. Implement recorded-event `at`, strict `next`/`previous`, [start,end) `between`; `synod_at` returning a handle supporting `event`, `next`, `previous` and `next_recorded_start`.
- **Scientific safety:** these APIs explicitly return **recorded events, not proven sign/motion states**. No inference across missing periods. Production-grade coverage state requires reviewed windows tied to the dataset digest.
- **Tests added (not executed):** six new Rust unit tests and two external integration tests covering strict bounds, pre-epoch timestamp, empty/missing series, duplicate rejection, cross-family same-time, same-body synod navigation and gap awareness. Existing 13 tests unchanged.
- **Checks performed:** GitHub connector readback and structural checks (file presence, public re-export, test declarations). No Cargo toolchain on authoring container; `cargo fmt/clippy/test/build` **NOT RUN**, and no compilation claim made. Standard Cargo verification remains mandatory before merge.
- **Next:** run Cargo checks and fix any compile/style errors; freeze VSEPH v1 header/index (#3), implement file decoder, and move SHA-bound coverage semantics from Python into Rust before exposing verified states in bindings.
- **No merge, deploy, package release, data change or GitHub Actions**.

## 2026-11-10 — P0.2 Rust workspace (draft PR #5)

- **Issue / PR / branch:** [#2](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/2) / [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5) / `feat/p0-2-rust-workspace` (branch from `main`; no merge).
- **Existing work check:** [draft PR #4](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/4) already addresses P0.1 synthetic JS fixtures. P0.2 does not copy or publish them.
- **Implementation:** dependency-free Rust workspace; `virgostars-events-format` candidate magic probe only; `virgostars-events-core` strongly typed body/timestamp/sign/direction, synod phase order/null semantics, motion and ingress models; unified event enum. Format/parser indexing and actual `next()` remain out of scope.
- **Files:** `Cargo.toml`, `crates/events-core/{Cargo.toml,src/lib.rs,tests/public_api.rs}`, `crates/events-format/{Cargo.toml,src/lib.rs}`, `docs/DEVELOPMENT.md`, `AGENTS.md`, this log and roadmap.
- **Checks NOT RUN:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release`; Cargo/rustc are absent from authoring environment. Test cases have been written but not executed.
- **Source verification:** GitHub file fetch/readback and manifest/source inspection performed; these are NOT a substitute for compilation.
- **Blockers:** no Rust toolchain in current environment; run standard checks before PR is eligible for review/merge.
- **Next step:** issue #3 byte-level VSEPH format/golden bytes after review, or iterate on any Cargo issues. Do not claim P0.2 complete, deploy or merge without verification and approval.

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
