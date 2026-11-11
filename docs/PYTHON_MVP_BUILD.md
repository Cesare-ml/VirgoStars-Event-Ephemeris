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

## 3a. Mandatory synthetic native end-to-end gate (no owner dataset needed)

Once `pip install .` succeeds, run:

```powershell
.\.venv\Scripts\python.exe tools\validate_python_mvp.py
```

This gate runs the **8 pure-Python tests** and **5 actual PyO3/Rust-to-Python tests** against the repository's independently generated synthetic 8-event golden P0 fixture. It checks exact source IDs, Moon new-moon equality boundaries, phase-label transitions, `NULL` vs `[]` extras, ISO timezone-offset equivalence, JSON serialization, and rejection of a wrong whole-file SHA-256.

**Unlike standard `unittest`, the validator exits with failure if a native test is skipped**, if the compiled `virgostars_events._native` module cannot be imported, or if fewer than the expected cases are discovered. An `OK (skipped=5)` from a basic unittest run is **not** a successful native gate.

The synthetic fixture is deliberately independent of the separate 380,844-event owner dataset and never requires a secret or network connection.

## 4. Verify against an independently trusted local snapshot

The user-authorized historical data are held separately in draft PR #6, not packaged inside the wheel. After assembling the dataset on a **read-only/disposable checkout** and verifying its SHA-256, set:

```powershell
$env:VSEPH_P0_PATH = "D:\\GitHub\\VirgoStars-Event-Ephemeris\\datasets\\2026-11-10\\_assembled\\virgostars-events-20261110.vseph-p0"
$env:VSEPH_P0_SHA256 = "c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e"
.\.venv\Scripts\python.exe tools\validate_python_mvp.py --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256
.\.venv\Scripts\python.exe examples\python_synods.py $env:VSEPH_P0_PATH "2026-11-11T12:00:00Z" --sha256 $env:VSEPH_P0_SHA256
```

The smoke tests must **run**, not skip, and return source IDs/recorded phases plus `coverage.status="unreviewed"`. Do not mistake the current zero-window coverage policy for verified astronomical completeness. Known lunar gaps 1919/2018 and Moon ingress 2099–4836 need independent review.

## 4a. Independent source-CSV parity of the installed Python API

After successfully compiling/installing the wheel and assembling the **owner-authorized** normalized CSV and P0 snapshot from draft PR #6 (no merging or publishing required), run:

```powershell
.\\.venv\\Scripts\\python.exe tools\\verify_python_synods.py --csv "D:\\path\\to\\authorized\\sinodi.csv" --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256
```

This is a **read-only, fail-closed** compatibility check of the *real installed native Python API* against all 70,554 normalized original `sinodi.csv` records. The script does **not** contain a second P0 binary decoder or astronomical engine; it reuses the independent CSV normalization from `tools/verify_csv_p0.py`, selects the last recorded anchor `<= instant` using the source CSV, and compares all bodies' source IDs, exact starts, next recorded anchors, `NULL` vs `[]` vs phase thresholds, derived phase code/status and the `new_moon` alias.

Default probes include `2026-11-11T12:00:00Z`, exact Moon/Mercury anchors, one microsecond before/after, each of the three original phase thresholds where available, and just before the first recorded Moon anchor. It rejects missing/extra bodies, false scientific coverage claims, wrong archive hash or first unexpected output. **This test compares recorded data and the API, not astronomical completeness.** All local CSV/P0 paths are explicit; neither original CSV nor P0 data are committed in the code PR.

The new test-only oracle's unit tests are in `tools/test_verify_python_synods.py`:

```powershell
py -3 -m unittest discover -s tools -p "test_verify_python_synods.py" -v
```

**Status as written:** independent user-provided normalized CSV files have been inspected (70,554 synod rows across 11 source bodies), but the new parity tool's tests and live Rust-backed parity **have not yet been executed** on the full patched checkout. Do not report the gate green until it runs on the installed wheel.

## 5. Produce a local wheel and reinstall it without build dependencies

```powershell
.\.venv\Scripts\python.exe -m pip install "maturin>=1.8,<2"
.\.venv\Scripts\maturin.exe build --release --out dist
py -3 -m venv .venv-wheel-test
.\.venv-wheel-test\Scripts\python.exe -m pip install --no-index (Get-ChildItem dist\*.whl | Select-Object -Last 1).FullName
.\.venv-wheel-test\Scripts\python.exe -c "import virgostars_events; print(virgostars_events.__version__)"
```

Wheel installation with `--no-index` proves it can install without downloading/building Rust dependencies (provided the target Python ABI and platform match). Real P0 smoke should also run in this second virtual environment.

## Validation matrix and scope

| Check | Status as documented here |
| --- | --- |
| Pure Python UTC microseconds + dictionary projection, 8 tests | **PASS 8/8** on locally reconstructed files verified byte-for-byte against GitHub blob SHA-1 |
| Synthetic native P0 fixture + PyO3 binding, 5 tests | **WRITTEN, NOT RUN** — `tools/validate_python_mvp.py` fails on a skip |
| Rust synod phase classifier, 5 new tests | **NOT RUN** |
| Rust latest-synods P0 batch query, 4 new tests | **NOT RUN** |
| Rust/PyO3 extension build, Clippy, workspace tests | **NOT RUN** on new commits |
| `pip install .`, editable install, wheel build, fresh wheel install | **NOT RUN** |
| Native Python smoke against assembled real P0 | **NOT RUN** |
| PostgreSQL backend parity and coverage review | **NOT RUN** |

The Python surface is a prototype **for precomputed recorded events**, not a new astronomy engine. Before a public PyPI `pip install virgostars-event-ephemeris` is advertised, obtain a separate package release authorization and the dataset license decision ([issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9)). Maintain CPAL-1.0 source licensing and visible **Powered by VirgoStars — https://www.virgostars.it/** credits.
