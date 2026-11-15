# The contents of this file are subject to the Common Public Attribution
# License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Rebuild and verify owner-supplied scientific dataset files from Git blob parts.

Only the published immutable snapshot is reconstructed. No astronomy computation,
source database, internet connection, external Python package, or write to repo
tracked files is needed.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import sys
from pathlib import Path

PART = re.compile(r"part-(\d{3,})\Z")
ALLOWED = {
    "sinodi.csv": ("id", "elemento", "dataora_utc", "dataora_extra_utc_json"),
    "moto_pianeti.csv": ("id", "moto", "segno", "dataora_utc", "elemento_celeste"),
    "ingresso_segni.csv": ("id", "segno", "dataora_utc", "elemento_celeste"),
    "yearly_counts.csv": ("family", "body", "year", "observed_count"),
    "candidate_large_intervals.csv": (
        "family", "body", "previous_event_utc", "next_event_utc",
        "interval_days", "body_median_interval_days", "warning"
    ),
    "virgostars-events-20261110.vseph-p0": None,
}


def file_chunks(dataset: Path, filename: str):
    """Return a checked, ordered list of Git-tracked immutable part files."""
    if filename not in ALLOWED:
        raise ValueError(f"Unexpected dataset file {filename!r}")
    direct = dataset / filename
    parts_dir = dataset / (filename + ".parts")
    if direct.is_file() and parts_dir.exists():
        raise ValueError(f"Ambiguous direct and multipart storage: {filename}")
    if direct.is_file():
        return [direct]
    if not parts_dir.is_dir():
        raise FileNotFoundError(f"Dataset file or parts not present: {filename}")
    chunks = sorted(parts_dir.iterdir())
    if not chunks:
        raise ValueError(f"No parts for {filename}")
    for i, part in enumerate(chunks):
        match = PART.fullmatch(part.name)
        if not part.is_file() or match is None or int(match.group(1)) != i:
            raise ValueError(f"Missing, unexpected, or non-contiguous part: {part.name}")
    return chunks


def rebuild(dataset: Path, output: Path, filename: str, meta: dict) -> Path:
    pieces = file_chunks(dataset, filename)
    output.mkdir(parents=True, exist_ok=True)
    target = output / filename
    digest = hashlib.sha256()
    count = 0
    tmp = target.with_name(target.name + ".tmp")
    try:
        with tmp.open("wb") as dest:
            for part in pieces:
                with part.open("rb") as source:
                    while data := source.read(1024 * 1024):
                        dest.write(data)
                        digest.update(data)
                        count += len(data)
        if count != meta["size"] or digest.hexdigest() != meta["sha256"]:
            raise ValueError(
                f"Integrity mismatch {filename}: size={count}, sha256={digest.hexdigest()}; "
                f"expected size={meta['size']}, sha256={meta['sha256']}"
            )
        if ALLOWED[filename] is not None:
            with tmp.open(encoding="utf-8", newline="") as source:
                reader = csv.reader(source)
                first = tuple(next(reader, []))
                if first != ALLOWED[filename]:
                    raise ValueError(f"Unexpected CSV header: {filename}: {first}")
                n = sum(1 for _ in reader)
            expected = meta["rows"]
            if n != expected:
                raise ValueError(f"Count mismatch {filename}: {n} != {expected}")
        elif not tmp.open("rb").read(8) == b"VSEPH-P0":
            raise ValueError("Invalid experimental VSEPH-P0 magic")
        tmp.replace(target)
    except Exception:
        tmp.unlink(missing_ok=True)
        raise
    return target


def main() -> None:
    default_dir = Path(__file__).resolve().parent.parent / "datasets" / "2026-11-10"
    parser = argparse.ArgumentParser(description="Rebuild/verify VirgoStars 2026-11-10 public snapshot")
    parser.add_argument("--dataset", type=Path, default=default_dir)
    parser.add_argument("--out", type=Path, default=None,
                        help="Destination for reconstructed files (default <dataset>/_assembled)")
    args = parser.parse_args()
    dataset = args.dataset.resolve()
    output = (args.out or dataset / "_assembled").resolve()
    if output == dataset:
        parser.error("Refusing to overwrite tracked dataset files; choose another output directory")
    manifest = json.loads((dataset / "manifest-publication.json").read_text(encoding="utf-8"))
    for filename, info in manifest["files"].items():
        target = rebuild(dataset, output, filename, info)
        print(f"PASS {filename}: {info['size']} bytes / {info['sha256']} -> {target}")
    print(f"Verified all {len(manifest['files'])} published scientific archive files.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"DATASET VERIFICATION FAILED: {exc}", file=sys.stderr)
        sys.exit(1)
