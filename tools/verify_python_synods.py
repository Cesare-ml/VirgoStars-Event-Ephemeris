#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Read-only *Python API vs source CSV* parity oracle for the M0 synod MVP.

This is DEVELOPMENT VALIDATION, not a Python runtime decoder or an astronomical
calculation. It reads original normalized source CSVs, independently selects
the latest row at/before several instants, and compares the installed Rust/PyO3
package's complete output: source IDs, timestamps, next anchors, exact phase
thresholds, and reported cycle-stage labels.

Does not bundle, export, download, mutate, or certify completeness of data.
The CSV data must be supplied separately from the owner's authorized snapshot.
"""
from __future__ import annotations

import argparse
from bisect import bisect_right
import csv
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import sys

from verify_csv_p0 import HEADERS, normalized_row

EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)
PLANETARY_STAGES = ("D1", "R2", "R3", "D4")
MOON_STAGES = ("CRESC1", "CRESC2", "CAL1", "CAL2")
PLANET_BODIES = frozenset(
    ("Sun", "Mercury", "Venus", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune", "Pluto")
)


@dataclass(frozen=True)
class SourceSynod:
    at: int
    source_id: int
    extras: tuple[int, ...] | None


def iso_utc(us: int) -> str:
    return (EPOCH + timedelta(microseconds=us)).isoformat(
        timespec="microseconds"
    ).replace("+00:00", "Z")


def instant_us(value: str) -> int:
    if "T" not in value or not value.endswith("Z"):
        raise ValueError("parity probe must have a UTC ISO-8601 Z suffix")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    delta = parsed - EPOCH
    return (delta.days * 86_400 + delta.seconds) * 1_000_000 + delta.microseconds


def load_synod_csv(path: Path) -> dict[str, list[SourceSynod]]:
    """Load independent headered, normalized source CSV; reject duplicates."""
    required = list(HEADERS["synod"][1])
    result: dict[str, list[SourceSynod]] = {}
    seen_source_ids: set[int] = set()
    with path.open("r", encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file)
        if reader.fieldnames != required:
            raise ValueError(f"normalized CSV columns differ: {reader.fieldnames!r}")
        for line, row in enumerate(reader, 2):
            if None in row:
                raise ValueError(f"unexpected CSV fields at line {line}")
            body, source_id, fields = normalized_row("synod", row)
            if source_id in seen_source_ids:
                raise ValueError(f"repeated source row ID {source_id} at line {line}")
            seen_source_ids.add(source_id)
            at, extras = fields
            result.setdefault(body, []).append(
                SourceSynod(at=at, source_id=source_id,
                            extras=None if extras is None else tuple(extras))
            )
    if not result:
        raise ValueError("source CSV contains no synods")
    for body, series in result.items():
        series.sort(key=lambda event: event.at)
        if any(a.at == b.at for a, b in zip(series, series[1:])):
            raise ValueError(f"duplicate synod timestamp for {body}")
    return dict(sorted(result.items()))


def expected_row(series: list[SourceSynod], instant: int) -> tuple[SourceSynod | None, SourceSynod | None]:
    timestamps = [event.at for event in series]
    position = bisect_right(timestamps, instant)
    if position == 0:
        return None, None
    selected = series[position - 1]
    following = series[position] if position < len(series) else None
    return selected, following


def expected_phase(
    body: str, event: SourceSynod | None, next_event: SourceSynod | None, instant: int,
) -> tuple[str | None, str | None, str]:
    if event is None:
        return None, None, "no_preceding_recorded_cycle"
    if body != "Moon" and body not in PLANET_BODIES:
        return None, None, "unknown_unsupported_phase_convention"
    if event.extras is None:
        return None, None, "unknown_missing_phase_thresholds"
    if len(event.extras) != 3:
        return None, None, "unknown_incomplete_phase_thresholds"
    a, b, c = event.extras
    if not event.at < a < b < c or (next_event is not None and c >= next_event.at):
        return None, None, "unknown_invalid_cycle_bounds"
    if instant < a:
        index, since = 0, event.at
    elif instant < b:
        index, since = 1, a
    elif instant < c:
        index, since = 2, b
    else:
        if next_event is None:
            return None, None, "unknown_open_ended_cycle"
        index, since = 3, c
    stages = MOON_STAGES if body == "Moon" else PLANETARY_STAGES
    return stages[index], iso_utc(since), "derived_from_recorded_thresholds"


def select_probes(source: dict[str, list[SourceSynod]], baseline: str) -> list[int]:
    """Deterministic probes for everyday usage and exact phase boundaries."""
    base = instant_us(baseline)
    probes = {base}
    for body in ("Moon", "Mercury"):
        series = source.get(body)
        if not series:
            continue
        current, _ = expected_row(series, base)
        if current is None:
            current = series[0]
        probes.add(current.at)
        probes.add(current.at - 1)
        probes.add(current.at + 1)
        if current.extras is not None:
            probes.update(current.extras[:3])
    moon = source.get("Moon")
    if moon:
        probes.add(moon[0].at - 1)
    return sorted(probes)


def assert_parity(
    reader: object, source: dict[str, list[SourceSynod]],
    probes: list[int], digest: str,
) -> dict:
    """Compare all indexed bodies at each probe; raise on the FIRST mismatch."""
    comparison_count = 0
    bodies = set(source)
    if not probes:
        raise ValueError("no parity probes were supplied")
    for instant in probes:
        result = reader.get_synods_at(iso_utc(instant))
        if result["requested_at_utc"] != iso_utc(instant):
            raise ValueError(f"requested instant mismatch at {instant}")
        dataset = result["dataset"]
        if dataset["sha256"] != digest or dataset["coverage_certified"] is not False:
            raise ValueError("dataset SHA-256 or coverage claim mismatch")
        actual = result["synods"]
        if set(actual) != bodies:
            raise ValueError(f"body set mismatch: missing={sorted(bodies-set(actual))}, "
                             f"extra={sorted(set(actual)-bodies)}")
        for body, series in source.items():
            previous, upcoming = expected_row(series, instant)
            row = actual[body]
            expected_id = previous.source_id if previous else None
            expected_start = iso_utc(previous.at) if previous else None
            expected_next = iso_utc(upcoming.at) if upcoming else None
            expected_extras = (
                None if previous is None or previous.extras is None
                else [iso_utc(value) for value in previous.extras]
            )
            code, started, status = expected_phase(body, previous, upcoming, instant)
            expected_fields = {
                "body": body,
                "source_id": expected_id,
                "cycle_start_utc": expected_start,
                "next_cycle_start_utc": expected_next,
                "phase_thresholds_utc": expected_extras,
                "status": "recorded" if previous else "no_preceding_recorded_cycle",
            }
            for key, expected in expected_fields.items():
                if row.get(key) != expected:
                    raise ValueError(
                        f"parity mismatch {body} at {iso_utc(instant)}: "
                        f"{key}={row.get(key)!r} expected={expected!r}"
                    )
            phase = row["phase"]
            for key, expected in (
                ("code", code), ("started_at_utc", started), ("status", status)
            ):
                if phase.get(key) != expected:
                    raise ValueError(
                        f"parity mismatch {body} at {iso_utc(instant)}: "
                        f"phase.{key}={phase.get(key)!r} expected={expected!r}"
                    )
            if row["coverage"]["status"] != "unreviewed":
                raise ValueError("unreviewed coverage was not surfaced")
            comparison_count += 1
        expected_moon = actual.get("Moon")
        if expected_moon is None or expected_moon["cycle_start_utc"] is None:
            expected_moon = None
        if result.get("new_moon") != expected_moon:
            raise ValueError(f"new_moon alias mismatch at {iso_utc(instant)}")
    return {
        "checked_bodies": len(bodies),
        "checked_instants": len(probes),
        "checked_body_instants": comparison_count,
        "source_rows": sum(len(events) for events in source.values()),
        "sha256": digest,
        "scientific_coverage_certified": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Check real Python/Rust synod API against original source CSV")
    parser.add_argument("--csv", type=Path, required=True, help="Authorized normalized sinodi.csv")
    parser.add_argument("--archive", type=Path, required=True, help="Authorized local VSEPH-P0 file")
    parser.add_argument("--sha256", required=True, help="Trusted lower-case whole-file SHA-256")
    parser.add_argument("--instant", default="2026-11-11T12:00:00Z",
                        help="Baseline UTC instant with Z suffix")
    args = parser.parse_args()
    try:
        source = load_synod_csv(args.csv)
        probes = select_probes(source, args.instant)
        from virgostars_events import VirgoStarsEvents
        reader = VirgoStarsEvents.open(args.archive, expected_sha256=args.sha256)
        report = assert_parity(reader, source, probes, args.sha256)
    except (OSError, ValueError, TypeError, KeyError, OverflowError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
