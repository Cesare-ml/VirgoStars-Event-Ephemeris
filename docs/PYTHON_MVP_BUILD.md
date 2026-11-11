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

## 4. Verify against an independently trusted local snapshot

The user-authorized historical data are held separately in draft PR #6, not packaged inside the wheel. After assembling the dataset on a **read-only/disposable checkout** and verifying its SHA-256, set:

```powershell
$env:VSEPH_P0_PATH = "D:\\GitHub\\VirgoStars-Event-Ephemeris\\datasets\\2026-11-10\\_assembled\\virgostars-events-20261110.vseph-p0"
$env:VSEPH_P0_SHA256 = "c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e"
.\.venv\Scripts\python.exe -m unittest discover -s python\tests -p "test_native_smoke.py" -v
.\.venv\Scripts\python.exe examples\python_synods.py $env:VSEPH_P0_PATH "2026-11-11T12:00:00Z" --sha256 $env:VSEPH_P0_SHA256
```

The smoke tests must **run**, not skip, and return source IDs/recorded phases plus `coverage.status="unreviewed"`. Do not mistake the current zero-window coverage policy for verified astronomical completeness. Known lunar gaps 1919/2018 and Moon ingress 2099–4836 need independent review.

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
| Rust synod phase classifier, 5 new tests | **NOT RUN** |
| Rust latest-synods P0 batch query, 4 new tests | **NOT RUN** |
| Rust/PyO3 extension build, Clippy, workspace tests | **NOT RUN** on new commits |
| `pip install .`, editable install, wheel build, fresh wheel install | **NOT RUN** |
| Native Python smoke against assembled real P0 | **NOT RUN** |
| PostgreSQL backend parity and coverage review | **NOT RUN** |

The Python surface is a prototype **for precomputed recorded events**, not a new astronomy engine. Before a public PyPI `pip install virgostars-event-ephemeris` is advertised, obtain a separate package release authorization and the dataset license decision ([issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9)). Maintain CPAL-1.0 source licensing and visible **Powered by VirgoStars — https://www.virgostars.it/** credits.
