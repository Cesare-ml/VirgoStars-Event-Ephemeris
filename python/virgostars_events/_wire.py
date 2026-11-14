# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Strict JSON transport checks for the Rust-backed synod response.

Reject ambiguous/non-standard JSON before projecting original event records.
"""
from __future__ import annotations

import json
import math
from typing import Any


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"native synod response has duplicate JSON key: {key!r}")
        result[key] = value
    return result


def _reject_nonfinite(token: str) -> None:
    # json.loads otherwise accepts NaN and +/-Infinity despite RFC 8259.
    raise ValueError(f"native synod response has non-finite JSON value: {token}")


def _finite_json_float(token: str) -> float:
    # A valid JSON numeric literal such as 1e400 also overflows to inf under
    # Python's float parser; parse_constant never sees these tokens.
    value = float(token)
    if not math.isfinite(value):
        raise ValueError(f"native synod response has non-finite JSON value: {token}")
    return value


def _require_unicode_scalars(value: Any) -> None:
    """Reject lone UTF-16 surrogates in JSON keys and values, at any depth.

    Python's json decoder accepts ``\\ud800`` on its own even though it
    cannot be represented as a Unicode scalar in UTF-8. The Rust side emits
    valid UTF-8, so accepting such strings would mask malformed test/native
    transport data. Traverse iteratively to avoid a second recursion limit.
    """
    pending = [value]
    while pending:
        item = pending.pop()
        if isinstance(item, str):
            try:
                item.encode("utf-8", errors="strict")
            except UnicodeEncodeError as exc:
                raise ValueError(
                    "native synod response contains an unpaired Unicode surrogate"
                ) from exc
        elif isinstance(item, dict):
            pending.extend(item.keys())
            pending.extend(item.values())
        elif isinstance(item, list):
            pending.extend(item)


def decode_native_json(payload: str) -> Any:
    """Parse only standard, unambiguous JSON (including nested objects)."""
    if type(payload) is not str:
        raise ValueError("native synod response must be JSON text")
    result = json.loads(
        payload,
        object_pairs_hook=_unique_object,
        parse_constant=_reject_nonfinite,
        parse_float=_finite_json_float,
    )
    _require_unicode_scalars(result)
    return result


def same_json_value(left: Any, right: Any) -> bool:
    """Compare native JSON values exactly, without recursive Python calls.

    Python's equality treats True == 1 and 1 == 1.0 as equal, although Rust
    emits different JSON scalar types. A stack avoids RecursionError on
    valid deeply nested native responses before the Moon alias is checked.
    """
    pending = [(left, right)]
    while pending:
        current, expected = pending.pop()
        if type(current) is not type(expected):
            return False
        if isinstance(current, dict):
            if current.keys() != expected.keys():
                return False
            pending.extend((value, expected[key]) for key, value in current.items())
        elif isinstance(current, list):
            if len(current) != len(expected):
                return False
            pending.extend(zip(current, expected))
        elif current != expected:
            return False
    return True
