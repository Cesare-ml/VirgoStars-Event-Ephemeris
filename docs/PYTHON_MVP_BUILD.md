# Windows: build and test the first Python/Rust synod MVP

**Status 2026-11-11: scaffold implemented on draft PR #5; not yet compiled, pip-installed or tested end-to-end with Rust.** Do not call this a released package. No GitHub Actions, dataset redistribution or production replacement.

## 1. Safely synchronize the development branch

From PowerShell in `D:\\GitHub\\VirgoStars-Event-Ephemeris`:

```powershell
git status --short
git branch --show-current
git fetch origin
```

**Before pulling, preserve any uncommitted work.** The maintainer previously ran local `cargo fmt`, which may have modified Rust files. Inspect `git diff`, then either commit the desired changes on a local branch, or use `git stash push -u -m "before Python MVP branch sync"` and later review/apply the stash deliberately. Do not blindly reset/restore or overwrite important local edits. Switch to `feat/p0-2-rust-workspace` and `git pull --ff-only origin feat/p0-2-rust-workspace` only once the working tree is safe and clean.

**Important:** Git conflict markers were found and removed from the remote PR #5 Rust sources in this increment; a stale local working tree may still contain them. Check that `git grep -n -E '^(<<<<<<< |=======|>>>>>>> )' -- '*.rs'` has no matches before compiling.

## Reproducible Cargo.lock required before any M0 wheel build

On this draft branch the checked-in `Cargo.lock` still lacks the Rust PyO3 binding crate and `tempfile`. The new validation pipeline **refuses** to compile or create a wheel while it is stale. With Cargo installed, regenerate and review the lock on your **development branch**, without silently modifying `main`:

```powershell
cargo generate-lockfile
git diff -- Cargo.lock
py -3 -m unittest discover -s tools -p "test_validate_rust.py" -v
py -3 tools\build_python_mvp.py
```

After successful Windows compilation, commit the *reviewed*, compiler-generated `Cargo.lock` onto PR #5. Do not hand-author crate digests. `tools/validate_rust.py` begins with `cargo metadata --locked --no-deps --format-version 1` and all subsequent cargo compile/test commands use `--locked`. `tools/build_python_mvp.py` runs `maturin build --locked`, and `pyproject.toml` sets `[tool.maturin] locked = true` so even source `pip install .` cannot update dependency resolution silently. This uses the documented maturin option; `--locked` is **not** an offline-mode guarantee. 

**Status:** lock regeneration and the real Rust/PyO3/42-test Windows acceptance **NOT RUN here**; the authoring container lacks Cargo/rustc/maturin.

## 2. Rust baseline gates (mandatory)

```powershell
cargo fmt --all
py -3 tools\validate_rust.py
```

The script checks rustfmt, Clippy with warnings as errors, all Rust unit/integration tests and the workspace release build. The Python binding crate now joins the Cargo workspace and requires a supported Python toolchain in addition to Cargo and Visual C++ Build Tools. **If any step fails, capture and report the first error**; do not waive the gate or call M0 finished.

## 3. Local pip install, without any dataset download

```powershell
py -3 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --upgrade pip
.\.venv\Scripts\python.exe -m pip install .
.\.venv\Scripts\python.exe -m unittest discover -s python\tests -p "test_*.py" -v
```

This builds the local PyO3/maturin source package. Installation from **source** needs Rust; a **prebuilt matching wheel** must install without Rust on the target PC. Pure-Python UTC/dictionary tests do not require a native wheel to be available and should pass independently. Native smoke tests **skip** if the compiled extension or explicit local data are absent; report skip counts rather than presenting them as passes.

For editable source development:

```powershell
.\.venv\Scripts\python.exe -m pip install -e .
```

**2026-11-12 UTC oracle precision update:** The independent source-CSV oracle now refuses submicrosecond ISO strings and noncanonical UTC probes instead of accepting Python's silent fractional truncation. Two new source-oracle regressions increase the mandatory installed acceptance gate to **42 non-skipped tests**: 15 Python API/time + 6 CLI + 12 CSV-oracle + 9 native fixture. These new cases were added to PR #5; three isolated exact-parser tests passed, but **the full repository/compiled native gate has NOT RUN**.\n\n## 3a. Mandatory synthetic native end-to-end gate (no owner dataset needed)

Once `pip install .` succeeds, run:

