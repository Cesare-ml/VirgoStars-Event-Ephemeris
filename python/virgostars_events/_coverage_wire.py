# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Check native Rust coverage warnings against recorded source anchors.

This checks transport consistency, not scientific completeness. The Rust
reader remains authoritative and source data are never repaired or invented.
"""
from __future__ import annotations

from typing import Any

from ._unknown_status import MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US

_BASE_WARNING = "no_reviewed_coverage_policy"
_MOON_GAP_WARNING = "moon_recorded_cycle_gap_over_45_days"


def validate_native_coverage(
    coverage: Any,
    body: str,
    current_anchor_us: int | None,
    next_anchor_us: int | None,
) -> None:
    """Require Rust coverage disclosures to agree with recorded anchors.

    Other nonempty string warnings remain extensible; only the existing
    mandatory disclosure and recorded-Moon-gap warning are authoritative.
    The public facade validates timestamp types and ordering before calling.
    """
    if not isinstance(coverage, dict) or coverage.get("status") != "unreviewed":
        raise ValueError(f"native synod coverage claim invalid for {body}")
    warnings = coverage.get("warnings")
    if (
        type(warnings) is not list
        or any(type(item) is not str or not item for item in warnings)
        or len(warnings) != len(set(warnings))
        or _BASE_WARNING not in warnings
    ):
        raise ValueError(f"native synod coverage claim invalid for {body}")

    recorded_moon_gap = (
        body == "Moon"
        and current_anchor_us is not None
        and next_anchor_us is not None
        and next_anchor_us - current_anchor_us > MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US
    )
    if (_MOON_GAP_WARNING in warnings) != recorded_moon_gap:
        raise ValueError(f"native synod Moon gap warning inconsistent with source for {body}")
