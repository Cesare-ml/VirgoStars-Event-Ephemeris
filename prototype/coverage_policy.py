# The contents of this file are subject to the Common Public Attribution
# License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed coverage-aware queries for experimental VSEPH-P0.

The index proves that an event is *recorded*. It never proves that events
between two instants have all been computed. A trusted state therefore
requires an explicitly reviewed coverage interval bound to this file's SHA256.
This adapter is provisional; the Rust reader will own these semantics in v1.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path

SCHEMA = "virgostars-reviewed-coverage/v0"
I64_MIN = -(1 << 63)
I64_MAX = (1 << 63) - 1
FAMILIES = frozenset(("ingress", "motion"))


def _time_value(value: object) -> int:
    if not isinstance(value, str) or not value or (value[0] == "-" and len(value) == 1):
        raise ValueError("coverage times must be signed decimal microsecond strings")
    digits = value[1:] if value[0] == "-" else value
    if not digits.isdecimal() or (len(digits) > 1 and digits.startswith("0")):
        raise ValueError("invalid coverage microseconds")
    number = int(value)
    if not I64_MIN <= number <= I64_MAX:
        raise ValueError("coverage timestamp exceeds signed i64")
    return number


@dataclass(frozen=True)
class ReviewedWindow:
    family: str
    body: str
    start_us: int
    end_us_exclusive: int
    reviewer: str
    evidence: str

    def contains(self, t: int) -> bool:
        return self.start_us <= t < self.end_us_exclusive


class CoveragePolicy:
    """Reviewed coverage only; never infer validity from observed min/max."""

    def __init__(self, windows: tuple[ReviewedWindow, ...]):
        self.windows = windows

    @classmethod
    def load(cls, policy_path: str | Path, archive_path: str | Path) -> "CoveragePolicy":
        doc = json.loads(Path(policy_path).read_text(encoding="utf-8"))
        if not isinstance(doc, dict) or doc.get("schema") != SCHEMA:
            raise ValueError("unsupported coverage policy schema")
        if doc.get("verification") != "human-reviewed-only":
            raise ValueError("coverage policy cannot infer verified ranges")
        sha = doc.get("archive_sha256")
        if not isinstance(sha, str) or len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha):
            raise ValueError("invalid archive SHA-256")
        digest = hashlib.sha256()
        with Path(archive_path).open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        if digest.hexdigest() != sha:
            raise ValueError("coverage policy does not match the selected archive SHA-256")
        raw = doc.get("reviewed_windows")
        if not isinstance(raw, list):
            raise ValueError("missing reviewed_windows array")
        windows = []
        for entry in raw:
            if not isinstance(entry, dict):
                raise ValueError("invalid coverage window")
            family, body = entry.get("family"), entry.get("body")
            reviewer, evidence = entry.get("reviewer"), entry.get("evidence")
            if family not in FAMILIES or not isinstance(body, str) or not body or body.strip() != body:
                raise ValueError("invalid family or body in coverage window")
            if not isinstance(reviewer, str) or not reviewer.strip() or not isinstance(evidence, str) or not evidence.strip():
                raise ValueError("reviewed intervals need reviewer and evidence")
            start = _time_value(entry.get("start_us"))
            end = _time_value(entry.get("end_us_exclusive"))
            if start >= end:
                raise ValueError("empty or inverted coverage interval")
            windows.append(ReviewedWindow(family, body, start, end, reviewer, evidence))
        windows.sort(key=lambda w: (w.family, w.body, w.start_us))
        for previous, current in zip(windows, windows[1:]):
            if (previous.family, previous.body) == (current.family, current.body) and previous.end_us_exclusive > current.start_us:
                raise ValueError("overlapping reviewed intervals")
        return cls(tuple(windows))

    def find(self, family: str, body: str, instant_us: int) -> ReviewedWindow | None:
        return next((w for w in self.windows if w.family == family and w.body == body and w.contains(instant_us)), None)


class CoverageAwareReader:
    """Opt-in safe state queries. The legacy raw reader API stays unchanged."""

    def __init__(self, reader, policy: CoveragePolicy):
        self.reader = reader
        self.policy = policy

    def _state(self, family: str, body: str, instant: str | int) -> dict:
        t = self.reader._time(instant)
        if family not in FAMILIES:
            raise ValueError("state family must be ingress or motion")
        recorded = self.reader.at(family, body, t)
        # The raw event timestamp is useful for diagnostics, but never a certified state.
        last_recorded_at_us = None if recorded is None else recorded[1]
        window = self.policy.find(family, body, t)
        if window is None:
            return {"status": "unknown_coverage", "state": None,
                    "last_recorded_at_us": last_recorded_at_us,
                    "reason": "no_human_reviewed_coverage_window"}
        if recorded is None or recorded[1] < window.start_us:
            return {"status": "unknown_initial_state", "state": None,
                    "last_recorded_at_us": last_recorded_at_us,
                    "reason": "no_recorded_event_within_reviewed_window_before_instant"}
        value = {"source_id": recorded[0], "at_us": recorded[1], "sign": recorded[2]}
        if family == "motion":
            value["direction"] = "retrograde" if recorded[3] == "r" else "direct"
        return {"status": "verified", "state": value,
                "reviewer": window.reviewer, "evidence": window.evidence,
                "reviewed_window_start_us": window.start_us,
                "reviewed_window_end_us_exclusive": window.end_us_exclusive}

    def sign_at(self, body: str, instant: str | int) -> dict:
        return self._state("ingress", body, instant)

    def direction_at(self, body: str, instant: str | int) -> dict:
        return self._state("motion", body, instant)