```powershell
.\.venv\Scripts\python.exe tools\validate_python_mvp.py
```

This gate now requires **15 pure-Python API/time tests, 6 CLI tests, 12 independent CSV-oracle tests and 9 actual PyO3/Rust-to-Python tests** against the repository's independently generated synthetic 8-event golden P0 fixture. It checks exact source IDs, Moon new-moon equality boundaries, phase-label transitions, `NULL` vs `[]` extras, ISO timezone-offset equivalence, JSON serialization, and rejection of a wrong whole-file SHA-256.

**Unlike standard `unittest`, the validator exits with failure if a native test is skipped**, if the compiled `virgostars_events._native` module cannot be imported, or if fewer than the expected cases are discovered. An `OK (skipped=5)` from a basic unittest run is **not** a successful native gate.

The synthetic fixture is deliberately independent of the separate 380,844-event owner dataset and never requires a secret or network connection.

## 3b. Offline installed command-line usage

After `pip install .`, the same Rust-backed query can be issued without writing a Python script:

```powershell
.\\.venv\\Scripts\\python.exe -m virgostars_events "D:\\path\\to\\authorized\\events.vseph-p0" "2026-11-11T12:00:00Z" --sha256 "<trusted whole-file SHA-256>"
.\\.venv\\Scripts\\virgostars-events.exe --version
```

A standard `[project.scripts]` console entry point (`virgostars-events`) is included in the package metadata. The CLI prints JSON to stdout, reports errors to stderr and requires an independently trusted digest and explicit local file path. Use `--compact` for one-line JSON. ISO strings use **up to six** fractional-second digits and an explicit `Z` or numeric `±HH:MM` offset; more precision is rejected, never silently rounded or truncated. **The console command still requires wheel-build validation on Windows.**

## 4. Verify against an independently trusted local snapshot

The user-authorized historical data are held separately in draft PR #6, not packaged inside the wheel. After assembling the dataset on a **read-only/disposable checkout** and verifying its SHA-256, set:

```powershell
$env:VSEPH_P0_PATH = "D:\\GitHub\\VirgoStars-Event-Ephemeris\\datasets\\2026-11-10\\_assembled\\virgostars-events-20261110.vseph-p0"
$env:VSEPH_P0_SHA256 = "c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e"
.\.venv\Scripts\python.exe tools\validate_python_mvp.py --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256
.\.venv\Scripts\python.exe examples\python_synods.py $env:VSEPH_P0_PATH "2026-11-11T12:00:00Z" --sha256 $env:VSEPH_P0_SHA256
```

The smoke tests must **run**, not skip, and return source IDs/recorded phases plus `coverage.status="unreviewed"`. Do not mistake the current zero-window coverage policy for verified astronomical completeness. The recorded Moon synod gaps in 1919/2018 are now guarded against year-long `CAL2` extrapolation; optional owner-data native smoke must verify explicit unknown phases and `moon_recorded_cycle_gap_over_45_days` warnings. This does **not** certify scientific coverage or repair the missing events; Moon ingress 2099–4836 and the gaps still require independent review. See [Moon source gaps](MOON_SYNOD_SOURCE_GAPS.md).

### Required owner-archive Moon-gap regression

The real-data smoke suite now contains **3 owner-snapshot tests**, including the two source gaps (1919 and 2018). Using the owner-approved 2026-11-10 P0 snapshot, the smoke test must prove that `get_synods_at("1919-06-01T00:00:00Z")` and `get_synods_at("2018-06-01T00:00:00Z")` retain recorded Moon source IDs **44934** and **46146**, report `phase.code=None` / `phase.status="unknown_discontinuous_moon_anchors"`, and include the `moon_recorded_cycle_gap_over_45_days` warning. Normal Moon cycles must still classify their source thresholds. A gap guard is not scientific completeness certification; see [Moon gaps](MOON_SYNOD_SOURCE_GAPS.md).

The independent CSV oracle contains **12 tests** (including UTC probe precision and a year-long gap regression); the mandatory base Python/PyO3 suite contains **42 non-skipped tests** (15 API/time + 6 CLI + 12 oracle + 9 native synthetic). With the owner archive, **three more** smoke tests and source CSV parity are required. **All new Rust and native Python changes remain to be built and executed on Windows.**

## 4a. Independent source-CSV parity of the installed Python API

