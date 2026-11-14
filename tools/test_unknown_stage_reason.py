# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Recorded-only synthetic contract regressions; no native wheel required."""
from __future__ import annotations

import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
from virgostars_events._unknown_status import validate_unknown_stage_reason, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US


def check_unknown(status, body, extras, next_us, *, anchor_us=0, requested_at_us=40):
    return validate_unknown_stage_reason(
        status, body, extras, next_us, anchor_us, requested_at_us
    )


class UnknownStageReasonTests(unittest.TestCase):
    def check_invalid(self, status: str, body: str, extras, next_us) -> None:
        with self.assertRaisesRegex(ValueError, "unknown phase reason inconsistent"):
            check_unknown(status, body, extras, next_us)

    def test_missing_threshold_reason_requires_null_extras(self):
        for extras in ([], [10], [10, 20, 30]):
            with self.subTest(extras=extras):
                self.check_invalid("unknown_missing_phase_thresholds", "Moon", extras, 90)
        check_unknown("unknown_missing_phase_thresholds", "Moon", None, 90)

    def test_incomplete_threshold_reason_requires_nonstandard_length(self):
        for extras in (None, [10, 20, 30]):
            with self.subTest(extras=extras):
                self.check_invalid("unknown_incomplete_phase_thresholds", "Mars", extras, 90)
        for extras in ([], [10], [10, 20], [10, 20, 30, 40]):
            with self.subTest(valid_extras=extras):
                check_unknown("unknown_incomplete_phase_thresholds", "Mars", extras, 90)

    def test_unsupported_reason_requires_unsupported_body(self):
        for body in ("Moon", "Mercury", "Jupiter", "Sun"):
            with self.subTest(body=body):
                self.check_invalid("unknown_unsupported_phase_convention", body, None, 90)
        check_unknown("unknown_unsupported_phase_convention", "North Node", None, 90)

    def test_unsupported_body_cannot_claim_missing_or_incomplete(self):
        self.check_invalid("unknown_missing_phase_thresholds", "North Node", None, 90)
        self.check_invalid("unknown_incomplete_phase_thresholds", "North Node", [], 90)

    def test_open_ended_reason_requires_three_extras_and_no_next_anchor(self):
        for extras, next_us in ((None, None), ([], None), ([10, 20], None), ([10, 20, 30], 90)):
            with self.subTest(extras=extras, next_us=next_us):
                self.check_invalid("unknown_open_ended_cycle", "Moon", extras, next_us)
        check_unknown("unknown_open_ended_cycle", "Moon", [10, 20, 30], None)

    def test_open_ended_reason_not_allowed_for_unsupported_body(self):
        self.check_invalid("unknown_open_ended_cycle", "North Node", [10, 20, 30], None)

    def test_discontinuous_moon_reason_requires_known_next_and_three_extras(self):
        for body, extras, next_us in (
            ("Mars", [10, 20, 30], 200),
            ("Moon", [10, 20, 30], None),
            ("Moon", [], 200),
            ("Moon", None, 200),
        ):
            with self.subTest(body=body, extras=extras, next_us=next_us):
                self.check_invalid("unknown_discontinuous_moon_anchors", body, extras, next_us)
        check_unknown("unknown_discontinuous_moon_anchors", "Moon", [10, 20, 30],
                      MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_public_query_rejects_out_of_cycle_reasons(self):
        for status in ("before_recorded_anchor", "at_or_after_next_recorded_anchor"):
            with self.subTest(status=status):
                self.check_invalid(status, "Moon", [10, 20, 30], 200)

    def test_invalid_cycle_bounds_remains_unknown_without_inference(self):
        check_unknown("unknown_invalid_cycle_bounds", "Moon", [10, 20, 30], 20,
                      requested_at_us=10)

    def test_open_end_reason_requires_last_threshold_already_reached(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_open_ended_cycle", "Moon", [10, 20, 30], None,
                          requested_at_us=29)
        check_unknown("unknown_open_ended_cycle", "Moon", [10, 20, 30], None,
                      requested_at_us=30)

    def test_moon_discontinuity_45_day_boundary_is_strict(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_discontinuous_moon_anchors", "Moon", [10, 20, 30],
                          MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US)
        check_unknown("unknown_discontinuous_moon_anchors", "Moon", [10, 20, 30],
                      MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_moon_discontinuity_is_not_an_early_phase(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_discontinuous_moon_anchors", "Moon", [10, 20, 30],
                          MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1,
                          requested_at_us=29)

    def test_moon_discontinuity_rejects_source_last_phase_after_next_anchor(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_discontinuous_moon_anchors", "Moon",
                          [10, 20, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 2],
                          MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_invalid_bounds_reason_requires_the_third_threshold_to_cross_next(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_invalid_cycle_bounds", "Moon", [10, 20, 30], 100)
        check_unknown("unknown_invalid_cycle_bounds", "Moon", [10, 20, 30], 30,
                      requested_at_us=25)

    def test_invalid_bounds_reason_cannot_come_from_unsupported_body(self):
        with self.assertRaisesRegex(ValueError, "reason inconsistent"):
            check_unknown("unknown_invalid_cycle_bounds", "North Node", [10, 20, 30], 30,
                          requested_at_us=25)


if __name__ == "__main__":
    unittest.main()
