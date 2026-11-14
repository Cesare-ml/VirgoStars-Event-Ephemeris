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