After successfully compiling/installing the wheel and assembling the **owner-authorized** normalized CSV and P0 snapshot from draft PR #6 (no merging or publishing required), run:

```powershell
.\\.venv\\Scripts\\python.exe tools\\verify_python_synods.py --csv "D:\\path\\to\\authorized\\sinodi.csv" --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256
```

This is a **read-only, fail-closed** compatibility check of the *real installed native Python API*: it **loads and validates all 70,554 original normalized `sinodi.csv` rows** to build an independent reference index, then compares the API at a bounded set of deterministic test instants. It does **not** execute an API lookup for every original row. The script does **not** contain a second P0 binary decoder or astronomical engine; it reuses the independent CSV normalization from `tools/verify_csv_p0.py`, selects the last recorded anchor `<= instant` using the source CSV, and compares all bodies' source IDs, exact starts, next recorded anchors, `NULL` vs `[]` vs phase thresholds, derived phase code/status and the `new_moon` alias.

Default probes include `2026-11-11T12:00:00Z`, the latest selected anchor **for every indexed synod body**, one microsecond before/at/after each anchor and up to three phase thresholds per selected body, just before the first recorded Moon anchor, and representative dates `1919-01-01` and `2018-01-01` for known candidate Moon gaps. These remain **sampled queries** across the independently loaded full CSV, not exhaustive per-row API execution or scientific coverage certification. It rejects missing/extra bodies, false scientific coverage claims, wrong archive hash or first unexpected output. **This test compares recorded data and the API, not astronomical completeness.** All local CSV/P0 paths are explicit; neither original CSV nor P0 data are committed in the code PR.

For a **single fail-closed validation command** (pure Python unit tests, synthetic native fixture, owner-data smoke and CSV source parity), supply the optional `--synods-csv` argument to the M0 validator:

```powershell
.\\.venv\\Scripts\\python.exe tools\\validate_python_mvp.py --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256 --synods-csv "D:\\path\\to\\authorized\\sinodi.csv"
```

The `--synods-csv` flag is rejected unless the archive and trusted SHA are also supplied. All comparisons are read-only. A missing native binding, skipped test, wrong CSV source ID, different phase threshold, missing body or falsely certified coverage produces a nonzero exit status.

The new test-only oracle's unit tests are in `tools/test_verify_python_synods.py`:

```powershell
py -3 -m unittest discover -s tools -p "test_verify_python_synods.py" -v
```

**Status:** the eight original CSV-oracle tests were written previously; one additional regression test now checks non-Moon/non-Mercury body selection and rejects empty indexed series, for **12 independent source-oracle tests**. Their current execution is **not yet verified**. The mandatory `tools/validate_python_mvp.py` now runs these 10 tests as a third suite, in addition to 12 pure-Python API/time tests, 6 CLI tests and 6 native golden tests. **Do not report a passing native or source-parity gate until it actually runs on an installed wheel.**

## 5. Produce a local wheel and reinstall it without build dependencies

```powershell
.\.venv\Scripts\python.exe -m pip install "maturin>=1.8,<2"
.\.venv\Scripts\maturin.exe build --release --out dist
py -3 -m venv .venv-wheel-test
.\.venv-wheel-test\Scripts\python.exe -m pip install --no-index (Get-ChildItem dist\*.whl | Select-Object -Last 1).FullName
.\.venv-wheel-test\Scripts\python.exe -c "import virgostars_events; print(virgostars_events.__version__)"
```

Wheel installation with `--no-index` proves it can install without downloading/building Rust dependencies (provided the target Python ABI and platform match). Real P0 smoke should also run in this second virtual environment.

## 6. One-command verification of a prebuilt wheel in an isolated offline environment

After `maturin build --release --out dist` succeeds, select the wheel that matches the host Python ABI/platform, then run the new **local installation gate** (no GitHub Actions, network, PyPI publishing or dataset downloads):

```powershell
$wheel = (Get-ChildItem .\dist\*.whl | Select-Object -Last 1).FullName
py -3 tools\verify_python_wheel.py --wheel $wheel
```

This creates and deletes a temporary virtual environment, performs `pip install --no-index --no-deps` on the **actual wheel**, requires the installed `_native` file to be a compiled extension (not merely a Python shim) physically inside the test venv, runs both `python -m virgostars_events --version` and the pip-created `virgostars-events --version` console launcher (Windows `.exe`), and invokes `tools/validate_python_mvp.py` with its mandatory 34 non-skipped checks (12 Python API, 6 CLI, 10 CSV-oracle, 6 native fixture). Invalid inputs and subprocess failures return nonzero. This requires a **prebuilt wheel**; it does not compile Rust or certify scientific completeness.

