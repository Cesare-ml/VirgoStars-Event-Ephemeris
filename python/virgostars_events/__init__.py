# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Offline VirgoStars Event Ephemeris (experimental VSEPH-P0 Python binding).

The binary reader and synod stage classifier live exclusively in Rust.
This layer handles input UTC datetimes and JSON-friendly output projection.
"""
from __future__ import annotations

import json
import os
import re
from datetime import datetime
from typing import Any

from ._time import iso_utc_from_microseconds, to_utc_microseconds

__all__ = ["VirgoStarsEvents"]
__version__ = "0.1.0"


class VirgoStarsEvents:
    """A read-only indexed event archive, not a live astronomy calculator."""

    def __init__(self, native_reader: Any) -> None:
        self._reader = native_reader

    @property
    def closed(self) -> bool:
        """Whether this wrapper no longer holds its native archive snapshot."""
        return self._reader is None

    def close(self) -> None:
        """Release the native reader and its private P0 snapshot if unshared.

        Calling close() multiple times is safe. Any other owners of the
        native reader may delay the actual temporary-file cleanup.
        """
        self._reader = None

    def __enter__(self) -> "VirgoStarsEvents":
        if self.closed:
            raise RuntimeError("VirgoStarsEvents reader is closed")
        return self

    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None:
        self.close()

    @classmethod
    def open(
        cls, path: str | os.PathLike[str], *, expected_sha256: str
    ) -> "VirgoStarsEvents":
        """Load the local archive, rejecting a mismatched trusted SHA-256.

        Installing the Python package does not download or bundle a dataset.
        """
        if not isinstance(expected_sha256, str) or re.fullmatch(
            r"[0-9a-fA-F]{64}", expected_sha256
        ) is None:
            raise ValueError("expected_sha256 must be a 64-digit hexadecimal SHA-256")
        from ._native import NativeReader

        return cls(NativeReader(os.fspath(path), expected_sha256.lower()))

    def get_synods_at(self, instant: datetime | str) -> dict[str, Any]:
        """Return the latest recorded synod per body plus the previous new moon.

        Derived phase labels refer to stored thresholds only. The output
        includes unreviewed coverage status and never promises completeness.
        """
        reader = self._reader
        if reader is None:
            raise RuntimeError("VirgoStarsEvents reader is closed")
        requested_us = to_utc_microseconds(instant)
        result = json.loads(reader.get_synods_at_us(requested_us))
        # Do not silently replace a mismatched timestamp from the native
        # extension. A Rust/Python transport regression must fail closed,
        # rather than mislabeling synod results with the requested instant.
        native_us = result.get("requested_at_us")
        if type(native_us) is not int or native_us != requested_us:
            raise ValueError("native synod response instant does not match the request")
        result.pop("requested_at_us")
        result["requested_at_utc"] = iso_utc_from_microseconds(requested_us)

        for record in result["synods"].values():
            record["status"] = (
                "recorded" if record["cycle_start_us"] is not None
                else "no_preceding_recorded_cycle"
            )
            record["cycle_start_utc"] = iso_utc_from_microseconds(
                record.pop("cycle_start_us")
            )
            record["next_cycle_start_utc"] = iso_utc_from_microseconds(
                record.pop("next_cycle_start_us")
            )
            extras = record.pop("phase_thresholds_us")
            record["phase_thresholds_utc"] = (
                None if extras is None
                else [iso_utc_from_microseconds(value) for value in extras]
            )
            record["phase"]["started_at_utc"] = iso_utc_from_microseconds(
                record["phase"].pop("started_at_us")
            )

        # Exactly the same Moon object, not a second query that may disagree.
        moon = result["synods"].get("Moon")
        result["new_moon"] = (
            moon if moon is not None and moon["cycle_start_utc"] is not None else None
        )
        return result
