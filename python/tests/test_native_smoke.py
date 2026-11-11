# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Optional integration smoke tests. Requires a compiled wheel and local data.

Set VSEPH_P0_PATH and VSEPH_P0_SHA256 only after assembling an authorized
owner snapshot. These tests must not download or bundle scientific data.
"""
import json
import os
import unittest

try:
    from virgostars_events import VirgoStarsEvents
    from virgostars_events._native import NativeReader
    _BINDINGS_AVAILABLE = True
except ImportError:
    _BINDINGS_AVAILABLE = False

_ARCHIVE = os.environ.get("VSEPH_P0_PATH")
_SHA256 = os.environ.get("VSEPH_P0_SHA256")


@unittest.skipUnless(_BINDINGS_AVAILABLE and _ARCHIVE and _SHA256,
                     "requires compiled PyO3 wheel and explicit trusted P0 dataset")
class NativeSmokeTests(unittest.TestCase):
    def test_latest_synods_new_moon_and_phase_codes_are_json_compatible(self):
        reader = VirgoStarsEvents.open(_ARCHIVE, expected_sha256=_SHA256)
        result = reader.get_synods_at("2026-11-11T12:00:00Z")
        self.assertEqual(result["dataset"]["sha256"], _SHA256.lower())
        self.assertFalse(result["dataset"]["coverage_certified"])
        self.assertIn("Moon", result["synods"])
        self.assertIs(result["new_moon"], result["synods"]["Moon"])
        moon = result["new_moon"]
        self.assertIsNotNone(moon["source_id"])
        self.assertLessEqual(moon["cycle_start_utc"], result["requested_at_utc"])
        self.assertIn(moon["phase"]["code"], [None, "CRESC1", "CRESC2", "CAL1", "CAL2"])
        self.assertEqual(moon["coverage"]["status"], "unreviewed")
        json.dumps(result)

    def test_known_source_moon_gaps_do_not_fabricate_a_year_long_phase(self):
        reader = VirgoStarsEvents.open(_ARCHIVE, expected_sha256=_SHA256)
        # The authorized 2026-11-10 owner snapshot has no Moon anchors
        # in calendar years 1919 or 2018; next recorded anchors are in
        # 1920/2019 respectively. Do not classify those years as CAL2.
        for instant, source_id in [
            ("1919-06-01T00:00:00Z", 44934),
            ("2018-06-01T00:00:00Z", 46146),
        ]:
            with self.subTest(instant=instant):
                result = reader.get_synods_at(instant)
                moon = result["new_moon"]
                self.assertIsNotNone(moon)
                self.assertEqual(moon["source_id"], source_id)
                self.assertIsNone(moon["phase"]["code"])
                self.assertEqual(
                    moon["phase"]["status"],
                    "unknown_discontinuous_moon_anchors",
                )
                self.assertIn(
                    "moon_recorded_cycle_gap_over_45_days",
                    moon["coverage"]["warnings"],
                )
                self.assertFalse(result["dataset"]["coverage_certified"])

    def test_explicit_timezone_offset_preserves_identical_source_records(self):
        reader = VirgoStarsEvents.open(_ARCHIVE, expected_sha256=_SHA256)
        utc = reader.get_synods_at("2026-11-11T12:00:00.123456Z")
        offset = reader.get_synods_at("2026-11-11T13:00:00.123456+01:00")
        self.assertEqual(utc, offset)


if __name__ == "__main__":
    unittest.main()
