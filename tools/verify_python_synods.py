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
from operator import attrgetter
import re
from pathlib import Path
import sys

from verify_csv_p0 import HEADERS, normalized_row

EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)
PLANETARY_STAGES = ("D1", "R2", "R3", "D4")
MOON_STAGES = ("CRESC1", "CRESC2", "CAL1", "CAL2")
# Conservative source-data discontinuity flag, not a predictive astronomy rule.
# Across the current 37,054 recorded lunar anchors, only two gaps exceed this
# bound (383 and 413 days); all other consecutive spans are below 30 days.
MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US = 45 * 86_400 * 1_000_000
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


# Only canonical UTC instants with at most six fractional digits may enter
# the parity oracle. datetime.fromisoformat() otherwise silently truncates
# sub-microsecond digits on supported Python versions.
_UTC_PROBE_RE = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}T"
    r"[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]{1,6})?Z"
)


def instant_us(value: str) -> int:
    """Parse an exact UTC microsecond probe without silent truncation."""
    if not isinstance(value, str) or _UTC_PROBE_RE.fullmatch(value) is None:
        raise ValueError(
            "parity probe must be strict YYYY-MM-DDTHH:MM:SS[.ffffff]Z "
            "with at most six fractional digits"
        )
    try:
        parsed = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise ValueError("invalid UTC parity probe") from exc
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


# Python >=3.10 supports bisect's key argument. Search directly in the
# already-sorted SourceSynod series instead of allocating a second O(N) list
# of timestamps for every body at every parity probe.
_EVENT_AT = attrgetter("at")


def expected_row(series: list[SourceSynod], instant: int) -> tuple[SourceSynod | None, SourceSynod | None]:
    position = bisect_right(series, instant, key=_EVENT_AT)
    if position == 0:
        return None, None
    selected = series[position - 1]
    following = series[position] if position < len(series) else None
    return selected, following


def has_moon_source_gap(
    body: str, event: SourceSynod | None, next_event: SourceSynod | None,
) -> bool:
    return (
        body == "Moon"
        and event is not None
        and next_event is not None
        and next_event.at - event.at > MAX_CONTIGUOUS_MOON_ANCHOR_SPAN_US
    )


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
        if has_moon_source_gap(body, event, next_event):
            return None, None, "unknown_discontinuous_moon_anchors"
        index, since = 3, c
    stages = MOON_STAGES if body == "Moon" else PLANETARY_STAGES
    return stages[index], iso_utc(since), "derived_from_recorded_thresholds"


