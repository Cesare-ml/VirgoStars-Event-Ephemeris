#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Read-only local prerequisites for the first Rust/PyO3 wheel.

A pass here is NOT a successful Rust build, installed wheel or scientific audit.
"""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import sys
import sysconfig
from typing import Callable

from prepare_cargo_lock import missing_packages

ROOT = Path(__file__).resolve().parents[1]


def problems(
    root: Path = ROOT,
    *,
    which: Callable[[str], str | None] = shutil.which,
    scripts_dir: Path | None = None,
    python_version: tuple[int, int] | None = None,
    python_impl: str | None = None,
) -> list[str]:
    """Inspect prerequisites without running installers or modifying files."""
    issues: list[str] = []
    version = python_version or sys.version_info[:2]
    impl = python_impl or sys.implementation.name
    if impl != "cpython" or not ((3, 10) <= version <= (3, 13)):
        issues.append("Use supported CPython 3.10–3.13 for the first M0 wheel")

    for name in ("Cargo.toml", "pyproject.toml", "crates/python-bindings/Cargo.toml"):
        if not (root / name).is_file():
            issues.append(f"Missing workspace file: {name}")

    lock = root / "Cargo.lock"
    if lock.is_symlink():
        issues.append("Cargo.lock is a symlink; use a tracked regular file")
    else:
        missing = missing_packages(lock)
        if missing:
            issues.append(
                "Cargo.lock missing: " + ", ".join(sorted(missing))
                + "; on a clean Cargo-equipped checkout run "
                + "py -3 tools\\prepare_cargo_lock.py --apply"
            )

    for tool in ("rustc", "cargo"):
        if which(tool) is None:
            issues.append(f"{tool} not found on PATH (install the Rust toolchain)")
    scripts = scripts_dir or Path(sysconfig.get_path("scripts"))
    exe = scripts / ("maturin.exe" if os.name == "nt" else "maturin")
    if not exe.is_file() and which("maturin") is None:
        issues.append("maturin not found for this Python or on PATH (python -m pip install maturin)")
    return issues


def main() -> int:
    found = problems()
    if found:
        print("M0 wheel prerequisites NOT ready:")
        for reason in found:
            print(f" - {reason}")
        print("No files changed; no Rust tests, wheel or owner-data parity executed.")
        return 1
    print("M0 preliminary environment checks PASS (not a build or test result).")
    print("Next run: py -3 tools\\build_python_mvp.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
