# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed local gate for the real Rust-backed Python MVP.

Run *after* pip installing the package in a virtual environment:
    python tools/validate_python_mvp.py
    python tools/validate_python_mvp.py --archive path/to/owner.p0 --sha256 <trusted-sha256>

Unlike unittest's default exit status, this gate treats SKIPPED native tests
as a validation failure. It never downloads or redistributes a dataset.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
TESTS = ROOT / "python" / "tests"


def run_tests(pattern: str, minimum_tests: int) -> bool:
    suite = unittest.defaultTestLoader.discover(
        start_dir=str(TESTS),
        pattern=pattern,
    )
    count = suite.countTestCases()
    print(f"\n[Python MVP] {pattern}: {count} discovered")
    if count < minimum_tests:
        print(
            f"FAIL: expected at least {minimum_tests} tests; found only {count}",
            file=sys.stderr,
        )
        return False
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if not result.wasSuccessful() or result.skipped:
        print(
            f"FAIL: {pattern}: failures={len(result.failures)}, "
            f"errors={len(result.errors)}, skipped={len(result.skipped)}",
            file=sys.stderr,
        )
        return False
    print(f"PASS: {pattern}: {result.testsRun}/{result.testsRun} without skips")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Require real, non-skipped Rust-backed Python synod tests"
    )
    parser.add_argument("--archive", help="Optional owner-authorized local P0 archive")
    parser.add_argument("--sha256", help="Trusted lower-case 64-digit SHA-256 for archive")
    args = parser.parse_args()

    if bool(args.archive) != bool(args.sha256):
        parser.error("--archive and --sha256 must be given together")

    try:
        import virgostars_events  # noqa: F401
        from virgostars_events._native import NativeReader  # noqa: F401
    except ImportError as exc:
        print(
            f"FAIL: compiled Rust/PyO3 module is not installed: {exc}",
            file=sys.stderr,
        )
        return 1

    if not run_tests("test_python_api.py", 8):
        return 1
    if not run_tests("test_native_fixture.py", 5):
        return 1

    if args.archive is not None:
        if not Path(args.archive).is_file():
            print("FAIL: supplied archive does not exist", file=sys.stderr)
            return 1
        os.environ["VSEPH_P0_PATH"] = str(Path(args.archive).resolve())
        os.environ["VSEPH_P0_SHA256"] = args.sha256
        if not run_tests("test_native_smoke.py", 2):
            return 1

    print("\nPASS: Python MVP validation on an installed Rust extension")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
