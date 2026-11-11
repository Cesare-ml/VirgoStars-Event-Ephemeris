#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Verify a PREBUILT Python wheel in a fresh offline virtual environment.

This is the last local M0 installation gate, not a build or PyPI publisher.
It installs only the caller-supplied wheel (no index, no dependencies),
checks that both the Python package and native extension import from the
new environment, and runs the repository's fail-closed native test harness.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import venv
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]

# Execute in a DIFFERENT working directory so source imports cannot mask a
# missing wheel. Python's import check also requires the installed location
# to be under the temporary venv, rather than a PYTHONPATH checkout.
INSTALLED_PROBE = """
from importlib.metadata import version
from pathlib import Path
import sys
import virgostars_events
from virgostars_events._native import NativeReader
package = Path(virgostars_events.__file__).resolve()
if not package.is_relative_to(Path(sys.prefix).resolve()):
    raise SystemExit(f'FAIL: package imported outside test venv: {package}')
if version('virgostars-event-ephemeris') != virgostars_events.__version__:
    raise SystemExit('FAIL: Python module version differs from installed wheel')
print(f'Installed VirgoStars native wheel: {package}')
""".strip()


def venv_python(directory: Path) -> Path:
    """Return the venv interpreter, including the Windows Scripts layout."""
    return directory / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def run_checked(command: Sequence[str], *, cwd: Path) -> None:
    """Propagate any failing subprocess; never report partial tests as pass."""
    print("\n[wheel check]", " ".join(map(str, command)), flush=True)
    completed = subprocess.run(list(map(str, command)), cwd=str(cwd), check=False)
    if completed.returncode != 0:
        raise RuntimeError(f"command exited {completed.returncode}: {command[0]}")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Prove that a prebuilt VirgoStars Python wheel installs offline and passes native tests"
    )
    parser.add_argument("--wheel", required=True, type=Path, help="Existing local .whl file")
    parser.add_argument("--archive", type=Path, help="Optional authorized VSEPH-P0 archive")
    parser.add_argument("--sha256", help="Trusted archive SHA-256 (required with --archive)")
    parser.add_argument("--synods-csv", type=Path, help="Optional independent source CSV parity")
    args = parser.parse_args(argv)

    if not args.wheel.is_file() or args.wheel.suffix != ".whl":
        parser.error("--wheel must name an existing .whl file")
    if bool(args.archive) != bool(args.sha256):
        parser.error("--archive and --sha256 must be provided together")
    if args.sha256 and re.fullmatch(r"[0-9a-fA-F]{64}", args.sha256) is None:
        parser.error("--sha256 must be a 64-character hexadecimal SHA-256")
    if args.archive and not args.archive.is_file():
        parser.error("--archive must name an existing file")
    if args.synods_csv and (not args.archive or not args.synods_csv.is_file()):
        parser.error("--synods-csv requires an archive and an existing source CSV")

    wheel = args.wheel.resolve()
    validator = ROOT / "tools" / "validate_python_mvp.py"
    with tempfile.TemporaryDirectory(prefix="virgostars-wheel-verify-") as tmp:
        temp = Path(tmp)
        venv.EnvBuilder(with_pip=True, clear=True).create(str(temp / "venv"))
        python = venv_python(temp / "venv")
        try:
            # --no-index + --no-deps must remain present: this is an offline
            # installation proof, not a request to fetch/build dependencies.
            run_checked(
                [python, "-m", "pip", "install", "--no-index", "--no-deps",
                 "--disable-pip-version-check", wheel], cwd=temp
            )
            run_checked([python, "-I", "-c", INSTALLED_PROBE], cwd=temp)
            run_checked([python, "-I", "-m", "virgostars_events", "--version"], cwd=temp)
            command: list[str | Path] = [python, "-I", validator]
            if args.archive:
                command.extend(["--archive", args.archive.resolve(), "--sha256", args.sha256.lower()])
            if args.synods_csv:
                command.extend(["--synods-csv", args.synods_csv.resolve()])
            run_checked(command, cwd=temp)
        except (OSError, RuntimeError) as exc:
            print(f"FAIL: clean offline Python wheel gate: {exc}", file=sys.stderr)
            return 1

    print("\nPASS: prebuilt wheel installed offline and passed all mandatory native tests")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