When the owner-authorized separate archive and normalized source CSV have been assembled and verified, the stricter read-only comparison is:

```powershell
py -3 tools\verify_python_wheel.py --wheel $wheel --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256 --synods-csv "D:\path\to\sinodi.csv"
```

Unit tests for the *orchestrator* (subprocess calls are mocked, not a substitute for a real wheel):

```powershell
py -3 -m unittest discover -s tools -p "test_verify_python_wheel.py" -v
```

**As of this change:** 8/8 wheel-gate orchestration tests were executed successfully under Python 3.13, with exact GitHub blobs SHA-verified against the locally tested files. A real offline-install **negative** smoke against a deliberately fake pure-Python `_native.py` wheel correctly failed the binary-extension check (exit 1), without validating any compiled Rust wheel. Subprocess environments strip inherited `PYTHONPATH`, `PYTHONHOME` and `PYTHONUSERBASE` to keep the installed console script from resolving developer source code. **No Rust wheel was built or installed here** because Cargo/maturin are unavailable; the complete native gate remains pending on a developer machine. Other Python/platform versions need their own matching wheels.

## 7. One-command local Rust build + wheel acceptance

The optional local helper `tools/build_python_mvp.py` combines all existing required gates into **one fail-closed workflow**. This is a source-checkout developer tool, not an installer or PyPI publisher. It requires Rust/Cargo and **maturin already installed for the Python interpreter being used**.

```powershell
# From the checked-out PR #5 source, with Cargo on PATH:
py -3 -m pip install "maturin>=1.8,<2"
py -3 -m unittest discover -s tools -p "test_build_python_mvp.py" -v
py -3 tools\build_python_mvp.py
```

The script runs `tools/validate_rust.py` first (fmt, Clippy, Rust tests, release build), calls `maturin build --release --interpreter <current Python>` into a disposable empty directory, requires exactly one newly built wheel, then runs `tools/verify_python_wheel.py` to perform the **real, offline wheel installation** and **all 40 mandatory non-skipped Python tests**. Only **after every gate has succeeded** is the SHA-256-checked wheel published into `dist/`. The helper writes a same-directory `.part` file, flushes/syncs it, compares the original and staged SHA-256 again, and uses an **atomic no-clobber hard link** to expose the completed `.whl`. Existing wheels are never overwritten (including a concurrent writer); a failed build/check does not create a new final `.whl`. This requires a filesystem that supports same-directory file hard links (normally local NTFS on Windows); unsupported filesystems fail closed instead of publishing a partial artifact. A process or machine crash might leave a hidden `.part` staging file, which may be deleted after verifying no build is active. This does not tag or upload a release, change GitHub Actions, or bundle original scientific data.

To additionally require original source CSV parity, provide your existing separately authorized local files:

```powershell
py -3 tools\build_python_mvp.py --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256 --synods-csv "D:\path\to\sinodi.csv"
```

**Atomic publication update (2026-11-12):** five additional Python regression cases have been added to `tools/test_build_python_mvp.py` covering invisible staging, interrupted copies, competing writers, mutable source and unsupported hard links (the previous race test was replaced). Six equivalent targeted checks ran successfully in an isolated Python 3 environment, but the complete repository test file, Rust/Cargo gates and any real compiled wheel have **NOT RUN** here. Review the atomic hard-link requirement before choosing a non-NTFS output volume.

**Verification status (2026-11-12):** 7/7 orchestration tests **PASS** on Python 3.13 using mocked Cargo/maturin/acceptance process calls. The committed source files exactly match tested Git object hashes: `957a4ea3b14cf7ac074cb65a94f4eca23f42cf8c` and `907bcfb814d8f96d5f05bfd20fea2de3d0d6f3b7`. `compileall` passed. **NOT EXECUTED:** actual Cargo/maturin build or Windows native wheel/owner CSV acceptance in the agent environment; no Rust toolchain is present. The script is not proof of successful production wheel delivery.

## Validation matrix and scope

