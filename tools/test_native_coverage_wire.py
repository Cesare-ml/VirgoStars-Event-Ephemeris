# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Synthetic native coverage transport regressions; no Cargo or dataset."""
from __future__ import annotations

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
from virgostars_events._coverage_wire import validate_native_coverage
from virgostars_events._unknown_status import MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US

BASE = "no_reviewed_coverage_policy"
GAP = "moon_recorded_cycle_gap_over_45_days"


def native_coverage(*warnings: str) -> dict:
    return {"status": "unreviewed", "warnings": [BASE, *warnings]}


class NativeCoverageWireTests(unittest.TestCase):
    def test_normal_moon_record_interval_does_not_need_gap_warning(self):
        validate_native_coverage(native_coverage(), "Moon", 100, 200)

    def test_normal_planet_record_interval_does_not_need_gap_warning(self):
        validate_native_coverage(native_coverage(), "Mars", 100, 200)

    def test_exact_45_day_moon_interval_is_contiguous_by_provisional_policy(self):
        validate_native_coverage(native_coverage(), "Moon", 0, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US)

    def test_over_45_day_moon_interval_requires_gap_warning(self):
        with self.assertRaisesRegex(ValueError, "Moon gap warning inconsistent"):
            validate_native_coverage(native_coverage(), "Moon", 0, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)
        validate_native_coverage(native_coverage(GAP), "Moon", 0, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_spurious_gap_warning_rejected_for_planets(self):
        with self.assertRaisesRegex(ValueError, "Moon gap warning inconsistent"):
            validate_native_coverage(native_coverage(GAP), "Mercury", 0, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_spurious_gap_warning_rejected_for_short_moon_cycle(self):
        with self.assertRaisesRegex(ValueError, "Moon gap warning inconsistent"):
            validate_native_coverage(native_coverage(GAP), "Moon", 0, MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US)

    def test_spurious_gap_warning_rejected_without_next_or_current_anchor(self):
        for current, next_anchor in ((None, 200), (100, None), (None, None)):
            with self.subTest(current=current, next_anchor=next_anchor):
                with self.assertRaisesRegex(ValueError, "Moon gap warning inconsistent"):
                    validate_native_coverage(native_coverage(GAP), "Moon", current, next_anchor)

    def test_moon_gap_supports_pre_epoch_anchors_without_float_rounding(self):
        start = -10_000_000
        validate_native_coverage(native_coverage(GAP), "Moon", start, start + MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_reject_missing_or_nonstandard_base_coverage(self):
        for value in (None, {}, {"status": "reviewed", "warnings": [BASE]},
                      {"status": "unreviewed", "warnings": []},
                      {"status": "unreviewed", "warnings": [GAP]}):
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, "coverage claim invalid"):
                    validate_native_coverage(value, "Mars", 0, 100)

    def test_reject_malformed_duplicate_or_non_string_warnings(self):
        for warnings in (BASE, None, {}, [BASE, BASE], [BASE, None], [BASE, True], [BASE, ""], [BASE, 13]):
            with self.subTest(warnings=warnings):
                with self.assertRaisesRegex(ValueError, "coverage claim invalid"):
                    validate_native_coverage({"status": "unreviewed", "warnings": warnings}, "Moon", 0, 100)

    def test_additional_factual_warning_is_preserved(self):
        validate_native_coverage(native_coverage("independently_observed_source_gap"), "Mars", 0, 100)
        validate_native_coverage(native_coverage(GAP, "independently_observed_source_gap"), "Moon", 0,
                                 MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_native_warning_list_must_be_a_real_list(self):
        with self.assertRaisesRegex(ValueError, "coverage claim invalid"):
            validate_native_coverage({"status": "unreviewed", "warnings": (BASE,)}, "Moon", 0, 100)


if __name__ == "__main__":
    unittest.main()
