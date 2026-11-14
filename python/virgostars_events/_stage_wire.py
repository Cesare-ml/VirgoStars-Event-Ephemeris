# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Verify the Rust synod stage status/code transport contract.

No phases are calculated here. The Rust classifier remains authoritative;
this guard only rejects impossible labels in the JSON response.
"""
from __future__ import annotations

from typing import Any

_MOON_CODES = frozenset({"CRESC1", "CRESC2", "CAL1", "CAL2"})
_PLANET_CODES = frozenset({"D1", "R2", "R3", "D4"})
_PLANET_BODIES = frozenset({
    "Mercury", "Venus", "Sun", "Mars", "Jupiter", "Saturn",
    "Uranus", "Neptune", "Pluto",
})
_UNKNOWN_STATUSES = frozenset({
    "before_recorded_anchor",
    "at_or_after_next_recorded_anchor",
    "unknown_missing_phase_thresholds",
    "unknown_incomplete_phase_thresholds",
    "unknown_unsupported_phase_convention",
    "unknown_invalid_cycle_bounds",
    "unknown_open_ended_cycle",
    "unknown_discontinuous_moon_anchors",
})
_DERIVED = "derived_from_recorded_thresholds"
_NO_PREDECESSOR = "no_preceding_recorded_cycle"


def validate_stage_status(phase: dict[str, Any], body: str, cycle_start_us: int | None) -> None:
    """Reject invalid Rust stage labels without inferring missing events."""
    status = phase.get("status")
    code = phase.get("code")
    if type(status) is not str:
        raise ValueError(f"native synod phase status invalid for {body}")
    if cycle_start_us is None:
        if status != _NO_PREDECESSOR or code is not None:
            raise ValueError(f"native synod phase status invalid for {body}")
        return
    if code is None:
        if status not in _UNKNOWN_STATUSES:
            raise ValueError(f"native synod phase status invalid for {body}")
        return
    allowed = _MOON_CODES if body == "Moon" else (
        _PLANET_CODES if body in _PLANET_BODIES else frozenset()
    )
    if type(code) is not str or code not in allowed or status != _DERIVED:
        raise ValueError(f"native synod phase status invalid for {body}")
