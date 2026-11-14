#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed, no-Rust preflight of wheel packaging and install orchestration.

Run before locked Cargo and maturin, so a packaging policy regression cannot
reach a native build or an apparently releasable local dist/ wheel.
This does not install a wheel and is not native/scientific acceptance.
"""
from __future__ import annotations

from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).resolve().parent
# Baseline minimums: additions run automatically, removals must be reviewed.
REQUIRED_SUITES = (
    ("test_wheel_contents_policy.py", 16),
    ("test_wheel_identity.py", 9),
    ("test_verify_python_wheel.py", 12),
)


def run_suite(pattern: str, minimum: int, *, directory: Path = TOOLS) -> bool:
    """Run ALL discovered tests; never accept skips or partial execution."""
    suite = unittest.defaultTestLoader.discover(
        start_dir=str(directory), pattern=pattern
    )
    discovered = suite.countTestCases()
    print(f"[wheel preflight] {pattern}: {discovered} discovered (minimum {minimum})",
          flush=True)
    if discovered < minimum:
        print(
            f"FAIL: wheel preflight test count {discovered} below {minimum} for {pattern}",
            file=sys.stderr,
        )
        return False
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if (
        not result.wasSuccessful()
        or result.testsRun != discovered
        or result.skipped
        or result.expectedFailures
        or result.unexpectedSuccesses
    ):
        print(
            f"FAIL: wheel preflight {pattern}: discovered={discovered}, "
            f"ran={result.testsRun}, failures={len(result.failures)}, "
            f"errors={len(result.errors)}, skipped={len(result.skipped)}, "
            f"expected_failures={len(result.expectedFailures)}, "
            f"unexpected_successes={len(result.unexpectedSuccesses)}",
            file=sys.stderr,
        )
        return False
    return True


def main() -> int:
    if any(not (TOOLS / name).is_file() for name, _ in REQUIRED_SUITES):
        print("FAIL: mandatory wheel policy test source missing", file=sys.stderr)
        return 1
    for pattern, minimum in REQUIRED_SUITES:
        if not run_suite(pattern, minimum):
            return 1
    print("PASS: wheel packaging and installation orchestration preflight. "
          "Native wheel, Cargo and scientific parity NOT tested.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
