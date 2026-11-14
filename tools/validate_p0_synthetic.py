#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Require every independent synthetic P0 byte-audit regression to pass.

The separately authorized owner snapshot test is deliberately excluded: it
belongs to the optional real-data gate, never to the portable synthetic gate.
No Rust toolchain, network, or original dataset is required.
"""
from __future__ import annotations

import sys
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
OPTIONAL_OWNER_TEST = "test_real_snapshot_full_block_audit"
REQUIRED_SYNTHETIC = 14


def run_synthetic(cls: type[unittest.TestCase]) -> bool:
    names = unittest.defaultTestLoader.getTestCaseNames(cls)
    if OPTIONAL_OWNER_TEST not in names:
        print("FAIL: owner-data audit test missing from source suite", file=sys.stderr)
        return False
    selected = [name for name in names if name != OPTIONAL_OWNER_TEST]
    if len(selected) != REQUIRED_SYNTHETIC:
        print(
            f"FAIL: independent P0 synthetic audit count: "
            f"{len(selected)} != {REQUIRED_SYNTHETIC}",
            file=sys.stderr,
        )
        return False
    suite = unittest.TestSuite(cls(name) for name in selected)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if (
        not result.wasSuccessful()
        or result.testsRun != REQUIRED_SYNTHETIC
        or result.skipped
        or result.expectedFailures
        or result.unexpectedSuccesses
    ):
        print(
            f"FAIL: P0 synthetic audit: ran={result.testsRun}, "
            f"errors={len(result.errors)}, failures={len(result.failures)}, "
            f"skipped={len(result.skipped)}, "
            f"expected_failures={len(result.expectedFailures)}, "
            f"unexpected_successes={len(result.unexpectedSuccesses)}",
            file=sys.stderr,
        )
        return False
    print(f"PASS: {REQUIRED_SYNTHETIC}/{REQUIRED_SYNTHETIC} independent synthetic P0 audit tests")
    return True


def main() -> int:
    tools_root = ROOT / "tools"
    fixture = ROOT / "crates" / "events-p0" / "tests" / "fixtures" / "synthetic.p0.base64"
    if not fixture.is_file():
        print(f"FAIL: missing synthetic P0 fixture: {fixture}", file=sys.stderr)
        return 1
    sys.path.insert(0, str(tools_root))
    try:
        from test_audit_p0_bytes import AuditP0BytesTest
        return 0 if run_synthetic(AuditP0BytesTest) else 1
    except (ImportError, OSError, SyntaxError, ValueError) as exc:
        print(f"FAIL: P0 audit preflight: {exc}", file=sys.stderr)
        return 1
    finally:
        sys.path.remove(str(tools_root))


if __name__ == "__main__":
    raise SystemExit(main())
