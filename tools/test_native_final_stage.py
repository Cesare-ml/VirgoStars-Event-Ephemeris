# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed synthetic regressions for Rust-derived terminal stage wire metadata.

No Cargo, wheel, real owner dataset or astronomical extrapolation is involved.
"""
from __future__ import annotations

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
from virgostars_events._phase_wire import validate_native_stage
from virgostars_events._unknown_status import MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US


def known(code: str, started_at_us: int) -> dict:
    return {"code": code, "started_at_us": started_at_us,
            "status": "derived_from_recorded_thresholds"}


class NativeTerminalStageTests(unittest.TestCase):
    def test_planet_terminal_stage_requires_known_next_anchor(self):
        with self.assertRaisesRegex(ValueError, "final stage lacks next recorded anchor"):
            validate_native_stage(known("D4", 30), "Mercury", 0, [10, 20, 30], 31, None)

    def test_moon_terminal_stage_requires_known_next_anchor(self):
        with self.assertRaisesRegex(ValueError, "final stage lacks next recorded anchor"):
            validate_native_stage(known("CAL2", 30), "Moon", 0, [10, 20, 30], 31, None)

    def test_terminal_stage_allowed_with_contiguous_known_next_anchor(self):
        validate_native_stage(known("D4", 30), "Mercury", 0, [10, 20, 30], 31, 40)
        validate_native_stage(known("CAL2", 30), "Moon", 0, [10, 20, 30], 31, 40)

    def test_long_moon_gap_must_not_be_called_cal2(self):
        with self.assertRaisesRegex(ValueError, "lunar final stage crosses source gap"):
            validate_native_stage(known("CAL2", 30), "Moon", 0, [10, 20, 30], 31,
                                  MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_moon_gap_threshold_equality_is_not_rejected(self):
        validate_native_stage(known("CAL2", 30), "Moon", 0, [10, 20, 30], 31,
                              MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US)

    def test_early_stage_rejects_last_phase_at_next_anchor(self):
        with self.assertRaisesRegex(ValueError, "phase bounds conflict with next anchor"):
            validate_native_stage(known("D1", 0), "Venus", 0, [10, 20, 100], 5, 100)

    def test_early_stage_rejects_last_phase_after_next_anchor(self):
        with self.assertRaisesRegex(ValueError, "phase bounds conflict with next anchor"):
            validate_native_stage(known("CRESC1", 0), "Moon", 0, [10, 20, 100], 5, 50)

    def test_early_moon_phase_survives_long_gap_without_inventing_cal2(self):
        validate_native_stage(known("CRESC1", 0), "Moon", 0, [10, 20, 30], 5,
                              MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US + 1)

    def test_last_phase_boundary_exactly_inclusive_with_valid_next(self):
        validate_native_stage(known("D4", 30), "Mars", 0, [10, 20, 30], 30, 40)
        validate_native_stage(known("CAL2", 30), "Moon", 0, [10, 20, 30], 30, 40)

    def test_unknown_open_ended_status_remains_valid(self):
        validate_native_stage(
            {"code": None, "started_at_us": None, "status": "unknown_open_ended_cycle"},
            "Mercury", 0, [10, 20, 30], 31, None,
        )


if __name__ == "__main__":
    unittest.main()
