# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Exact UTC/POSIX microsecond conversions for the Python binding boundary.

Conversion only: astronomical events, phases and searches remain in Rust.
"""
from __future__ import annotations

from datetime import datetime, timedelta, timezone
import re

_EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)
_OFFSET_RE = re.compile(r"(?:Z|[+-][0-9]{2}:[0-9]{2})$")


def to_utc_microseconds(instant: datetime | str) -> int:
    """Reject naive instants; avoid floating-point timestamp rounding."""
    if isinstance(instant, str):
        if "T" not in instant or not _OFFSET_RE.search(instant):
            raise ValueError("instant must be ISO-8601 with explicit Z or offset")
        try:
            instant = datetime.fromisoformat(instant.replace("Z", "+00:00"))
        except ValueError as exc:
            raise ValueError("invalid ISO-8601 instant") from exc
    if not isinstance(instant, datetime):
        raise TypeError("instant must be an aware datetime or ISO-8601 string")
    if instant.tzinfo is None or instant.utcoffset() is None:
        raise ValueError("naive datetime is not allowed; supply an explicit timezone")
    utc = instant.astimezone(timezone.utc)
    delta = utc - _EPOCH
    micros = (delta.days * 86400 + delta.seconds) * 1_000_000 + delta.microseconds
    if not -(2**63) <= micros < 2**63:
        raise OverflowError("instant outside signed i64 microsecond range")
    return micros


def iso_utc_from_microseconds(microseconds: int | None) -> str | None:
    """Convert to a UTC ISO string using integer arithmetic only."""
    if microseconds is None:
        return None
    if not isinstance(microseconds, int) or isinstance(microseconds, bool):
        raise TypeError("source timestamp must be an integer microsecond count")
    return (_EPOCH + timedelta(microseconds=microseconds)).isoformat(
        timespec="microseconds"
    ).replace("+00:00", "Z")
