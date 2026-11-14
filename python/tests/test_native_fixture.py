# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""End-to-end Rust/Python tests on an independently authored SYNTHETIC P0 file.

Unlike test_python_api.py, these tests use the compiled PyO3 extension.
They do NOT depend on the private owner dataset, PostgreSQL, or a network.
They intentionally SKIP if the native wheel is not yet installed; a skip
must never be reported as a passing native integration test.
"""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from virgostars_events import VirgoStarsEvents
from virgostars_events._time import iso_utc_from_microseconds

try:
    from virgostars_events._native import NativeReader  # noqa: F401
except ImportError:
    NATIVE_AVAILABLE = False
else:
    NATIVE_AVAILABLE = True

FIXTURE_SHA256 = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499"
FIXTURE_BASE64 = (
    Path(__file__).resolve().parents[2]
    / "crates" / "events-p0" / "tests" / "fixtures" / "synthetic.p0.base64"
)


@unittest.skipUnless(
    NATIVE_AVAILABLE,
    "requires compiled PyO3 wheel: these are native, not mocked tests",
)
class GoldenNativeIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        # A fixture with no owner-scientific data, authored independently
        # in Python and checked against the Rust golden test's digest.
        raw = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA256:
            raise AssertionError("independent synthetic P0 bytes changed")
        cls._temp = tempfile.TemporaryDirectory(prefix="vseph-native-fixture-")
        cls.path = Path(cls._temp.name) / "synthetic.p0"
        cls.path.write_bytes(raw)
        cls.reader = VirgoStarsEvents.open(cls.path, expected_sha256=FIXTURE_SHA256)

    @classmethod
    def tearDownClass(cls) -> None:
        cls._temp.cleanup()

    def test_exact_previous_new_moon_and_null_source_extras(self) -> None:
        result = self.reader.get_synods_at("1969-12-31T23:59:59.123456Z")
        self.assertEqual(sorted(result["synods"]), ["Mercury", "Moon"])
        self.assertIsNone(result["synods"]["Mercury"]["cycle_start_utc"])
        self.assertEqual(result["synods"]["Mercury"]["status"], "no_preceding_recorded_cycle")
        moon = result["new_moon"]
        self.assertIs(moon, result["synods"]["Moon"])
        self.assertEqual(moon["source_id"], 10)
        self.assertEqual(moon["cycle_start_utc"], "1969-12-31T23:59:59.123456Z")
        self.assertEqual(moon["next_cycle_start_utc"], "1970-01-01T00:00:01.500000Z")
        self.assertIsNone(moon["phase_thresholds_utc"])  # SQL NULL
        self.assertIsNone(moon["phase"]["code"])
        self.assertEqual(moon["phase"]["status"], "unknown_missing_phase_thresholds")
        self.assertFalse(result["dataset"]["coverage_certified"])
        self.assertEqual(result["dataset"]["sha256"], FIXTURE_SHA256)
        json.dumps(result)

    def test_before_first_new_moon_returns_no_fabricated_cycle(self) -> None:
        result = self.reader.get_synods_at("1969-12-31T23:59:59.123455Z")
        self.assertIsNone(result["new_moon"])
        self.assertIsNone(result["synods"]["Moon"]["source_id"])
        self.assertEqual(
            result["synods"]["Moon"]["status"],
            "no_preceding_recorded_cycle",
        )

    def test_lunar_phase_microsecond_boundaries_and_unknown_last_stage(self) -> None:
        # The independently authored golden P0 contains Moon source ID 10
        # at -876544 us (SQL NULL extras), then ID 12 at 1_500_000 us
        # with original recorded phase thresholds 1_700_000, 1_800_000
        # and 1_900_000 us. There is NO following recorded Moon anchor.
        # Therefore the final CAL2 segment must remain unknown; it is not
        # scientifically valid to extend it indefinitely.
        derived = "derived_from_recorded_thresholds"
        cases = (
            # instant_us, source_id, code, started_at_us, phase_status
            (1_499_999, 10, None, None, "unknown_missing_phase_thresholds"),
            (1_500_000, 12, "CRESC1", 1_500_000, derived),
            (1_500_001, 12, "CRESC1", 1_500_000, derived),
            (1_699_999, 12, "CRESC1", 1_500_000, derived),
            (1_700_000, 12, "CRESC2", 1_700_000, derived),
            (1_700_001, 12, "CRESC2", 1_700_000, derived),
            (1_799_999, 12, "CRESC2", 1_700_000, derived),
            (1_800_000, 12, "CAL1", 1_800_000, derived),
            (1_800_001, 12, "CAL1", 1_800_000, derived),
            (1_899_999, 12, "CAL1", 1_800_000, derived),
            (1_900_000, 12, None, None, "unknown_open_ended_cycle"),
            (1_900_001, 12, None, None, "unknown_open_ended_cycle"),
            (2_000_000, 12, None, None, "unknown_open_ended_cycle"),
        )
        expected_thresholds = [
            iso_utc_from_microseconds(us)
            for us in (1_700_000, 1_800_000, 1_900_000)
        ]
        for instant_us, source_id, code, started_us, status in cases:
            with self.subTest(instant_us=instant_us):
                response = self.reader.get_synods_at(
                    iso_utc_from_microseconds(instant_us)
                )
                moon = response["new_moon"]
                self.assertIs(moon, response["synods"]["Moon"])
                self.assertEqual(moon["source_id"], source_id)
                self.assertEqual(moon["phase"]["code"], code)
                self.assertEqual(
                    moon["phase"]["started_at_utc"],
                    iso_utc_from_microseconds(started_us),
                )
                self.assertEqual(moon["phase"]["status"], status)
                if source_id == 12:
                    self.assertEqual(
                        moon["phase_thresholds_utc"], expected_thresholds
                    )
                    self.assertIsNone(moon["next_cycle_start_utc"])
                else:
                    self.assertIsNone(moon["phase_thresholds_utc"])
                    self.assertEqual(
                        moon["next_cycle_start_utc"],
                        iso_utc_from_microseconds(1_500_000),
                    )

    def test_empty_mercury_extras_preserved_and_phase_unknown(self) -> None:
        result = self.reader.get_synods_at("1970-01-01T00:00:02Z")
        mercury = result["synods"]["Mercury"]
        self.assertEqual(mercury["source_id"], 21)
        self.assertEqual(mercury["phase_thresholds_utc"], [])  # SQL []
        self.assertIsNone(mercury["phase"]["code"])
        self.assertEqual(mercury["phase"]["status"], "unknown_incomplete_phase_thresholds")

    def test_python_module_cli_queries_the_real_native_golden_reader(self) -> None:
        import subprocess
        import sys

        command = [
            sys.executable,
            "-m",
            "virgostars_events",
            str(self.path),
            "1970-01-01T00:00:01.700000Z",
            "--sha256",
            FIXTURE_SHA256,
            "--compact",
        ]
        completed = subprocess.run(
            command, capture_output=True, text=True, check=False
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(len(completed.stdout.splitlines()), 1)
        result = json.loads(completed.stdout)
        self.assertEqual(result["new_moon"]["source_id"], 12)
        self.assertEqual(result["new_moon"]["phase"]["code"], "CRESC2")
        self.assertFalse(result["dataset"]["coverage_certified"])

        wrong_sha = command.copy()
        wrong_sha[wrong_sha.index(FIXTURE_SHA256)] = "0" * 64
        bad = subprocess.run(wrong_sha, capture_output=True, text=True, check=False)
        self.assertNotEqual(bad.returncode, 0)
        self.assertEqual(bad.stdout, "")
        self.assertIn("sha", bad.stderr.lower())

    def test_source_in_place_edit_cannot_mutate_reader_snapshot(self) -> None:
        # A query must return only the bytes verified at reader construction.
        original = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        instant = "1970-01-01T00:00:01.700000Z"
        with tempfile.TemporaryDirectory(prefix="vseph-mutate-fixture-") as folder:
            path = Path(folder) / "synods.p0"
            path.write_bytes(original)
            reader = VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)
            before = reader.get_synods_at(instant)
            self.assertEqual(before["new_moon"]["source_id"], 12)

            altered = bytearray(original)
            altered[-1] ^= 1  # original source is not the reader's private copy
            path.write_bytes(altered)
            self.assertEqual(reader.get_synods_at(instant), before)
            self.assertEqual(reader.get_synods_at(instant)["dataset"]["sha256"], FIXTURE_SHA256)
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)

    def test_source_path_replacement_cannot_mutate_reader_snapshot(self) -> None:
        original = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        instant = "1970-01-01T00:00:01.700000Z"
        with tempfile.TemporaryDirectory(prefix="vseph-replace-fixture-") as folder:
            path = Path(folder) / "synods.p0"
            replacement = Path(folder) / "replacement.p0"
            path.write_bytes(original)
            reader = VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)
            before = reader.get_synods_at(instant)

            altered = bytearray(original)
            altered[-2] ^= 1
            replacement.write_bytes(altered)
            replacement.replace(path)
            self.assertEqual(reader.get_synods_at(instant), before)
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)

            path.write_bytes(original)
            self.assertEqual(reader.get_synods_at(instant), before)
            self.assertEqual(
                VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)
                .get_synods_at(instant),
                before,
            )

    def test_source_path_deletion_does_not_break_open_reader(self) -> None:
        original = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        instant = "1970-01-01T00:00:01.700000Z"
        with tempfile.TemporaryDirectory(prefix="vseph-delete-fixture-") as folder:
            path = Path(folder) / "synods.p0"
            path.write_bytes(original)
            reader = VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)
            before = reader.get_synods_at(instant)
            path.unlink()
            self.assertEqual(reader.get_synods_at(instant), before)
            with self.assertRaises(OSError):
                VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)

    def test_offset_equivalence_and_digest_mismatch_fail_closed(self) -> None:
        zulu = self.reader.get_synods_at("1970-01-01T00:00:01.700000Z")
        offset = self.reader.get_synods_at("1970-01-01T01:00:01.700000+01:00")
        self.assertEqual(zulu, offset)
        with self.assertRaises(ValueError):
            VirgoStarsEvents.open(self.path, expected_sha256="0" * 64)

        # Test the directly importable PyO3 class, bypassing the public
        # Python facade. Malformed trust material must fail BEFORE a path
        # is opened (missing-path OSError would prove the wrong order).
        absent = str(self.path.with_name("not-present.p0"))
        for malformed in ("", "a" * 63, "a" * 65, "G" * 64, "A" * 64, "0" * 63 + "\n"):
            with self.subTest(sha=malformed):
                with self.assertRaisesRegex(ValueError, "64 lower-case hex"):
                    NativeReader(absent, malformed)

        # An otherwise valid SHA passes the format gate and therefore
        # exposes the actual missing-source error, not a digest error.
        with self.assertRaises(OSError):
            NativeReader(absent, FIXTURE_SHA256)


if __name__ == "__main__":
    unittest.main()
