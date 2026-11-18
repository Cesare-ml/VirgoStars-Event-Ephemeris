# Agent work log and handoffs

## 2026-11-18 — Windows cross-volume link containment

- A real Windows symlink from a temporary checkout on C: to a directory on
  D: was incorrectly accepted: `path.relative` returns an absolute path for
  different volumes. The checker now rejects that result before resolving it.
- Added a portable `path.win32` regression for a different drive, an outside
  sibling and a valid descendant. All 17 Node tests pass on Windows, the
  checker passes on this checkout, and the real C:-to-D: symlink probe now
  rejects the outside target. This is an offline checker change only.

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

## 2026-11-15 — offline Markdown link validator (agent chat-78b40c6e)

- **Issue/PR/branch:** [issue #17](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/17), [draft PR #18](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/18), `agent/chat-78b40c6e/17-check-markdown-links` → `main`.
- **Facts observed:** Gate A in [VALIDATION.md](VALIDATION.md) requires consistent local links; `main` has no dedicated offline Markdown-link checker (checked before implementing). No network or source dataset access needed.
- **Code/tests:** Added `tools/check-local-markdown-links.mjs` (root/doc tree, local references, missing targets, line numbers) and `test/doc-links.test.mjs` (seven isolated synthetic fixture cases). No astronomical or VSEPH-format changes.
- **Executed locally:** Node.js `v22.16.0`; `node --test test/doc-links.test.mjs` → 7 passed, 0 failed; `node tools/check-local-markdown-links.mjs` → exit 0, `Repository-local Markdown links: OK`. Both ran against a **self-contained local mirror of the new files**, not a complete GitHub repository checkout. The latter result does **not** establish that all existing repository documentation links are valid.
- **Additional verification (2026-11-15, session chat-78b40c6e):** locally re-ran `node --check tools/check-local-markdown-links.mjs`, `node --check test/doc-links.test.mjs`, `node --test test/doc-links.test.mjs` (7 passed, 0 failed) and `node tools/check-local-markdown-links.mjs` (exit 0) on the self-contained mirror. Independently fetched the PR-head GitHub tree and all **17** root/docs Markdown files, checked resolved local target paths against that tree: **zero missing targets detected**. This was a separate remote tree/content audit, **not** the CLI against a complete checkout.
- **Not run:** CLI across the complete repository checkout (outbound GitHub host unavailable in local container); no Rust, GitHub Actions or cross-language checks.
- **Risks and next:** This is not a CommonMark AST/fragment validator. In a full checkout of draft PR #18, rerun both Node commands and fix any reported links before final review. No merge or deploy.

### 2026-11-15 — reviewer-requested quoted/indented code regression fix

- **Reviewer finding:** Independent PR #18 review at head `2718d0527e6e803fc63ffeb1445a6d8e8f25ea2d` reported `AGENT-REVIEW ... verdict=changes`: the existing scanner treats Markdown links in blockquoted fenced code and 4-space-indented code as real references.
- **Change on the same agent-owned branch:** `stripBlockquoteMarkers` now normalizes `> ` containers before fence parsing, tracks fence-container depth, and skips 4+-space or tab-indented code lines while preserving real links and original file line numbers. No new dependencies or astronomical behavior. Three focused regressions added to `test/doc-links.test.mjs` (now **10 authored test cases**).
- **Post-fix exact-blob validation:** Recreated the **complete committed** `tools/check-local-markdown-links.mjs` and `test/doc-links.test.mjs` in an isolated local mirror; `git hash-object` outputs **`233cb86eab4cd23a283ba57fc66e7785b2f198ac`** and **`13a726bda03ac5aa26995090c8b312f4152b0a11`**, matching the GitHub blob SHAs on this PR. On Node v22.16.0, `node --check` on both exact files **passed**, `node --test test/doc-links.test.mjs` **10/10 passed, 0 failed** (including CLI exit-code fixtures), and `node tools/check-local-markdown-links.mjs` **exited 0** on the two-file mirror.
- **Post-fix remote repository audit:** Independently fetched the latest PR-head tree and all **17** root/`docs/**` Markdown files via GitHub. The updated quoted-fence/indented-code parsing and tree-path audit examined **38** relative link targets and found **0** missing targets. This is a tree/content link audit, **not** execution of the CLI on the complete checkout.
- **Still not run:** Full-checkout `node tools/check-local-markdown-links.mjs` or real symlink-target checks; the local execution container cannot resolve GitHub. The previous independent `verdict=changes` applies only to the prior commit, and a fresh independent review is still needed.
- **Separate follow-up:** [issue #19](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/19) tracks symlink-escape hardening, which is intentionally not bundled in this small quoted-code regression fix. No data, production, workflow, release or merge changes.

### 2026-11-15 — security follow-up for symlink escapes (#19)

- **Issue/review:** [#19](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/19) and independent `AGENT-REVIEW verdict=changes` at historical PR #18 heads `ed03b3253d7eabdb3c5bce6e035db7f67e65d302` and `3ebd9edea662ae0d373246d7be61fb80b430df17` identified a lexical-root/symlink traversal bypass. The owner of this same PR/issue-17 claim implemented the correction on the already-owned feature branch, avoiding a competing feature branch.
- **Files:** `tools/check-local-markdown-links.mjs` now canonicalizes the repo root and each existing target through `realpath`, rejects any resolved target outside the canonical root, fails closed on dangling/looped links, and prevents following a `docs` root symlink that escapes the checkout or dangles. Markdown scanning still skips nested directory symlinks; ordinary in-repo file/directory link targets and compatible internal symlinks are valid. `test/doc-links.test.mjs` adds six synthetic offline cases (16 total), including Windows privilege-aware skipping and a separate dangling-`docs`-root regression.
- **Exact-blob test evidence:** Recreated the complete committed checker and test files in `/mnt/data/vseph_agent17`. Local `git hash-object` results for them: `3b5d68479608ffa588c66ae3de192a7df6cb6cd8` and `c3b5a3c76500c0ea52775e5404e6ffdbff1dcb8e`, **both identical to the corresponding GitHub content SHA** on this PR. Node v22.16.0: `node --check` on each full file passed, `node --test test/doc-links.test.mjs` **16 passed, 0 failed, 0 skipped** (Linux, including file/directory symlinks and CLI nonzero), `node tools/check-local-markdown-links.mjs` exited 0 on the isolated two-file mirror.
- **Limits:** No real out-of-repository attack was observed in checked-in documentation; synthetic fixture tests demonstrate fail-closed resolution only. This checker is not an OS sandbox against concurrent hostile filesystem mutation or a full CommonMark parser. The CLI has **not** been executed against a complete GitHub checkout (local container cannot resolve GitHub DNS); prior GitHub tree audit saw 17 Markdown files/38 local links but did not perform a real filesystem symlink walk. GitHub Actions, astronomical code/data, production and deployment remain untouched.
- **Next:** Request independent exact-head review of [PR #18](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/18); owner handles integration of this code PR under the fleet's docs-only merge restriction.

## Template for future entries

### YYYY-MM-DD — <short task>
- **Issue/PR/branch:**
- **Facts observed / exact source:**
- **Code/docs/fixtures changed:**
- **Commands run and results (or not run):**
- **Unresolved risks / blockers:**
- **Next smallest safe task:**
