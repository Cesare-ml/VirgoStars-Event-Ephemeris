# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Validate native Rust synod phase timestamps before Python ISO projection.

Only verifies the recorded wire model; stage interpretation remains in Rust.
"""
from __future__ import annotations

from typing import Any


_I64_MIN = -(1 << 63)
_I64_MAX = (1 << 63) - 1


def validate_recorded_thresholds(
    record: dict[str, Any], anchor_us: int | None, body: str
) -> list[int] | None:
    """Retain SQL NULL vs empty, but reject impossible native phase arrays.

    The Rust SynodEvent constructor requires every phase strictly after its
    anchor and the previous phase. A phase may be after the query instant or
    even the next recorded anchor: those are original source observations,
    not permission to classify an otherwise invalid/incomplete cycle.
    """
    if "phase_thresholds_us" not in record:
        raise ValueError(f"native synod phase thresholds invalid for {body}")
    extras = record["phase_thresholds_us"]
    if extras is None:
        return None
    if anchor_us is None or type(extras) is not list:
        raise ValueError(f"native synod phase thresholds invalid for {body}")
    prior = anchor_us
    for value in extras:
        if (
            type(value) is not int
            or not _I64_MIN <= value <= _I64_MAX
            or value <= prior
        ):
            raise ValueError(f"native synod phase thresholds invalid for {body}")
        prior = value
    return extras


# These are wire-level names emitted by Rust SynodStage and
# SynodStageStatus. This validates the claimed result shape only;
# computing the stage from astronomical phase times remains in Rust.
_PLANET_STAGE_CODES = frozenset({"D1", "R2", "R3", "D4"})
_MOON_STAGE_CODES = frozenset({"CRESC1", "CRESC2", "CAL1", "CAL2"})
# The code's stage-start instant must be an original recorded threshold.
# -1 means the cycle anchor itself (D1/CRESC1), not an extra phase.
# This is a wire consistency check, not an independent stage classifier.
_STAGE_START_EXTRA_INDEX = {
    "D1": -1, "R2": 0, "R3": 1, "D4": 2,
    "CRESC1": -1, "CRESC2": 0, "CAL1": 1, "CAL2": 2,
}
_PLANET_STAGE_BODIES = frozenset({
    "Mercury", "Venus", "Sun", "Mars", "Jupiter",
    "Saturn", "Uranus", "Neptune", "Pluto",
})
_UNKNOWN_STAGE_STATUSES = frozenset({
    "before_recorded_anchor",
    "at_or_after_next_recorded_anchor",
    "unknown_missing_phase_thresholds",
    "unknown_incomplete_phase_thresholds",
    "unknown_unsupported_phase_convention",
    "unknown_invalid_cycle_bounds",
    "unknown_open_ended_cycle",
    "unknown_discontinuous_moon_anchors",
})


def validate_native_stage(
    phase: dict[str, Any],
    body: str,
    anchor_us: int | None,
    thresholds: list[int] | None,
) -> None:
    """Reject inconsistent Rust phase status/code pairs, without classifying.

    A stage is known only with three recorded extra thresholds, for a body
    supported by the current Rust four-stage convention. Unknown status
    codes never imply a known stage, regardless of source timestamps.
    """
    required = ("code", "started_at_us", "status")
    if any(key not in phase for key in required):
        raise ValueError(f"native synod phase status invalid for {body}")
    code = phase["code"]
    started_at_us = phase["started_at_us"]
    status = phase["status"]
    if type(status) is not str:
        raise ValueError(f"native synod phase status invalid for {body}")

    if anchor_us is None:
        if (
            status != "no_preceding_recorded_cycle"
            or code is not None
            or started_at_us is not None
        ):
            raise ValueError(f"native synod phase status invalid for {body}")
        return

    if status == "derived_from_recorded_thresholds":
        supported = (
            _MOON_STAGE_CODES if body == "Moon"
            else _PLANET_STAGE_CODES if body in _PLANET_STAGE_BODIES
            else frozenset()
        )
        if (
            type(code) is not str
            or code not in supported
            or type(started_at_us) is not int
            or thresholds is None
            or len(thresholds) != 3
        ):
            raise ValueError(f"native synod phase status invalid for {body}")
        start_index = _STAGE_START_EXTRA_INDEX[code]
        recorded_start = anchor_us if start_index == -1 else thresholds[start_index]
        if started_at_us != recorded_start:
            raise ValueError(f"native synod phase start disagrees with thresholds for {body}")
    elif status not in _UNKNOWN_STAGE_STATUSES or code is not None or started_at_us is not None:
        raise ValueError(f"native synod phase status invalid for {body}")
