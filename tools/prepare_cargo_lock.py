#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Check, or explicitly regenerate, the M0 Rust workspace dependency lock.

Run on a developer checkout with Rust/Cargo. Never commits, resets, merges,
installs software, publishes artifacts or changes scientific datasets.
Without --apply this is a read-only report and works even without Cargo.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_PACKAGES = frozenset({"pyo3", "tempfile", "virgostars-events-python"})
_PACKAGE_RE = re.compile(r'^\[\[package\]\]\s*\nname\s*=\s*"([^"\n]+)"', re.MULTILINE)


def missing_packages(lock_path: Path) -> set[str]:
    """Find actual Cargo.lock package sections, not incidental dependency text."""
    if not lock_path.is_file():
        return set(REQUIRED_PACKAGES)
    names = set(_PACKAGE_RE.findall(lock_path.read_text(encoding="utf-8")))
    return set(REQUIRED_PACKAGES) - names


def command(args: Sequence[str], root: Path, *, run=subprocess.run) -> str:
    """Run one local command and propagate failure without claiming success."""
    result = run(list(args), cwd=root, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        details = (result.stderr or result.stdout or "").strip()
        raise RuntimeError(f"{' '.join(args)} exited {result.returncode}: {details}")
    return result.stdout


def prepare(root: Path = ROOT, *, apply: bool = False,
            which=shutil.which, run=subprocess.run) -> int:
    manifest = root / "Cargo.toml"
    binding = root / "crates" / "python-bindings" / "Cargo.toml"
    lock = root / "Cargo.lock"
    if not manifest.is_file() or not binding.is_file():
        print("FAIL: Cargo workspace or PyO3 binding manifest absent", file=sys.stderr)
        return 1

    missing = missing_packages(lock)
    if not apply:
        if missing:
            print("STALE Cargo.lock: missing package entries: " + ", ".join(sorted(missing)))
            print("On a clean developer checkout run: py -3 tools\\prepare_cargo_lock.py --apply")
            return 1
        print("Cargo.lock includes the required Python-binding packages.")
        print("This read-only check does not prove Cargo --locked or compilation.")
        return 0

    git, cargo = which("git"), which("cargo")
    if not git or not cargo:
        print("FAIL: --apply requires both Git and Cargo on PATH", file=sys.stderr)
        return 1
    try:
        # Preserve all local tracked changes, even those outside Cargo.lock.
        # Untracked developer scratch files are intentionally ignored.
        status = command([git, "status", "--porcelain=v1", "--untracked-files=no"], root, run=run)
        if status.strip():
            print("FAIL: tracked changes exist; review, commit or stash them first:\n" + status,
                  file=sys.stderr)
            return 1
        # Cargo.lock is a tracked release input: an untracked lockfile is
        # invisible to `git diff` and could otherwise be reported as verified
        # without any change available to commit or review.
        command([git, "ls-files", "--error-unmatch", "--", "Cargo.lock"], root, run=run)
        print("[M0 lock] cargo generate-lockfile", flush=True)
        command([cargo, "generate-lockfile"], root, run=run)
        missing = missing_packages(lock)
        if missing:
            raise RuntimeError("generated Cargo.lock still misses: " + ", ".join(sorted(missing)))
        print("[M0 lock] cargo metadata --locked --no-deps --format-version 1", flush=True)
        command([cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"], root, run=run)
        changed = command([git, "diff", "--name-only"], root, run=run)
        unexpected = sorted(path for path in changed.splitlines() if path != "Cargo.lock")
        if unexpected:
            raise RuntimeError("unexpected tracked changes (not reverted): " + ", ".join(unexpected))
    except (OSError, UnicodeError, RuntimeError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        print("Review git diff; this tool never resets or discards changes.", file=sys.stderr)
        return 1

    print("PASS: lockfile regenerated and Cargo metadata --locked passed.")
    print("Review git diff -- Cargo.lock, then commit the reviewed lockfile on your branch.")
    print("Next: py -3 tools\\build_python_mvp.py (native tests still required).")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true",
                        help="Regenerate Cargo.lock only after verifying a clean tracked tree")
    return prepare(apply=parser.parse_args(argv).apply)


if __name__ == "__main__":
    raise SystemExit(main())
