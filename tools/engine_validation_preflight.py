# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Read-only structural preflight for a proposed engine-validation plan.

This validates declarations, not their truth, approval, provenance or scientific
independence. It never reads declared artifacts or runs a provider.
"""
from __future__ import annotations

import argparse
from decimal import Decimal, InvalidOperation
import hashlib
import json
import math
import re
import sys
from pathlib import Path

MAX_BYTES = 65536
MAX_DEPTH = 16
QUANTITIES = frozenset({
    "longitude_degrees", "latitude_degrees",
    "longitude_speed_degrees_per_day", "latitude_speed_degrees_per_day",
})
UNITS = {
    "longitude_degrees": "degrees",
    "latitude_degrees": "degrees",
    "longitude_speed_degrees_per_day": "degrees/day",
    "latitude_speed_degrees_per_day": "degrees/day",
}
I64_MIN, I64_MAX = -(2**63), 2**63 - 1


def _fail() -> None:
    raise ValueError("invalid or incomplete plan")


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            _fail()
        result[key] = value
    return result


def _bad_constant(_value):
    _fail()


def _finite_float(token):
    value = float(token)
    try:
        underflow = value == 0.0 and Decimal(token) != 0
    except InvalidOperation:
        _fail()
    if not math.isfinite(value) or underflow:
        _fail()
    return value


def _shape(value, keys):
    if type(value) is not dict or set(value) != set(keys):
        _fail()


def _text(value):
    if type(value) is not str or not value.strip():
        _fail()
    if value.strip().casefold() in {"todo", "tbd", "unset", "unknown", "n/a"}:
        _fail()


def _digest(value, lengths):
    if type(value) is not str or len(value) not in lengths or not re.fullmatch(r"[0-9a-f]+", value):
        _fail()


def _unicode_and_depth(root):
    pending = [(root, 0)]
    while pending:
        value, depth = pending.pop()
        if depth > MAX_DEPTH:
            _fail()
        if type(value) is str:
            value.encode("utf-8", errors="strict")
        elif type(value) is dict:
            pending.extend((v, depth + 1) for v in value.values())
            pending.extend((k, depth + 1) for k in value)
        elif type(value) is list:
            pending.extend((v, depth + 1) for v in value)


def check_plan_bytes(payload: bytes) -> dict:
    """Validate one bounded UTF-8 plan; never open its declared artifacts."""
    if type(payload) is not bytes or len(payload) > MAX_BYTES:
        _fail()
    try:
        plan = json.loads(
            payload.decode("utf-8", errors="strict"),
            object_pairs_hook=_object,
            parse_constant=_bad_constant,
            parse_float=_finite_float,
        )
        _unicode_and_depth(plan)
    except (ValueError, TypeError, RecursionError, UnicodeError):
        _fail()
    _shape(plan, ("schema", "scope", "engine", "reference", "cases", "tolerances", "review"))
    if plan["schema"] != "engine-validation-plan-v1":
        _fail()

    scope = plan["scope"]
    _shape(scope, ("bodies", "start_us", "end_us", "quantities", "center", "frame",
                   "corrections", "time_convention"))
    bodies, quantities = scope["bodies"], scope["quantities"]
    if type(bodies) is not list or not bodies or len(bodies) > 100:
        _fail()
    for body in bodies:
        _text(body)
        if body != body.strip():
            _fail()
    if len({body.casefold() for body in bodies}) != len(bodies):
        _fail()
    if type(quantities) is not list or not quantities or any(type(q) is not str for q in quantities):
        _fail()
    if len(set(quantities)) != len(quantities) or not set(quantities) <= QUANTITIES:
        _fail()
    start, end = scope["start_us"], scope["end_us"]
    if type(start) is not int or type(end) is not int or not I64_MIN <= start < end <= I64_MAX:
        _fail()
    for field in ("center", "frame", "corrections", "time_convention"):
        _text(scope[field])

    engine = plan["engine"]
    _shape(engine, ("source_revision", "binary_sha256", "binding_sha256", "kernel_sha256",
                    "build_attestation", "kernel_provenance", "time_data_identity"))
    _digest(engine["source_revision"], (40, 64))
    for field in ("binary_sha256", "binding_sha256", "kernel_sha256"):
        _digest(engine[field], (64,))
    for field in ("build_attestation", "kernel_provenance", "time_data_identity"):
        _text(engine[field])

    reference = plan["reference"]
    _shape(reference, ("implementation", "version", "provenance", "shared_dependencies",
                       "independence_rationale"))
    for value in reference.values():
        _text(value)
    cases = plan["cases"]
    _shape(cases, ("selection", "holdout", "expected_count"))
    _text(cases["selection"])
    _text(cases["holdout"])
    if type(cases["expected_count"]) is not int or not 1 <= cases["expected_count"] <= 1_000_000:
        _fail()

    tolerances = plan["tolerances"]
    if type(tolerances) is not list or len(tolerances) != len(bodies) * len(quantities):
        _fail()
    by_pair = set()
    for tolerance in tolerances:
        _shape(tolerance, ("body", "quantity", "max_absolute_error", "unit", "rationale"))
        body, quantity = tolerance["body"], tolerance["quantity"]
        if (type(body) is not str or body not in bodies or type(quantity) is not str
                or quantity not in quantities or (body, quantity) in by_pair):
            _fail()
        by_pair.add((body, quantity))
        number = tolerance["max_absolute_error"]
        try:
            finite = type(number) in (int, float) and math.isfinite(number)
        except OverflowError:
            finite = False
        if not finite or number < 0 or tolerance["unit"] != UNITS[quantity]:
            _fail()
        _text(tolerance["rationale"])
    review = plan["review"]
    _shape(review, ("reviewer", "relationship", "preapproval_record"))
    for value in review.values():
        _text(value)
    return {
        "schema": "engine-validation-preflight-v1",
        "plan_sha256": hashlib.sha256(payload).hexdigest(),
        "declarations_well_formed": True,
        "engine_independently_validated": False,
        "astronomical_accuracy_verified": False,
        "coverage_certified": False,
        "safe_to_import": False,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", type=Path, help="UTF-8 JSON plan; declared artifacts are not opened")
    args = parser.parse_args(argv)
    try:
        with args.plan.open("rb") as handle:
            payload = handle.read(MAX_BYTES + 1)
        result = check_plan_bytes(payload)
    except (OSError, ValueError, OverflowError):
        print("engine plan preflight: invalid, incomplete or unreadable plan", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