def select_probes(source: dict[str, list[SourceSynod]], baseline: str) -> list[int]:
    """Bounded cross-body probes, including exact and adjacent phase instants.

    The underlying recorded-event index is read in full, but the compiled
    API is queried at a deterministic sample rather than at every source row.
    All indexed bodies are represented, not only Moon and Mercury. Include
    candidate historical Moon gaps as *queries*, not inferred coverage.
    """
    base = instant_us(baseline)
    probes = {base}
    for series in source.values():
        if not series:
            raise ValueError("empty indexed body series")
        current, _ = expected_row(series, base)
        if current is None:
            current = series[0]

        # Sample every body's oldest, middle, latest and baseline-nearest
        # anchors. A single recent baseline alone cannot catch historical
        # source-order or phase-boundary regressions across millennia.
        # Four anchors/body keeps the native parity workload bounded even
        # when the original CSV contains tens of thousands of cycles.
        selected = {event.at: event for event in (
            series[0], series[len(series) // 2], series[-1], current
        )}
        for event in selected.values():
            # Equality, one microsecond before, and one after distinguish
            # inclusive anchor lookup from next/previous event semantics.
            probes.update((event.at - 1, event.at, event.at + 1))

            # Source phase boundaries are independently sampled with the
            # same exact/adjacent rule for every selected historical era.
            for at in (event.extras or ())[:3]:
                probes.update((at - 1, at, at + 1))

    moon = source.get("Moon")
    if moon:
        probes.add(moon[0].at - 1)
        # Inspect source gaps directly, not just fixed civil dates. Query
        # mid-gap and the instant before the next recorded new moon: this
        # catches an incorrectly extended CAL2 phase for a whole missing year.
        suspicious = 0
        for preceding, following in zip(moon, moon[1:]):
            if has_moon_source_gap("Moon", preceding, following):
                probes.update((
                    (preceding.at + following.at) // 2,
                    following.at - 1,
                ))
                suspicious += 1
                if suspicious >= 8:
                    break

    # Explicit historical regression dates from the authorized 2026 snapshot.
    # Jan 2018 still has stored early-Moon thresholds, while June 2018 must
    # NOT be mislabelled as CAL2 after the final recorded 2017 Moon phase.
    for instant in (
        "1919-01-01T00:00:00Z", "1919-06-01T00:00:00Z",
        "2018-01-01T00:00:00Z", "2018-06-01T00:00:00Z",
    ):
        probes.add(instant_us(instant))

    # Normalized source timestamps can legally fall on the first or last
    # representable Python datetime. The adjacent +/-1 us parity queries
    # must NOT cross year 0001/9999: iso_utc() and the Python-facing API
    # cannot represent those synthetic out-of-range instants.
    earliest = instant_us("0001-01-01T00:00:00Z")
    latest = instant_us("9999-12-31T23:59:59.999999Z")
    return sorted(instant for instant in probes if earliest <= instant <= latest)


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
        # The experiment is identified by both its trusted whole-file digest
        # and the P0 schema. Missing/changed identity must not pass source
        # parity just because the event rows happen to look plausible.
        if (
            dataset.get("sha256") != digest
            or dataset.get("format") != "VSEPH-P0"
            or "id" not in dataset
            or dataset["id"] is not None
            or dataset.get("coverage_certified") is not False
        ):
            raise ValueError("dataset identity, format, SHA-256 or coverage claim mismatch")
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
            warnings = row["coverage"].get("warnings")
            if not isinstance(warnings, list) or "no_reviewed_coverage_policy" not in warnings:
                raise ValueError(f"missing unreviewed coverage warning for {body}")
            suspect = has_moon_source_gap(body, previous, upcoming)
            flagged = "moon_recorded_cycle_gap_over_45_days" in warnings
            if flagged != suspect:
                raise ValueError(
                    f"incorrect Moon source-discontinuity warning for {body} "
                    f"at {iso_utc(instant)}"
                )
            comparison_count += 1
        expected_moon = actual.get("Moon")
        if expected_moon is None or expected_moon["cycle_start_utc"] is None:
            expected_moon = None
        # This public Python convenience field must be the *same object* as
        # synods["Moon"], not a separately queried/detached copy that could
        # diverge after callers inspect or update their dictionaries.
        # The public JSON contract requires an explicit new_moon key even
        # before the first recorded Moon, where its value must be None.
        # dict.get() alone cannot distinguish that null from an absent key.
        if "new_moon" not in result or result["new_moon"] is not expected_moon:
            raise ValueError(f"new_moon alias mismatch at {iso_utc(instant)}")
    return {
        "checked_bodies": len(bodies),
        "checked_instants": len(probes),
        "checked_body_instants": comparison_count,
        "source_rows": sum(len(events) for events in source.values()),
        "sha256": digest,
        "scientific_coverage_certified": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check real Python/Rust synod API against original source CSV")
    parser.add_argument("--csv", type=Path, required=True, help="Authorized normalized sinodi.csv")
    parser.add_argument("--archive", type=Path, required=True, help="Authorized local VSEPH-P0 file")
    parser.add_argument("--sha256", required=True, help="Trusted lower-case whole-file SHA-256")
    parser.add_argument("--instant", default="2026-11-11T12:00:00Z",
                        help="Baseline UTC instant with Z suffix")
    args = parser.parse_args(argv)
    # Validate trust material and the UTC instant BEFORE reading a potentially
    # large CSV or creating a verified private P0 archive snapshot.
    if re.fullmatch(r"[0-9a-fA-F]{64}", args.sha256) is None:
        parser.error("--sha256 must be a 64-character hexadecimal SHA-256")
    trusted_sha256 = args.sha256.lower()
    try:
        instant_us(args.instant)
        source = load_synod_csv(args.csv)
        probes = select_probes(source, args.instant)
        from virgostars_events import VirgoStarsEvents
        # The native reader keeps a verified temporary archive copy. Release
        # it promptly even if source parity detects an error.
        with VirgoStarsEvents.open(args.archive, expected_sha256=trusted_sha256) as reader:
            report = assert_parity(reader, source, probes, trusted_sha256)
    except (ImportError, OSError, RuntimeError, ValueError, TypeError, KeyError, OverflowError) as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
