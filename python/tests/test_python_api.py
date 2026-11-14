# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Unit tests for the Python surface; native Rust wheel needs separate tests."""
import gc
import json
import sys
import weakref
import types
import unittest
from datetime import datetime, timedelta, timezone, tzinfo
from unittest.mock import patch

from virgostars_events import VirgoStarsEvents
from virgostars_events._time import (
    iso_utc_from_microseconds,
    to_utc_microseconds,
)


class SyntheticSpringForward(tzinfo):
    """Portable fake +01/+02 DST jump (no OS tzdata needed on Windows)."""

    def utcoffset(self, dt):
        if dt is None:
            return None
        return timedelta(
            hours=2
            if (dt.year, dt.month, dt.day, dt.hour) >= (2026, 3, 29, 3)
            else 1
        )

    def dst(self, dt):
        return self.utcoffset(dt) - timedelta(hours=1)

    def fromutc(self, dt):
        if dt.tzinfo is not self:
            raise ValueError("foreign timezone")
        utc = dt.replace(tzinfo=None)
        offset = timedelta(hours=2 if utc >= datetime(2026, 3, 29, 1) else 1)
        return (utc + offset).replace(tzinfo=self)


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

    def test_submicrosecond_iso_precision_is_rejected_not_truncated(self):
        for value in [
            "2026-11-11T12:00:00.1234567Z",
            "1970-01-01T00:00:00.0000009+00:00",
            "2026-11-11T12:00:00.123456789+01:00",
        ]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                to_utc_microseconds(value)

    def test_strict_offset_and_civil_iso_format_while_preserving_valid_input(self):
        for value in [
            "20261111T120000Z",
            "2026-11-11 12:00:00Z",
            "2026-11-11T12:00:00+01:00:01",
            "2026-11-11T12:00:00Z\\n",
            "2026-11-11T12:00:00+01:00.2",
        ]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                to_utc_microseconds(value)
        self.assertEqual(
            to_utc_microseconds("1970-01-01T01:30:00+01:30"),
            0,
        )
        self.assertEqual(
            to_utc_microseconds("1969-12-31T18:30:00-05:30"),
            0,
        )
        self.assertEqual(
            to_utc_microseconds("1970-01-01T00:00:00.1Z"),
            100_000,
        )

    def test_nonexistent_dst_local_hour_is_rejected_not_moved(self):
        zone = SyntheticSpringForward()
        with self.assertRaisesRegex(ValueError, "nonexistent local time"):
            to_utc_microseconds(datetime(2026, 3, 29, 2, 30, tzinfo=zone))
        self.assertEqual(
            to_utc_microseconds(datetime(2026, 3, 29, 1, 30, tzinfo=zone)),
            to_utc_microseconds("2026-03-29T00:30:00Z"),
        )
        self.assertEqual(
            to_utc_microseconds(datetime(2026, 3, 29, 3, 30, tzinfo=zone)),
            to_utc_microseconds("2026-03-29T01:30:00Z"),
        )

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
    raw = {
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
    # Rust serializes a value-equal Moon object (distinct JSON location).
    # Use the same raw mapping here so mutation tests stay consistent.
    raw["new_moon"] = raw["synods"]["Moon"]
    return raw


class PythonApiTests(unittest.TestCase):
    def test_native_body_and_source_id_match_recorded_cycle(self):
        instant = "1970-01-01T00:00:00Z"
        for body_value in ("Mars", "", None, 123, True):
            with self.subTest(body=body_value):
                raw = fake_response()
                raw["synods"]["Mercury"]["body"] = body_value
                with self.assertRaisesRegex(ValueError, "native synod body identity mismatch"):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(instant)

        # A recorded cycle must come from an actual positive source row ID.
        for source_id in (None, False, True, 0, -1, "27", 27.0, 1 << 64, 1 << 128):
            with self.subTest(source_id=source_id):
                raw = fake_response()
                raw["synods"]["Mercury"]["source_id"] = source_id
                with self.assertRaisesRegex(ValueError, "native synod source ID inconsistent"):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(instant)

        # Rust serializes source IDs as u64. Its maximum value remains a
        # valid original identity; values beyond it cannot come from Rust.
        raw = fake_response()
        raw["synods"]["Mercury"]["source_id"] = (1 << 64) - 1
        accepted = VirgoStarsEvents(DummyNative(raw)).get_synods_at(instant)
        self.assertEqual(accepted["synods"]["Mercury"]["source_id"], (1 << 64) - 1)

        # Without a preceding recorded cycle, there is no source row ID.
        raw = fake_response()
        raw["synods"]["Mercury"]["cycle_start_us"] = None
        with self.assertRaisesRegex(ValueError, "native synod source ID inconsistent"):
            VirgoStarsEvents(DummyNative(raw)).get_synods_at(instant)

        raw["synods"]["Mercury"]["source_id"] = None
        raw["synods"]["Mercury"]["next_cycle_start_us"] = None
        raw["synods"]["Mercury"]["phase_thresholds_us"] = None
        raw["synods"]["Mercury"]["phase"] = {
            "code": None,
            "started_at_us": None,
            "status": "no_preceding_recorded_cycle",
        }
        accepted = VirgoStarsEvents(DummyNative(raw)).get_synods_at(instant)
        self.assertIsNone(accepted["synods"]["Mercury"]["source_id"])
        self.assertIsNone(accepted["synods"]["Mercury"]["cycle_start_utc"])
        self.assertEqual(accepted["synods"]["Mercury"]["status"], "no_preceding_recorded_cycle")

    def test_native_cycle_boundaries_must_enclose_the_requested_instant(self):
        # A precomputed reader must never report future events as the
        # already current cycle, or a next anchor <= the requested instant.
        for body, field, invalid in (
            ("Mercury", "cycle_start_us", 1),
            ("Mercury", "cycle_start_us", True),
            ("Mercury", "cycle_start_us", "-300"),
            ("Mercury", "cycle_start_us", -300.0),
            ("Mercury", "cycle_start_us", []),
            ("Mercury", "cycle_start_us", {}),
            ("Moon", "next_cycle_start_us", 0),
            ("Moon", "next_cycle_start_us", -1),
            ("Moon", "next_cycle_start_us", False),
            ("Moon", "next_cycle_start_us", "2000000"),
        ):
            with self.subTest(body=body, field=field, invalid=invalid):
                response = fake_response()
                response["synods"][body][field] = invalid
                with self.assertRaisesRegex(
                    ValueError, "native synod (current|next) anchor invalid"
                ):
                    VirgoStarsEvents(DummyNative(response)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        # The native stage assessment is current only when its source start
        # is a genuine integer in [cycle_start, requested_at]. A boolean,
        # floating-point number, absent start or future threshold cannot
        # silently be presented as a recorded current phase.
        for code, started in (
            ("R3", 1),
            ("R3", -301),
            ("R3", True),
            ("R3", -100.0),
            ("R3", "-100"),
            ("R3", None),
            (None, -100),
        ):
            with self.subTest(phase_code=code, started_at_us=started):
                response = fake_response()
                response["synods"]["Mercury"]["phase"]["code"] = code
                response["synods"]["Mercury"]["phase"]["started_at_us"] = started
                with self.assertRaisesRegex(ValueError, "native synod phase start invalid"):
                    VirgoStarsEvents(DummyNative(response)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        missing_anchor = fake_response()
        missing_anchor["synods"]["Mercury"]["source_id"] = None
        missing_anchor["synods"]["Mercury"]["cycle_start_us"] = None
        with self.assertRaisesRegex(ValueError, "native synod phase start invalid"):
            VirgoStarsEvents(DummyNative(missing_anchor)).get_synods_at(
                "1970-01-01T00:00:00Z"
            )

        # A real phase start exactly at its anchor remains valid.
        at_stage_start = fake_response()
        at_stage_start["synods"]["Mercury"]["phase"]["code"] = "D1"
        at_stage_start["synods"]["Mercury"]["phase"]["started_at_us"] = -300
        accepted = VirgoStarsEvents(DummyNative(at_stage_start)).get_synods_at(
            "1970-01-01T00:00:00Z"
        )
        self.assertEqual(
            accepted["synods"]["Mercury"]["phase"]["started_at_utc"],
            "1969-12-31T23:59:59.999700Z",
        )

        # The current anchor is inclusive at X, while the *next* anchor
        # must be strictly later than X. Both cases are common boundaries.
        at_boundary = fake_response()
        at_boundary["synods"]["Mercury"]["cycle_start_us"] = 0
        at_boundary["synods"]["Mercury"]["next_cycle_start_us"] = 1
        # With a one-microsecond cycle, do not keep thresholds from the
        # original -300us fixture: they predate this new synthetic anchor.
        at_boundary["synods"]["Mercury"]["phase_thresholds_us"] = None
        at_boundary["synods"]["Mercury"]["phase"] = {
            "code": None,
            "started_at_us": None,
            "status": "unknown_missing_phase_thresholds",
        }
        result = VirgoStarsEvents(DummyNative(at_boundary)).get_synods_at(
            "1970-01-01T00:00:00Z"
        )
        self.assertEqual(
            result["synods"]["Mercury"]["cycle_start_utc"],
            "1970-01-01T00:00:00.000000Z",
        )
        self.assertEqual(
            result["synods"]["Mercury"]["next_cycle_start_utc"],
            "1970-01-01T00:00:00.000001Z",
        )

        # An indexed body with no preceding event remains a legitimate
        # missing state, not a fabricated future cycle.
        missing = fake_response()
        missing["synods"]["Moon"]["source_id"] = None
        missing["synods"]["Moon"]["cycle_start_us"] = None
        missing["synods"]["Moon"]["next_cycle_start_us"] = None
        # NativeReader serializes null, not a Moon record, when no recorded
        # lunar anchor precedes the requested instant. Keep the fixture
        # consistent with the fail-closed transport contract.
        missing["new_moon"] = None
        result = VirgoStarsEvents(DummyNative(missing)).get_synods_at(
            "1970-01-01T00:00:00Z"
        )
        self.assertIsNone(result["new_moon"])
        self.assertIsNone(result["synods"]["Moon"]["cycle_start_utc"])

    def test_native_requested_instant_must_match_exact_microseconds(self):
        requested = "1970-01-01T01:00:00.000001+01:00"
        for native_us in (None, True, False, "1", -1, 0, 2, 1.0):
            with self.subTest(native_us=native_us):
                raw = fake_response()
                raw["requested_at_us"] = native_us
                native = DummyNative(raw)
                with self.assertRaisesRegex(ValueError, "native synod response instant"):
                    VirgoStarsEvents(native).get_synods_at(requested)
                self.assertEqual(native.received, 1)
        for payload in ([], None, "invalid", 42):
            with self.subTest(payload=payload):
                with self.assertRaisesRegex(ValueError, "JSON object"):
                    VirgoStarsEvents(DummyNative(payload)).get_synods_at(requested)

        missing = fake_response()
        del missing["requested_at_us"]
        with self.assertRaisesRegex(ValueError, "native synod response instant"):
            VirgoStarsEvents(DummyNative(missing)).get_synods_at(requested)

        exact = fake_response()
        exact["requested_at_us"] = 1
        native = DummyNative(exact)
        response = VirgoStarsEvents(native).get_synods_at(requested)
        self.assertEqual(response["requested_at_utc"], "1970-01-01T00:00:00.000001Z")
        self.assertNotIn("requested_at_us", response)
        self.assertEqual(native.received, 1)

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

        # Native Rust emits standards-compliant JSON. Reject ambiguous wire
        # payloads instead of letting Python choose the last duplicate key or
        # accepting its permissive NaN/Infinity JSON extensions.
        class RawWireNative:
            def __init__(self, payload):
                self.payload = payload

            def get_synods_at_us(self, requested_us):
                return self.payload

        canonical = json.dumps(fake_response())
        malformed_wire = (
            (
                canonical.replace(
                    '"requested_at_us": 0',
                    '"requested_at_us": 0, "requested_at_us": 0',
                    1,
                ),
                "duplicate JSON key",
            ),
            (
                canonical.replace(
                    '"source_id": 27',
                    '"source_id": 27, "source_id": 27',
                    1,
                ),
                "duplicate JSON key",
            ),
            (
                canonical.replace(
                    '"synods": {',
                    '"synods": {}, "\\u0073ynods": {',
                    1,
                ),
                "duplicate JSON key",
            ),
            *(
                (
                    canonical.replace('"source_id": 27', f'"source_id": {token}', 1),
                    "non-finite JSON value",
                )
                for token in ("NaN", "Infinity", "-Infinity")
            ),
        )
        for payload, error in malformed_wire:
            with self.subTest(error=error, payload=payload), self.assertRaisesRegex(
                ValueError, error
            ):
                VirgoStarsEvents(RawWireNative(payload)).get_synods_at(
                    "1970-01-01T00:00:00Z"
                )
        for invalid_wire in (None, b"{}", 42):
            with self.subTest(wire=invalid_wire), self.assertRaisesRegex(
                ValueError, "JSON text"
            ):
                VirgoStarsEvents(RawWireNative(invalid_wire)).get_synods_at(
                    "1970-01-01T00:00:00Z"
                )

        # The wrapper must not quietly repair a contradictory native Moon
        # alias. Both omitted and value-divergent native fields are invalid.
        for corruption in ("missing", "null", "wrong-id", "unrelated"):
            with self.subTest(corruption=corruption):
                raw = fake_response()
                if corruption == "missing":
                    del raw["new_moon"]
                elif corruption == "null":
                    raw["new_moon"] = None
                elif corruption == "wrong-id":
                    raw["new_moon"] = dict(raw["synods"]["Moon"], source_id=999)
                else:
                    raw["new_moon"] = raw["synods"]["Mercury"]
                with self.assertRaisesRegex(ValueError, "new_moon disagrees"):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        for malformed_synods in (None, [], {"Moon": None}):
            with self.subTest(malformed_synods=malformed_synods):
                raw = fake_response()
                raw["synods"] = malformed_synods
                with self.assertRaisesRegex(ValueError, "synods object of records"):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        # Native dataset identity is part of the response contract.
        # Reject a plausible source row if the native transport omits or
        # changes the format, ID, SHA-256, or coverage-certification claim.
        for corrupted in (
            "missing dataset", "wrong shape", "wrong format", "missing format",
            "wrong id", "missing id", "certified", "missing sha", "upper sha",
            "short sha", "boolean sha",
        ):
            with self.subTest(corrupted=corrupted):
                raw = fake_response()
                if corrupted == "missing dataset":
                    del raw["dataset"]
                elif corrupted == "wrong shape":
                    raw["dataset"] = []
                elif corrupted == "wrong format":
                    raw["dataset"]["format"] = "VSEPH-v1"
                elif corrupted == "missing format":
                    del raw["dataset"]["format"]
                elif corrupted == "wrong id":
                    raw["dataset"]["id"] = "other-archive"
                elif corrupted == "missing id":
                    del raw["dataset"]["id"]
                elif corrupted == "certified":
                    raw["dataset"]["coverage_certified"] = True
                elif corrupted == "missing sha":
                    del raw["dataset"]["sha256"]
                elif corrupted == "upper sha":
                    raw["dataset"]["sha256"] = "A" * 64
                elif corrupted == "short sha":
                    raw["dataset"]["sha256"] = "a" * 63
                elif corrupted == "boolean sha":
                    raw["dataset"]["sha256"] = False
                with self.assertRaisesRegex(ValueError, "dataset identity is invalid"):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        # When opened with a trusted independent hash, the *returned* native
        # identity must also agree, not just have a valid 64-character shape.
        with self.assertRaisesRegex(ValueError, "dataset identity is invalid"):
            VirgoStarsEvents(
                DummyNative(fake_response()), expected_sha256="b" * 64
            ).get_synods_at("1970-01-01T00:00:00Z")

        # A P0 archive has no scientifically reviewed body coverage.
        # Native transport may add gap warnings, but must not omit the
        # mandatory unreviewed-policy warning or fabricate certification.
        for altered in (
            "missing coverage", "non-object coverage", "certified status",
            "missing warning", "non-list warnings", "missing phase object",
        ):
            with self.subTest(altered=altered):
                raw = fake_response()
                record = raw["synods"]["Mercury"]
                if altered == "missing coverage":
                    del record["coverage"]
                elif altered == "non-object coverage":
                    record["coverage"] = "unreviewed"
                elif altered == "certified status":
                    record["coverage"]["status"] = "certified"
                elif altered == "missing warning":
                    record["coverage"]["warnings"] = []
                elif altered == "non-list warnings":
                    record["coverage"]["warnings"] = "no_reviewed_coverage_policy"
                elif altered == "missing phase object":
                    record["phase"] = None
                expected = (
                    "phase must be an object" if altered == "missing phase object"
                    else "coverage claim invalid"
                )
                with self.assertRaisesRegex(ValueError, expected):
                    VirgoStarsEvents(DummyNative(raw)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        # Extra factual gap warnings are allowed without changing the
        # mandatory disclosure that source coverage is unreviewed.
        with_warnings = fake_response()
        with_warnings["synods"]["Mercury"]["coverage"]["warnings"].append(
            "independently_observed_source_gap"
        )
        accepted = VirgoStarsEvents(DummyNative(with_warnings)).get_synods_at(
            "1970-01-01T00:00:00Z"
        )
        self.assertIn(
            "independently_observed_source_gap",
            accepted["synods"]["Mercury"]["coverage"]["warnings"],
        )

    def test_empty_extra_array_not_confused_with_sql_null(self):
        raw = fake_response()
        raw["synods"]["Moon"]["phase_thresholds_us"] = []
        data = VirgoStarsEvents(DummyNative(raw)).get_synods_at("1970-01-01T00:00:00Z")
        self.assertEqual(data["synods"]["Moon"]["phase_thresholds_utc"], [])

        # Preserve source observations, but never accept invented, reversed,
        # duplicated or non-integer phase timestamps from the native boundary.
        for invalid in (
            12, "100", {}, True, [True], [-100.0], ["-100"],
            [None], [-300], [-100, -100], [-100, -101],
            [(1 << 63)], [-(1 << 63) - 1],
        ):
            with self.subTest(invalid=invalid):
                broken = fake_response()
                broken["synods"]["Mercury"]["phase_thresholds_us"] = invalid
                with self.assertRaisesRegex(
                    ValueError, "native synod phase thresholds invalid"
                ):
                    VirgoStarsEvents(DummyNative(broken)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        for omitted in ("missing field", "missing anchor"):
            with self.subTest(omitted=omitted):
                broken = fake_response()
                if omitted == "missing field":
                    del broken["synods"]["Mercury"]["phase_thresholds_us"]
                else:
                    broken["synods"]["Moon"]["source_id"] = None
                    broken["synods"]["Moon"]["cycle_start_us"] = None
                    broken["synods"]["Moon"]["phase_thresholds_us"] = []
                    broken["new_moon"] = None
                with self.assertRaisesRegex(
                    ValueError, "native synod phase thresholds invalid"
                ):
                    VirgoStarsEvents(DummyNative(broken)).get_synods_at(
                        "1970-01-01T00:00:00Z"
                    )

        # An original phase timestamp after X is recorded data, not a claim
        # that the associated future phase has already begun.
        future = fake_response()
        future["synods"]["Mercury"]["phase_thresholds_us"] = [1, 2, 3]
        future["synods"]["Mercury"]["phase"] = {
            "code": "D1", "started_at_us": -300,
            "status": "derived_from_recorded_thresholds",
        }
        returned = VirgoStarsEvents(DummyNative(future)).get_synods_at(
            "1970-01-01T00:00:00Z"
        )
        self.assertEqual(
            returned["synods"]["Mercury"]["phase_thresholds_utc"],
            [
                "1970-01-01T00:00:00.000001Z",
                "1970-01-01T00:00:00.000002Z",
                "1970-01-01T00:00:00.000003Z",
            ],
        )

    def test_missing_previous_moon_anchor_returns_none_alias(self):
        raw = fake_response()
        raw["synods"]["Moon"]["source_id"] = None
        raw["synods"]["Moon"]["cycle_start_us"] = None
        raw["synods"]["Moon"]["phase"]["status"] = "no_preceding_recorded_cycle"
        raw["new_moon"] = None  # Native P0 emits null when no Moon anchor precedes X.
        data = VirgoStarsEvents(DummyNative(raw)).get_synods_at("1970-01-01T00:00:00Z")
        self.assertIsNone(data["new_moon"])
        self.assertEqual(
            data["synods"]["Moon"]["status"], "no_preceding_recorded_cycle"
        )

    def test_context_manager_closes_and_releases_native_reader(self):
        native = DummyNative(fake_response())
        native_ref = weakref.ref(native)
        events = VirgoStarsEvents(native)
        del native  # the wrapper must be the only live native owner
        self.assertFalse(events.closed)
        with events as opened:
            self.assertIs(opened, events)
            self.assertEqual(
                opened.get_synods_at("1970-01-01T00:00:00Z")["new_moon"]["source_id"],
                11,
            )
        self.assertTrue(events.closed)
        gc.collect()
        self.assertIsNone(native_ref())  # no retained private snapshot
        with self.assertRaisesRegex(RuntimeError, "reader is closed"):
            events.get_synods_at("1970-01-01T00:00:00Z")

    def test_close_idempotent_and_closed_reader_cannot_be_reentered(self):
        events = VirgoStarsEvents(DummyNative(fake_response()))
        self.assertFalse(events.closed)
        events.close()
        events.close()
        self.assertTrue(events.closed)
        with self.assertRaisesRegex(RuntimeError, "reader is closed"):
            events.get_synods_at("not-even-an-instant")
        with self.assertRaisesRegex(RuntimeError, "reader is closed"):
            with events:
                self.fail("closed context must not run")

    def test_context_exit_closes_reader_without_suppressing_errors(self):
        events = VirgoStarsEvents(DummyNative(fake_response()))
        with self.assertRaisesRegex(ValueError, "example failure"):
            with events:
                raise ValueError("example failure")
        self.assertTrue(events.closed)

    def test_invalid_sha256_never_constructs_a_native_reader(self):
        class FakeReader:
            def __init__(self, *_args):
                raise AssertionError("native file I/O must not start")

        module = types.ModuleType("virgostars_events._native")
        module.NativeReader = FakeReader
        with patch.dict(sys.modules, {"virgostars_events._native": module}):
            for digest in (None, "", "abc", "g" * 64, "f" * 63, "f" * 65):
                with self.subTest(digest=digest), self.assertRaisesRegex(
                    ValueError, "64-digit hexadecimal"
                ):
                    VirgoStarsEvents.open("missing.p0", expected_sha256=digest)

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
        self.assertEqual(reader._trusted_sha256, "b" * 64)
        self.assertEqual(requests, [("local.p0", "b" * 64)])
        with patch.dict(sys.modules, {"virgostars_events._native": module}):
            upper_reader = VirgoStarsEvents.open("upper.p0", expected_sha256="A" * 64)
        self.assertEqual(requests[-1], ("upper.p0", "a" * 64))
        self.assertEqual(upper_reader._trusted_sha256, "a" * 64)


if __name__ == "__main__":
    unittest.main()
