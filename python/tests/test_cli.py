# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Pure-Python tests for the offline command line entrypoint.

No Rust wheel or scientific dataset is required; native integration is
verified separately by tools/validate_python_mvp.py.
"""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
import json
import unittest
from unittest.mock import patch

from virgostars_events import __main__ as cli
from virgostars_events._time import to_utc_microseconds


class FakeReader:
    def __init__(self) -> None:
        self.requested = None
        self.closed = False

    def __enter__(self):
        if self.closed:
            raise RuntimeError("reader is closed")
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.closed = True  # mimic native snapshot release

    def get_synods_at(self, instant: str) -> dict:
        to_utc_microseconds(instant)
        self.requested = instant
        moon = {
            "body": "Moon",
            "source_id": 12,
            "cycle_start_utc": "1970-01-01T00:00:00.000000Z",
            "phase": {"code": "CRESC1", "status": "derived_from_recorded_thresholds"},
            "coverage": {"status": "unreviewed"},
        }
        return {
            "requested_at_utc": instant,
            "dataset": {"sha256": "a" * 64, "coverage_certified": False},
            "synods": {"Moon": moon},
            "new_moon": moon,
        }


class CommandLineTests(unittest.TestCase):
    def test_cli_returns_json_without_inventing_new_moon(self) -> None:
        fake = FakeReader()
        stdout = StringIO()
        with patch.object(cli.VirgoStarsEvents, "open", return_value=fake) as opener:
            with redirect_stdout(stdout):
                result = cli.main(["my file.p0", "2026-11-11T12:00:00Z",
                                   "--sha256", "a" * 64])
        self.assertEqual(result, 0)
        opener.assert_called_once_with("my file.p0", expected_sha256="a" * 64)
        self.assertEqual(fake.requested, "2026-11-11T12:00:00Z")
        data = json.loads(stdout.getvalue())
        self.assertEqual(data["new_moon"], data["synods"]["Moon"])
        self.assertFalse(data["dataset"]["coverage_certified"])
        self.assertIn("\n  \"dataset\"", stdout.getvalue())

    def test_cli_closes_verified_snapshot_after_successful_json(self) -> None:
        fake = FakeReader()
        with patch.object(cli.VirgoStarsEvents, "open", return_value=fake):
            with redirect_stdout(StringIO()):
                result = cli.main([
                    "archive.p0", "2026-11-11T12:00:00Z",
                    "--sha256", "a" * 64,
                ])
        self.assertEqual(result, 0)
        self.assertTrue(fake.closed)

    def test_cli_closes_verified_snapshot_after_query_failure(self) -> None:
        class FaultyReader(FakeReader):
            def get_synods_at(self, instant):
                raise ValueError("fixture lookup failed")

        fake = FaultyReader()
        stdout, stderr = StringIO(), StringIO()
        with patch.object(cli.VirgoStarsEvents, "open", return_value=fake):
            with redirect_stdout(stdout), redirect_stderr(stderr):
                result = cli.main([
                    "archive.p0", "2026-11-11T12:00:00Z",
                    "--sha256", "b" * 64,
                ])
        self.assertEqual(result, 1)
        self.assertTrue(fake.closed)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("fixture lookup failed", stderr.getvalue())

    def test_compact_output_is_one_valid_json_line(self) -> None:
        stdout = StringIO()
        with patch.object(cli.VirgoStarsEvents, "open", return_value=FakeReader()):
            with redirect_stdout(stdout):
                status = cli.main(["archive.p0", "1970-01-01T01:30:00+01:30",
                                   "--sha256", "b" * 64, "--compact"])
        self.assertEqual(status, 0)
        self.assertEqual(len(stdout.getvalue().splitlines()), 1)
        self.assertEqual(json.loads(stdout.getvalue())["new_moon"]["source_id"], 12)

    def test_errors_do_not_print_json_or_a_traceback(self) -> None:
        for error in (OSError("missing source archive"),
                      ValueError("trusted SHA-256 mismatch")):
            stdout, stderr = StringIO(), StringIO()
            with self.subTest(error=error):
                with patch.object(cli.VirgoStarsEvents, "open", side_effect=error):
                    with redirect_stdout(stdout), redirect_stderr(stderr):
                        status = cli.main(["missing.p0", "2026-11-11T12:00:00Z",
                                           "--sha256", "0" * 64])
                self.assertEqual(status, 1)
                self.assertEqual(stdout.getvalue(), "")
                self.assertIn(str(error), stderr.getvalue())
                self.assertNotIn("Traceback", stderr.getvalue())

    def test_missing_digest_and_version_are_handled_by_argparse(self) -> None:
        with redirect_stderr(StringIO()):
            with self.assertRaises(SystemExit) as missing:
                cli.main(["archive.p0", "2026-11-11T12:00:00Z"])
        self.assertEqual(missing.exception.code, 2)
        stdout = StringIO()
        with redirect_stdout(stdout):
            with self.assertRaises(SystemExit) as version:
                cli.main(["--version"])
        self.assertEqual(version.exception.code, 0)
        self.assertIn("virgostars-events 0.1.0", stdout.getvalue())

    def test_invalid_instant_rejected_before_archive_is_opened(self) -> None:
        stdout, stderr = StringIO(), StringIO()
        with patch.object(
            cli.VirgoStarsEvents, "open",
            side_effect=AssertionError("must not open an archive"),
        ) as opener:
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = cli.main([
                    "huge.p0", "2026-11-11T12:00:00.1234567Z",
                    "--sha256", "a" * 64,
                ])
        self.assertEqual(status, 1)
        opener.assert_not_called()
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("up to 6 fractional", stderr.getvalue())

    def test_invalid_submicrosecond_input_fails_before_json(self) -> None:
        stdout, stderr = StringIO(), StringIO()
        with patch.object(cli.VirgoStarsEvents, "open", return_value=FakeReader()):
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = cli.main(["archive.p0", "2026-11-11T12:00:00.1234567Z",
                                   "--sha256", "a" * 64])
        self.assertEqual(status, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("up to 6 fractional", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
