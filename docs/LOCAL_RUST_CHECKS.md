# Local Rust validation: Windows, macOS and Linux

**Status: development tool in [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); no deployment, GitHub Actions or merge.** Read `AGENTS.md` before intervening.

The project has Rust as the authoritative codebase, but the agent environment used on 2026-11-10 does **not** have `rustc` or `cargo`. Therefore the Rust implementation is not compiled or validated for release. To enable contributors — including those unfamiliar with Rust — to run the same checks easily, use the stdlib-only helper `tools/validate_rust.py`.

## First-time setup

1. Install **Python 3.10+**. For Windows, use a Python installation that exposes the `py` launcher.
2. Install the standard Rust toolchain from [rustup.rs](https://rustup.rs/), including `cargo`, `rustfmt` and `clippy`. Restart the terminal.
3. From your clone, check that you are on the **intended development branch**, not changing `main` or production. The code currently lives on `feat/p0-2-rust-workspace`, pending PR #5.
4. Run from repository root in Windows PowerShell:

   ```powershell
   py -3 tools\validate_rust.py
   ```

   Or on macOS/Linux:

   ```bash
   python3 tools/validate_rust.py
   ```

**It runs, in order:**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Any failed command stops the run and exits with status 1. There is no automatic merge, release, package upload, network service, GitHub Actions workflow or live database change. Cargo may need crates.io to install the explicitly declared development dependencies on the first build. No live backend is involved.

## Full compressed-data parity, when both draft PRs are available in a local testing checkout

The experimental Rust P0 adapter and validation scripts live on PR #5, whereas **the complete 2026-11-10 dataset and `tools/assemble_dataset.py` are on separate draft PR #6**. Test in a temporary combined checkout: **do not merge either PR to `main` just to run the check**.

First reconstruct the immutable dataset as described in the [dataset PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). Then run:

Windows PowerShell:

```powershell
py -3 tools\validate_rust.py --archive "datasets\2026-11-10\_assembled\virgostars-events-20261110.vseph-p0" --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

macOS/Linux:

```bash
python3 tools/validate_rust.py --archive datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

After the four Cargo gates, the script executes:

- The independent Python P0 byte-level auditor with the expected file SHA-256.
- The Rust `p0-verify` CLI with the **same** expected file SHA-256.
- A **strict comparison of JSON output**: `sha256`, `blocks`, `records`, `sections`. Both audits must agree exactly. The Rust CLI must report `sha256_checked_against_expected: true` and `coverage_certified: false`.

Expected counts for this specific 2026-11-10 archive: **761 blocks**, **380,844 records** (70,554 synods, 266,516 motions, 43,774 ingresses). The SHA-256 is fixed **only for this immutable snapshot**, not for future versions. Do not copy this digest to an updated dataset manifest.

## How the helper itself was tested

Run the self-tests (Python stdlib, no Rust dependency):

```sh
python -m unittest discover -s tools -p test_validate_rust.py -v
```

On 2026-11-10 **9/9 tests passed**, verifying fail-fast behavior when Cargo is missing, exact gate ordering, rejection of malformed digest/missing archives, independent auditor invocation and refusal of mismatched Python/Rust count reports. **These tests use a simulated Cargo result and DO NOT compile Rust.** Source and test bytes were checked against the GitHub branch using exact Git blob SHA-1 values.

Also separately executed: `tools/test_audit_p0_bytes.py` **8/8 PASS** on the actual P0 binary, with all **761** blocks and **380,844** encoded events passing the independent Python audit.

**Still required before merge:** Rust cargo fmt, Clippy, workspace tests, release build and actual Rust/Python archive parity on a machine with Cargo; fix any failures before approval. Avoid GitHub Actions by project policy. This helper checks file integrity and consistency, not the astronomical completeness of missing intervals.

Attribution: Powered by VirgoStars — https://www.virgostars.it/. Code CPAL-1.0; scientific dataset license to be decided separately.
