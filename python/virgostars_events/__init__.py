# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Offline VirgoStars Event Ephemeris (experimental VSEPH-P0 Python binding).

The binary reader and synod stage classifier live exclusively in Rust.
This layer handles input UTC datetimes and JSON-friendly output projection.
"""
from __future__ import annotations

import os
import re
from datetime import datetime
from typing import Any

from ._time import iso_utc_from_microseconds, to_utc_microseconds
from ._phase_wire import validate_recorded_thresholds
from ._wire import decode_native_json

__all__ = ["VirgoStarsEvents"]
__version__ = "0.1.0"


class VirgoStarsEvents:
    """A read-only indexed event archive, not a live astronomy calculator."""

    def __init__(self, native_reader: Any, *, expected_sha256: str | None = None) -> None:
        self._reader = native_reader
        # Only open() supplies a separately trusted digest. Direct test
        # readers can still use the wrapper without a local archive.
        self._trusted_sha256 = expected_sha256

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

        canonical_sha256 = expected_sha256.lower()
        return cls(
            NativeReader(os.fspath(path), canonical_sha256),
            expected_sha256=canonical_sha256,
        )

    def get_synods_at(self, instant: datetime | str) -> dict[str, Any]:
        """Return the latest recorded synod per body plus the previous new moon.

        Derived phase labels refer to stored thresholds only. The output
        includes unreviewed coverage status and never promises completeness.
        """
        reader = self._reader
        if reader is None:
            raise RuntimeError("VirgoStarsEvents reader is closed")
        requested_us = to_utc_microseconds(instant)
        result = decode_native_json(reader.get_synods_at_us(requested_us))
        if not isinstance(result, dict):
            raise ValueError("native synod response must be a JSON object")
        # Do not silently replace a mismatched timestamp from the native
        # extension. A Rust/Python transport regression must fail closed,
        # rather than mislabeling synod results with the requested instant.
        native_us = result.get("requested_at_us")
        if type(native_us) is not int or native_us != requested_us:
            raise ValueError("native synod response instant does not match the request")
        # A native transport regression must not silently change the archive
        # identity supplied by the owner. Check the full experimental P0
        # envelope before exposing even otherwise plausible event rows.
        dataset = result.get("dataset")
        if (
            not isinstance(dataset, dict)
            or dataset.get("format") != "VSEPH-P0"
            or "id" not in dataset
            or dataset["id"] is not None
            or dataset.get("coverage_certified") is not False
            or not isinstance(dataset.get("sha256"), str)
            or re.fullmatch(r"[0-9a-f]{64}", dataset["sha256"]) is None
            or (
                self._trusted_sha256 is not None
                and dataset["sha256"] != self._trusted_sha256
            )
        ):
            raise ValueError("native synod response dataset identity is invalid")
        # Validate the Rust convenience field BEFORE reprojecting records.
        # Silently overwriting a divergent native new_moon would mask a
        # cross-language binding regression even if every Moon row is valid.
        synods = result.get("synods")
        if not isinstance(synods, dict) or any(
            not isinstance(record, dict) for record in synods.values()
        ):
            raise ValueError("native synod response must contain a synods object of records")
        moon_raw = synods.get("Moon")
        expected_native_moon = (
            moon_raw if moon_raw is not None and moon_raw.get("cycle_start_us") is not None
            else None
        )
        if "new_moon" not in result or result["new_moon"] != expected_native_moon:
            raise ValueError("native synod response new_moon disagrees with Moon record")

        result.pop("requested_at_us")
        result["requested_at_utc"] = iso_utc_from_microseconds(requested_us)

        for body, record in synods.items():
            # Rust serializes the body twice (map key and record field), plus
            # an original positive source ID only if a recorded cycle exists.
            # Reject cross-body swaps or fabricated/missing source identities.
            if not isinstance(body, str) or record.get("body") != body:
                raise ValueError(f"native synod body identity mismatch for {body}")
            source_id = record.get("source_id")
            current_us = record["cycle_start_us"]
            if (
                (source_id is None) != (current_us is None)
                or (
                    source_id is not None
                    and (type(source_id) is not int or not 0 < source_id <= (1 << 64) - 1)
                )
            ):
                raise ValueError(f"native synod source ID inconsistent with cycle for {body}")
            # Validate native anchor types and bounds BEFORE comparing a
            # phase start against them: a malformed non-integer anchor must
            # fail as a ValueError, never as an incidental Python TypeError.
            # The Rust index promises a latest anchor <= X and a strictly
            # later next anchor. Never display a future start as recorded
            # current state, or a stale end boundary, if transport regresses.
            current_us = record["cycle_start_us"]
            next_us = record["next_cycle_start_us"]
            if current_us is not None and (
                type(current_us) is not int or current_us > requested_us
            ):
                raise ValueError(f"native synod current anchor invalid for {body}")
            if next_us is not None and (
                type(next_us) is not int
                or next_us <= requested_us
            ):
                raise ValueError(f"native synod next anchor invalid for {body}")

            # The P0 archive has no reviewed coverage policy. Reject any
            # transport payload that omits the mandatory warning or falsely
            # claims a scientifically certified body-specific event history.
            coverage = record.get("coverage")
            if (
                not isinstance(coverage, dict)
                or coverage.get("status") != "unreviewed"
                or not isinstance(coverage.get("warnings"), list)
                or "no_reviewed_coverage_policy" not in coverage["warnings"]
            ):
                raise ValueError(f"native synod coverage claim invalid for {body}")
            phase = record.get("phase")
            if not isinstance(phase, dict):
                raise ValueError(f"native synod phase must be an object for {body}")
            # A stage can only begin within its recorded cycle and no later
            # than X. Unknown stages have neither a code nor a start instant.
            # Reject malformed native transport rather than presenting a
            # future/pre-cycle stage as the current astronomical state.
            phase_code = phase.get("code")
            phase_started_us = phase.get("started_at_us")
            if (
                (phase_code is None) != (phase_started_us is None)
                or (
                    phase_started_us is not None
                    and (
                        type(phase_started_us) is not int
                        or current_us is None
                        or phase_started_us < current_us
                        or phase_started_us > requested_us
                    )
                )
            ):
                raise ValueError(f"native synod phase start invalid for {body}")

            # Verify the recorded model without inferring a scientifically
            # complete cycle: preserve NULL vs [] and future source phases.
            extras = validate_recorded_thresholds(record, current_us, body)

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
            record.pop("phase_thresholds_us")
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
