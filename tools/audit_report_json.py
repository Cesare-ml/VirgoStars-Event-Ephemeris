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

# A P0 audit is a small summary, never a dump of individual events.
# Limit untrusted subprocess output before parsing it, not afterwards.
MAX_AUDIT_REPORT_BYTES = 64 * 1024
MAX_AUDIT_REPORT_NESTING = 32


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


def _require_unicode_scalars(value: Any) -> None:
    """Reject unpaired UTF-16 surrogates accepted by Python's JSON decoder.

    Traverse without recursion and apply an explicit maximum nesting depth.
    Paired escapes decode to a single legal Unicode scalar and remain valid.
    """
    pending = [(value, 0)]
    while pending:
        item, depth = pending.pop()
        if depth > MAX_AUDIT_REPORT_NESTING:
            raise ValueError("audit report nesting exceeds 32 levels")
        if isinstance(item, str):
            item.encode("utf-8", errors="strict")
        elif isinstance(item, dict):
            pending.extend((key, depth + 1) for key in item)
            pending.extend((entry, depth + 1) for entry in item.values())
        elif isinstance(item, list):
            pending.extend((entry, depth + 1) for entry in item)


def decode_audit_report(payload: str, *, source: str) -> dict[str, Any]:
    """Fail closed on oversized, ambiguous and non-UTF-8 JSON reports."""
    if type(payload) is not str:
        raise RuntimeError(f"{source} P0 audit must emit JSON text")
    try:
        if len(payload.encode("utf-8", errors="strict")) > MAX_AUDIT_REPORT_BYTES:
            raise ValueError("audit report exceeds 65536 UTF-8 bytes")
        result = json.loads(
            payload,
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
            parse_float=_finite_float,
        )
        _require_unicode_scalars(result)
    except (ValueError, TypeError, RecursionError, UnicodeError) as exc:
        raise RuntimeError(
            f"{source} P0 audit emitted invalid or ambiguous JSON: {exc}"
        ) from exc
    if type(result) is not dict:
        raise RuntimeError(f"{source} P0 audit must report a JSON object")
    return result
