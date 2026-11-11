# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Unit tests for the Python surface; native Rust wheel needs separate tests."""
import json
import sys
import types
import unittest
from datetime import datetime, timedelta, timezone
from unittest.mock import patch

from virgostars_events import VirgoStarsEvents
from virgostars_events._time import (
    iso_utc_from_microseconds,
    to_utc_microseconds,
)


class TimestampTests(unittest.TestCase):
    def test_explicit_utc_offset_and_microseconds_match(self):
        x = "2026-11-11T12:00:00.123456Z"
        y = "2026-11-11T13:00:00.123456+01:00"
        self.assertEqual(to_utc_microseconds(x), to_utc_microseconds(y))
        self.assertEqual(
            iso_utc_from_microseconds(to_utc_microseconds(x)),
            "2026-11-11T12:00:00.123456Z",
        )

    def test_timezone_aware_datetime_and_pre_epoch_round_trip(self):
        aware = datetime(1969, 12, 31, 23, 59, 59, 999999, tzinfo=timezone.utc)
        self.assertEqual(to_utc_microseconds(aware), -1)
        self.assertEqual(iso_utc_from_microseconds(-1), "1969-12-31T23:59:59.999999Z")
        self.assertEqual(
            to_utc_microseconds(datetime(1970, 1, 1, 1, tzinfo=timezone(timedelta(hours=1)))),
            0,
        )

    def test_naive_dates_invalid_offsets_and_non_dates_rejected(self):
        for value in ["2026-11-11T12:00:00", "2026-11-11", "not-a-date"]:
            with self.assertRaises(ValueError):
                to_utc_microseconds(value)
        with self.assertRaises(ValueError):
            to_utc_microseconds(datetime(2026, 11, 11, 12))
        with self.assertRaises(TypeError):
            to_utc_microseconds(42)
        with self.assertRaises(TypeError):
            iso_utc_from_microseconds(True)

    def test_no_float_timestamp_precision_loss_for_year_0002(self):
        source = "0002-01-01T00:00:00.000001Z"
        self.assertEqual(iso_utc_from_microseconds(to_utc_microseconds(source)), source)


class DummyNative:
    def __init__(self, raw):
        self.raw = raw
        self.received = None

    def get_synods_at_us(self, when):
        self.received = when
        return json.dumps(self.raw)


def fake_response():
    return {
        "requested_at_us": 0,
        "dataset": {"id": None, "sha256": "a" * 64, "format": "VSEPH-P0", "coverage_certified": False},
        "synods": {
            "Mercury": {
                "body": "Mercury",
                "source_id": 27,
                "cycle_start_us": -300,
                "next_cycle_start_us": 400,
                "phase_thresholds_us": [-200, -100, 100],
                "phase": {
                    "code": "R3", "started_at_us": -100,
                    "status": "derived_from_recorded_thresholds"
                },
                "coverage": {"status": "unreviewed", "warnings": ["no_reviewed_coverage_policy"]},
            },
            "Moon": {
                "body": "Moon",
                "source_id": 11,
                "cycle_start_us": -2000,
                "next_cycle_start_us": 2_000_000,
                "phase_thresholds_us": None,
                "phase": {
                    "code": None, "started_at_us": None,
                    "status": "unknown_missing_phase_thresholds"
                },
                "coverage": {"status": "unreviewed", "warnings": ["no_reviewed_coverage_policy"]},
            },
        },
        "new_moon": None,
    }


class PythonApiTests(unittest.TestCase):
    def test_response_is_json_serializable_and_new_moon_same_object(self):
        native = DummyNative(fake_response())
        data = VirgoStarsEvents(native).get_synods_at("1970-01-01T00:00:00Z")
        self.assertEqual(native.received, 0)
        self.assertEqual(data["requested_at_utc"], "1970-01-01T00:00:00.000000Z")
        self.assertEqual(data["synods"]["Mercury"]["phase"]["code"], "R3")
        self.assertEqual(data["synods"]["Mercury"]["phase"]["started_at_utc"], "1969-12-31T23:59:59.999900Z")
        self.assertIsNone(data["synods"]["Moon"]["phase_thresholds_utc"])
        self.assertIs(data["new_moon"], data["synods"]["Moon"])
        self.assertFalse(data["dataset"]["coverage_certified"])
        json.dumps(data)

    def test_empty_extra_array_not_confused_with_sql_null(self):
        raw = fake_response()
        raw["synods"]["Moon"]["phase_thresholds_us"] = []
        data = VirgoStarsEvents(DummyNative(raw)).get_synods_at("1970-01-01T00:00:00Z")
        self.assertEqual(data["synods"]["Moon"]["phase_thresholds_utc"], [])

    def test_missing_previous_moon_anchor_returns_none_alias(self):
        raw = fake_response()
        raw["synods"]["Moon"]["source_id"] = None
        raw["synods"]["Moon"]["cycle_start_us"] = None
        raw["synods"]["Moon"]["phase"]["status"] = "no_preceding_recorded_cycle"
        data = VirgoStarsEvents(DummyNative(raw)).get_synods_at("1970-01-01T00:00:00Z")
        self.assertIsNone(data["new_moon"])
        self.assertEqual(
            data["synods"]["Moon"]["status"], "no_preceding_recorded_cycle"
        )

    def test_open_forwards_path_and_digest_to_native_extension(self):
        requests = []

        class FakeReader:
            def __init__(self, path, expected_sha256):
                requests.append((path, expected_sha256))

        module = types.ModuleType("virgostars_events._native")
        module.NativeReader = FakeReader
        with patch.dict(sys.modules, {"virgostars_events._native": module}):
            reader = VirgoStarsEvents.open("local.p0", expected_sha256="b" * 64)
        self.assertIsInstance(reader._reader, FakeReader)
        self.assertEqual(requests, [("local.p0", "b" * 64)])


if __name__ == "__main__":
    unittest.main()
