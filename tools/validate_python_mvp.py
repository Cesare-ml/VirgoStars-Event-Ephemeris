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
import re
from pathlib import Path
import sys
import unittest
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]
TESTS = ROOT / "python" / "tests"
TOOLS_TESTS = ROOT / "tools"


def run_tests(pattern: str, minimum_tests: int, test_dir: Path = TESTS) -> bool:
    suite = unittest.defaultTestLoader.discover(
        start_dir=str(test_dir),
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


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Require real, non-skipped Rust-backed Python synod tests"
    )
    parser.add_argument("--archive", help="Optional owner-authorized local P0 archive")
    parser.add_argument("--sha256", help="Trusted lower-case 64-digit SHA-256 for archive")
    parser.add_argument("--synods-csv", help="Optional authorized normalized sinodi.csv for API parity")
    args = parser.parse_args(argv)

    # Reject invalid owner-data arguments BEFORE importing PyO3 or spending
    # time running the mandatory 44 tests. Never silently normalize a
    # potentially mistyped digest: the original snapshot identity is trusted.
    if bool(args.archive) != bool(args.sha256):
        parser.error("--archive and --sha256 must be given together")
    if args.synods_csv and not args.archive:
        parser.error("--synods-csv requires --archive and --sha256")
    if args.sha256 is not None and re.fullmatch(r"[0-9a-f]{64}", args.sha256) is None:
        parser.error("--sha256 must be exactly 64 lowercase hexadecimal characters")
    if args.archive and not Path(args.archive).is_file():
        parser.error("--archive must name an existing local file")
    if args.synods_csv and not Path(args.synods_csv).is_file():
        parser.error("--synods-csv must name an existing local CSV file")

    try:
        import virgostars_events  # noqa: F401
        from virgostars_events._native import NativeReader  # noqa: F401
    except ImportError as exc:
        print(
            f"FAIL: compiled Rust/PyO3 module is not installed: {exc}",
            file=sys.stderr,
        )
        return 1

    # Explicit close()/with cleanup must release the native per-reader snapshot.
    if not run_tests("test_python_api.py", 16):
        return 1
    # CLI source tests validate safe JSON output, required trusted digest,
    # timestamp precision and usable nonzero exit status for invalid input.
    if not run_tests("test_cli.py", 8):
        return 1
    # This suite independently checks original source-CSV normalization,
    # exact phase boundaries, duplicate IDs and rejection of altered API
    # results. It needs neither owner data nor a network connection.
    if not run_tests("test_verify_python_synods.py", 16, TOOLS_TESTS):
        return 1
    # The native reader owns a SHA-verified private P0 snapshot. Source-file
    # edits, path replacement or deletion must never change its results.
    if not run_tests("test_native_fixture.py", 9):
        return 1

    if args.archive is not None:
        os.environ["VSEPH_P0_PATH"] = str(Path(args.archive).resolve())
        os.environ["VSEPH_P0_SHA256"] = args.sha256
        if not run_tests("test_native_smoke.py", 3):
            return 1

    if args.synods_csv:
        try:
            from verify_python_synods import assert_parity, load_synod_csv, select_probes
            from virgostars_events import VirgoStarsEvents

            source = load_synod_csv(Path(args.synods_csv))
            # An owner-data parity sweep can perform many queries; release
            # its private P0 snapshot on both success and comparison failure.
            with VirgoStarsEvents.open(args.archive, expected_sha256=args.sha256) as reader:
                probes = select_probes(source, "2026-11-11T12:00:00Z")
                report = assert_parity(reader, source, probes, args.sha256)
            print(
                "\n[Python MVP source parity] "
                f"{report['checked_body_instants']} body/instant checks PASS"
            )
        except (OSError, ValueError, TypeError, KeyError, OverflowError) as exc:
            print(f"FAIL: native Python/CSV parity: {exc}", file=sys.stderr)
            return 1

    print("\nPASS: Python MVP validation on an installed Rust extension")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
