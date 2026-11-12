#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Run the three real source-only Python MVP suites, without Rust or pip.

This developer preflight is deliberately NOT the 50-case native/wheel
acceptance gate. The mandatory native fixture, archive integrity and original
CSV/P0 parity must still pass via validate_python_mvp.py after wheel install.
"""
from __future__ import annotations

import importlib
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
# The suites here use mocked readers or an independent synthetic CSV oracle.
# The nine genuinely compiled native tests belong ONLY to the separate gate.
SOURCE_SUITES = (
    ("test_python_api.py", 17, "python/tests"),
    ("test_cli.py", 8, "python/tests"),
    ("test_verify_python_synods.py", 16, "tools"),
)


def run_suite(pattern: str, required: int, directory: Path) -> bool:
    """Never count skipped, expected-failed, missing or failed source tests as a pass."""
    suite = unittest.defaultTestLoader.discover(
        start_dir=str(directory), pattern=pattern
    )
    found = suite.countTestCases()
    print(f"\n[Python sources] {pattern}: {found} discovered (required {required})", flush=True)
    if found != required:
        print(
            f"FAIL: source-suite count changed: {pattern}: {found} != {required}. "
            "Update the source-only and native gates together.",
            file=sys.stderr,
        )
        return False
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    # unittest reports "OK (expected failures=N)" as wasSuccessful().
    # A pending/expected-failing test is NOT a passed MVP acceptance case.
    if (
        not result.wasSuccessful()
        or result.skipped
        or result.expectedFailures
        or result.unexpectedSuccesses
        or result.testsRun != required
    ):
        print(
            f"FAIL: {pattern}: ran={result.testsRun}, "
            f"failures={len(result.failures)}, errors={len(result.errors)}, "
            f"skipped={len(result.skipped)}, "
            f"expected_failures={len(result.expectedFailures)}, "
            f"unexpected_successes={len(result.unexpectedSuccesses)}",
            file=sys.stderr,
        )
        return False
    return True


def main() -> int:
    python_root = (ROOT / "python").resolve()
    tools_root = (ROOT / "tools").resolve()
    if not (python_root / "virgostars_events" / "__init__.py").is_file():
        print("FAIL: run from a complete source checkout", file=sys.stderr)
        return 1

    # Avoid accidentally accepting a separately pip-installed package instead
    # of the *current checkout*, without requiring PYTHONPATH or a venv.
    sys.path.insert(0, str(python_root))
    sys.path.insert(0, str(tools_root))
    try:
        importlib.invalidate_caches()
        from virgostars_events import __file__ as package_file
        if package_file is None or not Path(package_file).resolve().is_relative_to(python_root):
            print("FAIL: the package was imported outside this checkout", file=sys.stderr)
            return 1
        for pattern, required, relative_dir in SOURCE_SUITES:
            if not run_suite(pattern, required, ROOT / relative_dir):
                return 1
    except (ImportError, OSError, SyntaxError) as exc:
        print(f"FAIL: Python source-only preflight: {exc}", file=sys.stderr)
        return 1
    finally:
        sys.path.remove(str(tools_root))
        sys.path.remove(str(python_root))

    passed = sum(expected for _, expected, _ in SOURCE_SUITES)
    print(f"\nPASS: {passed}/{passed} source-only tests. "
          "Native wheel and owner-data parity NOT validated.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
