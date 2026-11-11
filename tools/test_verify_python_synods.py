#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Pure-stdlib tests for the Python API vs source-CSV oracle.

No compiled extension, scientific dataset or PostgreSQL required.
"""
from __future__ import annotations

import csv
import json
from pathlib import Path
import tempfile
import unittest

from verify_python_synods import (
    SourceSynod, assert_parity, expected_phase, expected_row,
    instant_us, iso_utc, load_synod_csv, select_probes,
)


def synthetic_source() -> dict[str, list[SourceSynod]]:
    return {
        "Mercury": [
            SourceSynod(100, 20, (110, 120, 130)),
            SourceSynod(200, 21, (210, 220, 230)),
        ],
        "Moon": [
            SourceSynod(50, 10, None),
            SourceSynod(150, 11, (160, 170, 180)),
            SourceSynod(250, 12, (260, 270, 280)),
        ],
        "true Node": [SourceSynod(100, 30, ())],
    }


class OracleReader:
    """Test-only fake: lets the oracle test reject corrupt Python responses."""
    def __init__(self, data, mutate=None):
        self.data = data
        self.mutate = mutate

    def get_synods_at(self, iso):
        t = instant_us(iso)
        rows = {}
        for body, series in self.data.items():
            previous, following = expected_row(series, t)
            code, started_at, status = expected_phase(body, previous, following, t)
            rows[body] = {
                "body": body,
                "source_id": previous.source_id if previous else None,
                "cycle_start_utc": iso_utc(previous.at) if previous else None,
                "next_cycle_start_utc": iso_utc(following.at) if following else None,
                "phase_thresholds_utc": (
                    None if previous is None or previous.extras is None
                    else [iso_utc(v) for v in previous.extras]
                ),
                "status": "recorded" if previous else "no_preceding_recorded_cycle",
                "phase": {
                    "code": code, "started_at_utc": started_at, "status": status
                },
                "coverage": {"status": "unreviewed", "warnings": []},
            }
        moon = rows.get("Moon")
        result = {
            "requested_at_utc": iso,
            "dataset": {"sha256": "a" * 64, "coverage_certified": False},
            "synods": rows,
            "new_moon": moon if moon and moon["cycle_start_utc"] else None,
        }
        if self.mutate:
            self.mutate(result)
        return result


class SourceCsvTests(unittest.TestCase):
    def test_headered_normalized_csv_null_empty_and_ordering(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "sinodi.csv"
            with path.open("w", encoding="utf-8", newline="") as f:
                writer = csv.writer(f)
                writer.writerow(["id", "elemento", "dataora_utc", "dataora_extra_utc_json"])
                writer.writerow([21, "Moon", "1970-01-01 00:00:02+00", "[]"])
                writer.writerow([20, "Moon", "1969-12-31 23:59:59+00", ""])
                writer.writerow([30, "Mercury", "1970-01-01 00:00:00+00",
                                 json.dumps(["1970-01-01 00:00:01+00",
                                             "1970-01-01 00:00:02+00",
                                             "1970-01-01 00:00:03+00"])])
            source = load_synod_csv(path)
            self.assertEqual(sorted(source), ["Mercury", "Moon"])
            self.assertIsNone(source["Moon"][0].extras)
            self.assertEqual(source["Moon"][1].extras, ())
            self.assertEqual(source["Moon"][0].source_id, 20)
            self.assertEqual(len(source["Mercury"][0].extras), 3)

    def test_csv_duplicate_source_ids_fail_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "sinodi.csv"
            path.write_text(
                "id,elemento,dataora_utc,dataora_extra_utc_json\n"
                "1,Moon,1970-01-01 00:00:00+00,\n"
                "1,Mercury,1970-01-01 00:00:01+00,\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "repeated source row ID"):
                load_synod_csv(path)

    def test_latest_anchor_inclusive_and_before_first_is_missing(self):
        series = synthetic_source()["Moon"]
        before, nxt = expected_row(series, 49)
        self.assertIsNone(before)
        self.assertIsNone(nxt)
        at_anchor, nxt = expected_row(series, 150)
        self.assertEqual(at_anchor.source_id, 11)
        self.assertEqual(nxt.source_id, 12)
        self.assertEqual(expected_row(series, 249)[0].source_id, 11)
        self.assertIsNone(expected_row(series, 250)[1])

    def test_exact_planet_and_moon_stage_boundary_oracle(self):
        merc = synthetic_source()["Mercury"][0]
        moon = synthetic_source()["Moon"][1]
        next_merc = synthetic_source()["Mercury"][1]
        next_moon = synthetic_source()["Moon"][2]
        for at, code in ((100, "D1"), (110, "R2"), (120, "R3"), (130, "D4")):
            self.assertEqual(expected_phase("Mercury", merc, next_merc, at)[0], code)
        for at, code in ((150, "CRESC1"), (160, "CRESC2"),
                         (170, "CAL1"), (180, "CAL2")):
            self.assertEqual(expected_phase("Moon", moon, next_moon, at)[0], code)
        self.assertEqual(expected_phase("Moon", moon, None, 180)[2],
                         "unknown_open_ended_cycle")
        self.assertEqual(expected_phase("true Node",
                                         synthetic_source()["true Node"][0],
                                         None, 101)[2],
                         "unknown_unsupported_phase_convention")

    def test_complete_result_comparison_without_native_rust(self):
        source = synthetic_source()
        probes = [49, 50, 100, 110, 120, 130, 150, 160, 170, 180, 200, 249, 250]
        outcome = assert_parity(OracleReader(source), source, probes, "a" * 64)
        self.assertEqual(outcome["checked_body_instants"], 3 * len(probes))
        self.assertFalse(outcome["scientific_coverage_certified"])

    def test_wrong_source_row_id_is_detected(self):
        def bad(result):
            result["synods"]["Moon"]["source_id"] = 999
        with self.assertRaisesRegex(ValueError, "source_id"):
            assert_parity(OracleReader(synthetic_source(), bad),
                          synthetic_source(), [150], "a" * 64)

    def test_missing_body_and_unreviewed_coverage_claim_rejected(self):
        def missing(result):
            del result["synods"]["Mercury"]
        with self.assertRaisesRegex(ValueError, "body set mismatch"):
            assert_parity(OracleReader(synthetic_source(), missing),
                          synthetic_source(), [150], "a" * 64)
        def overclaimed(result):
            result["dataset"]["coverage_certified"] = True
        with self.assertRaisesRegex(ValueError, "coverage claim"):
            assert_parity(OracleReader(synthetic_source(), overclaimed),
                          synthetic_source(), [150], "a" * 64)

    def test_deterministic_probes_include_exact_lunar_thresholds(self):
        source = synthetic_source()
        probes = select_probes(source, "1970-01-01T00:00:00.000160Z")
        for t in (150, 160, 170, 180):
            self.assertIn(t, probes)
        self.assertIn(49, probes)


if __name__ == "__main__":
    unittest.main()