| Check | Status as documented here |
| --- | --- |
| Python UTC microseconds + dictionary projection, now 10 tests | **8/8 passed on an earlier revision**; two new precision/strict-ISO tests **NOT RUN** on the current branch |
| Python CLI JSON/exit behavior, 5 tests | **WRITTEN, NOT RUN** — requires no native extension for these mocked tests |
| Synthetic native P0 fixture + PyO3 binding, 9 tests | **WRITTEN, NOT RUN** — `tools/validate_python_mvp.py` fails on a skip |
| Rust synod phase classifier, 5 new tests | **NOT RUN** |
| Rust latest-synods P0 batch query, 4 new tests | **NOT RUN** |
| Rust/PyO3 extension build, Clippy, workspace tests | **NOT RUN** on new commits |
| `pip install .`, editable install, wheel build, fresh wheel install | **NOT RUN** |
| Native Python smoke against assembled real P0 | **NOT RUN** |
| PostgreSQL backend parity and coverage review | **NOT RUN** |

The Python surface is a prototype **for precomputed recorded events**, not a new astronomy engine. Before a public PyPI `pip install virgostars-event-ephemeris` is advertised, obtain a separate package release authorization and the dataset license decision ([issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9)). Maintain CPAL-1.0 source licensing and visible **Powered by VirgoStars — https://www.virgostars.it/** credits.

### Input checks before archive I/O (2026-11-11)

`VirgoStarsEvents.open(..., expected_sha256=...)` validates exactly 64 hexadecimal SHA-256 characters **before importing the Rust extension**, then forwards a lowercase digest for the native whole-file check. Invalid instants on the CLI are rejected before opening/hashing the archive. For aware Python datetimes using a DST timezone, nonexistent local spring-forward wall times raise `ValueError` rather than converting silently. These boundary guards have three new tests, using a synthetic timezone so Windows does not require system timezone data. Run the expanded **34-test** native gate after installing the local wheel; this is not evidence of a successful wheel build.

## 8. Private authenticated archive snapshot (source edits do not affect a reader)

The Rust Python binding now creates a secure private temporary directory, copies the separately authorized source P0 file into it, and checks **the copied bytes** against the independently trusted whole-file SHA-256 at reader creation. Queries use only the private copy; modifying/replacing/deleting the source path later no longer affects an open reader. No per-query full-file checksum scans are required. Temporary copies persist for as long as native readers are alive and are cleaned up when released. Budget extra free disk space roughly equal to one archive per live reader and use a trusted temp directory; this is not an adversary-proof file lock.

**Required native regression count: 9; total mandatory: 40 = 15 API/time + 6 CLI + 12 source oracle + 9 PyO3 golden fixture.** The native fixture cases exercise source in-place alteration, atomic replacement and deletion after construction. The modified native binding and these tests **have not been compiled or run** in this authoring environment.

**Cargo.lock checkpoint:** introducing the new Rust `tempfile` crate requires regenerating the tracked `Cargo.lock`. That lockfile did not include PyO3's workspace dependencies previously either. Do not hand-edit checksums. On the Windows developer checkout with Cargo available, run `cargo generate-lockfile`, review/commit the result, then validate and build:

```powershell
cargo generate-lockfile
py -3 tools\build_python_mvp.py
```

Only report successful compiled wheel/native test results after the commands actually pass. Scientific owner archive parity requires the additional trusted P0/CSV arguments documented above.

## 9. Close the Python reader to release its private archive copy

Each `VirgoStarsEvents.open()` keeps a per-reader private P0 snapshot in temporary storage. Prefer an explicit context manager for scripts and long-lived services:

```python
from virgostars_events import VirgoStarsEvents

with VirgoStarsEvents.open(path, expected_sha256=trusted_sha256) as events:
    result = events.get_synods_at("2026-11-11T12:00:00Z")
# Reader closed even if the query raises an exception.
```

Or call `events.close()` when finished; repeated calls are safe. `events.closed` becomes `True`. Attempting `events.get_synods_at(...)` or re-entering a closed reader raises a descriptive `RuntimeError`. Clearing the native reference allows Rust `tempfile::TempDir` cleanup when no other references remain. No underlying P0 data are deleted. Three new Python lifecycle regressions raise the required gate from 37 to **40 tests** (15 pure-Python/time, 6 CLI, 10 source oracle, 9 compiled native fixtures). Three cases in an *isolated local reproduction* passed Python 3.13, **not** the compiled wheel or full repository suite. `Cargo.lock` regeneration remains required before compiling the PyO3 crate.
