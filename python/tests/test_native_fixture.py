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

    def test_source_threshold_equality_and_four_lunar_stages(self) -> None:
        at_new = self.reader.get_synods_at("1970-01-01T00:00:01.500000Z")
        self.assertEqual(at_new["new_moon"]["source_id"], 12)
        self.assertEqual(at_new["new_moon"]["phase"]["code"], "CRESC1")

        at_first = self.reader.get_synods_at("1970-01-01T00:00:01.700000Z")
        moon = at_first["new_moon"]
        self.assertEqual(moon["phase"]["code"], "CRESC2")
        self.assertEqual(
            moon["phase"]["started_at_utc"], "1970-01-01T00:00:01.700000Z"
        )
        self.assertEqual(len(moon["phase_thresholds_utc"]), 3)
        self.assertEqual(moon["source_id"], 12)

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

    def test_modified_archive_after_open_is_rejected_at_query_time(self) -> None:
        # This is an independent copy: never alter the class-level fixture
        # shared by the other native end-to-end tests.
        original = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        with tempfile.TemporaryDirectory(prefix="vseph-mutate-fixture-") as folder:
            path = Path(folder) / "synods.p0"
            path.write_bytes(original)
            reader = VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)
            self.assertEqual(reader.get_synods_at("1970-01-01T00:00:01.700000Z")["new_moon"]["source_id"], 12)

            altered = bytearray(original)
            altered[-1] ^= 1  # change an unrelated byte, leaving cached TOC intact
            path.write_bytes(altered)
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                reader.get_synods_at("1970-01-01T00:00:01.700000Z")

    def test_archive_path_replacement_does_not_inherit_verified_identity(self) -> None:
        # A P0Archive stores the path, not a permanently locked snapshot.
        original = base64.b64decode(FIXTURE_BASE64.read_text(encoding="utf-8").strip())
        with tempfile.TemporaryDirectory(prefix="vseph-replace-fixture-") as folder:
            path = Path(folder) / "synods.p0"
            replacement = Path(folder) / "replacement.p0"
            path.write_bytes(original)
            reader = VirgoStarsEvents.open(path, expected_sha256=FIXTURE_SHA256)

            altered = bytearray(original)
            altered[-2] ^= 1
            replacement.write_bytes(altered)
            replacement.replace(path)
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                reader.get_synods_at("1970-01-01T00:00:01.700000Z")

            # Returning the original bytes restores the same trusted identity;
            # a failed read must not leave an invalid cached result behind.
            path.write_bytes(original)
            self.assertEqual(
                reader.get_synods_at("1970-01-01T00:00:01.700000Z")["new_moon"]["source_id"],
                12,
            )

    def test_offset_equivalence_and_digest_mismatch_fail_closed(self) -> None:
        zulu = self.reader.get_synods_at("1970-01-01T00:00:01.700000Z")
        offset = self.reader.get_synods_at("1970-01-01T01:00:01.700000+01:00")
        self.assertEqual(zulu, offset)
        with self.assertRaises(ValueError):
            VirgoStarsEvents.open(self.path, expected_sha256="0" * 64)


if __name__ == "__main__":
    unittest.main()
