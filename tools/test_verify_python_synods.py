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
    SourceSynod, assert_parity, expected_phase, expected_row, has_moon_source_gap,
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
            warnings = ["no_reviewed_coverage_policy"]
            if has_moon_source_gap(body, previous, following):
                warnings.append("moon_recorded_cycle_gap_over_45_days")
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
                "coverage": {"status": "unreviewed", "warnings": warnings},
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

    def test_strict_utc_probe_preserves_exact_microseconds(self):
        self.assertEqual(instant_us("1970-01-01T00:00:00Z"), 0)
        self.assertEqual(instant_us("1970-01-01T00:00:00.1Z"), 100_000)
        self.assertEqual(instant_us("1970-01-01T00:00:00.123456Z"), 123_456)
        self.assertEqual(instant_us("1969-12-31T23:59:59.999999Z"), -1)
        self.assertEqual(
            iso_utc(instant_us("0002-01-01T00:00:00.000001Z")),
            "0002-01-01T00:00:00.000001Z",
        )

    def test_utc_probe_never_silently_truncates_or_normalizes(self):
        bad = (
            "1970-01-01T00:00:00.0000009Z",
            "2026-11-11T12:00:00.1234567Z",
            "2026-11-11T12:00:00.123456789Z",
            "2026-11-11T12:00:00+00:00",
            "2026-11-11T12:00:00+01:00",
            "2026-11-11 12:00:00Z",
            "2026-11-11T12:00:00Z\n",
            "2026-11-11T12:00Z",
            "2026-02-30T12:00:00Z",
            "0000-01-01T00:00:00Z",
        )
        for value in bad:
            with self.subTest(value=value), self.assertRaises(ValueError):
                instant_us(value)

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

    def test_expected_row_does_not_iterate_or_copy_large_series(self):
        class NoIteration(list):
            def __iter__(self):
                raise AssertionError("bisect must not copy all timestamps")

        series = NoIteration(SourceSynod(i * 10, i + 1, None) for i in range(20_000))
        for when, expected_id, next_id in (
            (-1, None, None),
            (0, 1, 2),
            (10, 2, 3),
            (15, 2, 3),
            (199_990, 20_000, None),
            (199_991, 20_000, None),
        ):
            with self.subTest(when=when):
                before, after = expected_row(series, when)
                self.assertEqual(before.source_id if before else None, expected_id)
                self.assertEqual(after.source_id if after else None, next_id)

    def test_expected_row_uses_logarithmic_index_lookups(self):
        class VirtualMillionEvents:
            def __init__(self):
                self.reads = 0

            def __len__(self):
                return 1_000_000

            def __getitem__(self, index):
                self.reads += 1
                return SourceSynod(index * 10, index + 1, None)

            def __iter__(self):
                raise AssertionError("a full scan is not allowed")

        series = VirtualMillionEvents()
        for when, expected_id in ((-1, None), (0, 1), (555_555, 55_556),
                                  (9_999_990, 1_000_000), (10_000_000, 1_000_000)):
            with self.subTest(when=when):
                series.reads = 0
                before, after = expected_row(series, when)
                self.assertEqual(before.source_id if before else None, expected_id)
                self.assertLessEqual(series.reads, 24)
                if before and after:
                    self.assertGreater(after.at, before.at)

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

    def test_deterministic_probes_cover_all_bodies_and_adjacent_boundaries(self):
        source = synthetic_source()
        probes = select_probes(source, "1970-01-01T00:00:00.000160Z")
        # Selected anchors: Moon=150, Mercury=100, true Node=100.
        for t in (100, 150):
            for near in (t - 1, t, t + 1):
                self.assertIn(near, probes)
        # Exactly at, immediately before, and immediately after each threshold
        # of the selected Moon/Mercury cycle.
        for t in (110, 120, 130, 160, 170, 180):
            for near in (t - 1, t, t + 1):
                self.assertIn(near, probes)
        self.assertIn(49, probes)  # before first Moon
        self.assertIn(instant_us("1919-01-01T00:00:00Z"), probes)
        self.assertIn(instant_us("2018-01-01T00:00:00Z"), probes)
        self.assertEqual(probes, sorted(set(probes)))

    def test_lunar_1919_2018_style_gap_is_unknown_not_year_long_cal2(self):
        day_us = 86_400_000_000
        last = SourceSynod(100, 44934, (110, 120, 130))
        after_gap = SourceSynod(100 + 413 * day_us, 44935, (140, 150, 160))
        moon = [last, after_gap]
        source = {"Moon": moon}
        probes = select_probes(source, "1970-01-01T00:00:00.000131Z")
        self.assertIn((last.at + after_gap.at) // 2, probes)
        self.assertIn(after_gap.at - 1, probes)
        self.assertIn(instant_us("1919-06-01T00:00:00Z"), probes)
        self.assertIn(instant_us("2018-06-01T00:00:00Z"), probes)
        self.assertTrue(has_moon_source_gap("Moon", last, after_gap))
        self.assertFalse(has_moon_source_gap("Mars", last, after_gap))
        self.assertEqual(expected_phase("Moon", last, after_gap, 120)[0], "CAL1")
        self.assertEqual(
            expected_phase("Moon", last, after_gap, 131),
            (None, None, "unknown_discontinuous_moon_anchors"),
        )
        self.assertEqual(
            expected_phase("Moon", last, after_gap, 100 + 200 * day_us)[2],
            "unknown_discontinuous_moon_anchors",
        )
        self.assertEqual(
            expected_phase("Moon", last, SourceSynod(100 + 30 * day_us, 44935, ()), 131)[0],
            "CAL2",
        )
        for when in (120, 131, 100 + 200 * day_us):
            report = assert_parity(OracleReader(source), source, [when], "a" * 64)
            self.assertEqual(report["checked_body_instants"], 1)

        def strip_warning(result):
            result["synods"]["Moon"]["coverage"]["warnings"] = [
                "no_reviewed_coverage_policy"
            ]

        with self.assertRaisesRegex(ValueError, "discontinuity warning"):
            assert_parity(
                OracleReader(source, strip_warning),
                source,
                [100 + 200 * day_us],
                "a" * 64,
            )

    def test_probes_include_non_moon_planets_and_reject_empty_series(self):
        source = {"Mars": [SourceSynod(700, 901, (710, 720, 730))]}
        probes = select_probes(source, "1970-01-01T00:00:00.000705Z")
        for t in (700, 710, 720, 730):
            for near in (t - 1, t, t + 1):
                self.assertIn(near, probes)
        with self.assertRaisesRegex(ValueError, "empty indexed body series"):
            select_probes({"Mars": []}, "1970-01-01T00:00:00Z")


if __name__ == "__main__":
    unittest.main()
