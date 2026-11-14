# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Strict JSON transport boundary for independent P0 audit subprocesses.

Audit reports must be unambiguous standards-compliant JSON before their
counts, digest and coverage declarations are compared across implementations.
"""
from __future__ import annotations

import json
import math
from typing import Any


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON object key: {key!r}")
        result[key] = value
    return result


def _reject_constant(value: str) -> None:
    raise ValueError(f"nonstandard JSON number: {value}")


def _finite_float(token: str) -> float:
    value = float(token)
    if not math.isfinite(value):
        raise ValueError(f"nonfinite JSON number: {token}")
    return value


def decode_audit_report(payload: str, *, source: str) -> dict[str, Any]:
    """Fail closed on duplicate keys and nonfinite numbers, at any depth."""
    if type(payload) is not str:
        raise RuntimeError(f"{source} P0 audit must emit JSON text")
    try:
        result = json.loads(
            payload,
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
            parse_float=_finite_float,
        )
    except (ValueError, TypeError) as exc:
        raise RuntimeError(
            f"{source} P0 audit emitted invalid or ambiguous JSON: {exc}"
        ) from exc
    if type(result) is not dict:
        raise RuntimeError(f"{source} P0 audit must report a JSON object")
    return result
