# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Sanity-check unknown Rust synod stage reasons at the Python wire boundary.

These constraints check recorded metadata only; the Rust engine remains the
sole source of stage classification. They do not certify coverage.
"""
from __future__ import annotations

# Mirror the provisional Rust recorded-Moon discontinuity threshold ONLY for
# checking native reason codes. This does not reconstruct missing lunations.
MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US = 45 * 86_400 * 1_000_000

_PLANETS = frozenset({
    "Mercury", "Venus", "Sun", "Mars", "Jupiter",
    "Saturn", "Uranus", "Neptune", "Pluto",
})


def validate_unknown_stage_reason(
    status: str,
    body: str,
    thresholds: list[int] | None,
    next_anchor_us: int | None,
    anchor_us: int,
    requested_at_us: int,
) -> None:
    """Refuse unknown reasons contradicted by their own recorded metadata.

    The public query has already verified that its current anchor is <= X
    and the next anchor (if known) is > X. Therefore Rust's out-of-cycle
    reasons are not valid wire responses to this particular query API.
    """
    supported = body == "Moon" or body in _PLANETS
    invalid = (
        status in {"before_recorded_anchor", "at_or_after_next_recorded_anchor"}
        or (status == "unknown_missing_phase_thresholds" and (not supported or thresholds is not None))
        or (status == "unknown_incomplete_phase_thresholds" and (
            not supported or thresholds is None or len(thresholds) == 3
        ))
        or (status == "unknown_unsupported_phase_convention" and supported)
        or (status == "unknown_open_ended_cycle" and (
            not supported or thresholds is None or len(thresholds) != 3
            or next_anchor_us is not None
            or (thresholds is not None and len(thresholds) == 3
                and requested_at_us < thresholds[2])
        ))
        or (status == "unknown_discontinuous_moon_anchors" and (
            body != "Moon" or thresholds is None or len(thresholds) != 3
            or next_anchor_us is None
            or (next_anchor_us is not None and
                next_anchor_us - anchor_us <= MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US)
            or (thresholds is not None and len(thresholds) == 3 and (
                requested_at_us < thresholds[2]
                or (next_anchor_us is not None and thresholds[2] >= next_anchor_us)
            ))
        ))
        or (status == "unknown_invalid_cycle_bounds" and (
            not supported or thresholds is None or len(thresholds) != 3
            or next_anchor_us is None
            or (thresholds is not None and len(thresholds) == 3 and
                next_anchor_us is not None and thresholds[2] < next_anchor_us)
        ))
    )
    if invalid:
        raise ValueError(f"native synod unknown phase reason inconsistent with records for {body}")
