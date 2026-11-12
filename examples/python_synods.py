# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Local offline Python MVP example (requires a built Rust wheel).

Example after installing the wheel and assembling your authorized P0 file:
  python examples/python_synods.py path/to/events.vseph-p0 \
      2026-11-11T12:00:00Z --sha256 <trusted manifest SHA-256>
"""
from __future__ import annotations

import argparse
import json

from virgostars_events import VirgoStarsEvents


def main() -> None:
    parser = argparse.ArgumentParser(description="Query recorded VirgoStars synods offline")
    parser.add_argument("archive", help="Local authorized experimental VSEPH-P0 file")
    parser.add_argument("instant", help="ISO-8601 date-time with Z or UTC offset")
    parser.add_argument("--sha256", required=True, help="Trusted whole-file SHA-256")
    args = parser.parse_args()

    # Close immediately: this releases the Rust-owned temporary P0 snapshot.
    with VirgoStarsEvents.open(args.archive, expected_sha256=args.sha256) as reader:
        result = reader.get_synods_at(args.instant)
        print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
