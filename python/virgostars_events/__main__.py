# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Offline CLI for the Rust-backed VirgoStars synod reader.

    python -m virgostars_events local.p0 2026-11-11T12:00:00Z --sha256 <trusted>
    virgostars-events local.p0 2026-11-11T12:00:00Z --sha256 <trusted>

No scientific data is included, downloaded or computed by this command.
"""
from __future__ import annotations

import argparse
import json
import sys
from typing import Sequence

from . import VirgoStarsEvents, __version__
from ._time import to_utc_microseconds


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="virgostars-events",
        description="Read recorded synods and previous new moon from an offline P0 archive",
    )
    parser.add_argument("archive", help="Existing authorized local VSEPH-P0 file")
    parser.add_argument("instant", help="ISO-8601 UTC instant (Z or explicit offset)")
    parser.add_argument(
        "--sha256",
        required=True,
        help="Independently trusted whole-file SHA-256 (required)",
    )
    parser.add_argument(
        "--compact", action="store_true", help="Output compact one-line JSON"
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    args = parser.parse_args(argv)

    try:
        # Fail before hashing/opening a potentially large archive on invalid input.
        to_utc_microseconds(args.instant)
        # NativeReader owns a verified private P0 copy. Close on successful
        # serialization *and* on every query/JSON failure, before printing.
        with VirgoStarsEvents.open(args.archive, expected_sha256=args.sha256) as reader:
            result = reader.get_synods_at(args.instant)
            output = json.dumps(
                result,
                ensure_ascii=False,
                indent=None if args.compact else 2,
            )
    except (ImportError, OSError, ValueError, TypeError, OverflowError, RuntimeError) as exc:
        print(f"virgostars-events: {exc}", file=sys.stderr)
        return 1

    print(output)
    return 0


def run() -> None:
    raise SystemExit(main())


if __name__ == "__main__":
    run()
