# The contents of this file are subject to the Common Public Attribution
# License Version 1.0 (CPAL-1.0); see LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Independent, dependency-free byte-contract audit for experimental VSEPH-P0.

This is a TEST oracle for the provisional Rust decoder, not another public
runtime reader or a VSEPH v1 specification. Never infers event completeness.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import zlib
from collections import Counter
from pathlib import Path

MAGIC = b"VSEPH-P0"
MAX_HEADER = 5_000_000
MAX_FILE = 256_000_000  # explicit prototype cap before allocating full archive
MAX_BLOCK = 5_000_000
MAX_RAW = 25_000_000
MAX_COUNT = 512
FAMILIES = ("ingress", "motion", "synod")


def _varint(data: bytes, pos: int) -> tuple[int, int]:
    value = 0
    for step in range(10):
        if pos >= len(data):
            raise ValueError("truncated varint")
        byte = data[pos]
        pos += 1
        if step == 9 and byte > 1:
            raise ValueError("varint overflow")
        value |= (byte & 127) << (7 * step)
        if byte < 128:
            # Shortest unsigned LEB128 only: a redundant zero high group
            # must not produce a second binary encoding for the same record.
            if step > 0 and byte == 0:
                raise ValueError("non-canonical unsigned varint")
            return value, pos
    raise ValueError("varint too long")


def _decode(family: str, data: bytes, expected: int, seen_source_ids: set[int]) -> tuple[int, int, int]:
    if len(data) < 8:
        raise ValueError("missing signed i64 base timestamp")
    first = struct.unpack_from("<q", data)[0]
    previous = first
    pos = 8
    for n in range(expected):
        source_id, pos = _varint(data, pos)
        if source_id == 0 or source_id in seen_source_ids:
            raise ValueError(f"zero or duplicate original source ID in {family}: {source_id}")
        seen_source_ids.add(source_id)
        delta, pos = _varint(data, pos)
        current = previous + delta
        if current > (1 << 63) - 1 or current < -(1 << 63):
            raise ValueError("timestamp i64 overflow")
        if (n == 0 and delta != 0) or (n > 0 and current <= previous):
            raise ValueError("event timestamps not strictly increasing")
        previous = current
        if family == "synod":
            code, pos = _varint(data, pos)
            if code > 1024:
                raise ValueError("too many phases")
            phase = current
            for _ in range(max(0, code - 1)):
                d, pos = _varint(data, pos)
                if d == 0:
                    raise ValueError("non-increasing phase")
                phase += d
                if phase > (1 << 63) - 1:
                    raise ValueError("phase timestamp overflow")
        elif family == "motion":
            if pos + 2 > len(data):
                raise ValueError("truncated motion fields")
            sign, direction = data[pos], data[pos + 1]
            pos += 2
            if sign > 11 or direction > 1:
                raise ValueError("invalid motion fields")
        elif family == "ingress":
            if pos >= len(data) or data[pos] > 11:
                raise ValueError("invalid ingress sign")
            pos += 1
        else:
            raise ValueError("unknown family")
    if pos != len(data):
        raise ValueError("trailing decoded bytes")
    return first, previous, expected


def audit_file(path: str | Path, expected_sha256: str | None = None) -> dict:
    path = Path(path)
    if path.stat().st_size > MAX_FILE:
        raise ValueError("P0 audit input exceeds maximum file size")
    raw_file = path.read_bytes()
    sha = hashlib.sha256(raw_file).hexdigest()
    if expected_sha256 is not None and sha != expected_sha256:
        raise ValueError("archive SHA-256 does not match expected snapshot")
    if len(raw_file) < 12 or raw_file[:8] != MAGIC:
        raise ValueError("wrong or truncated VSEPH-P0 magic")
    header_len = struct.unpack_from("<I", raw_file, 8)[0]
    if not 0 < header_len <= MAX_HEADER or header_len + 12 > len(raw_file):
        raise ValueError("invalid JSON header size")
    doc = json.loads(raw_file[12:12 + header_len])
    if not isinstance(doc, dict):
        raise ValueError("P0 header must be a JSON object")
    version = doc.get("format_version")
    # Python considers True == 1 and 1.0 == 1; Rust serde_json does not.
    # Explicit types are essential for independent cross-language auditing.
    if (
        doc.get("format") != "VSEPH-P0-EXPERIMENTAL"
        or type(version) is not list
        or len(version) != 3
        or any(type(value) is not int for value in version)
        or version != [0, 0, 1]
        or doc.get("codec") != "zlib"
        or type(doc.get("block_records_max")) is not int
        or doc["block_records_max"] != MAX_COUNT
    ):
        raise ValueError("unsupported P0 version or codec")
    toc = doc["table_of_contents"]
    if not isinstance(toc, list):
        raise ValueError("invalid table of contents")
    offset = 0
    previous_key = None
    previous_last = None
    counts = Counter()
    # IDs are globally unique per original PostgreSQL table/family, including
    # across celestial bodies and compressed blocks.
    seen_source_ids = {family: set() for family in FAMILIES}
    base = 12 + header_len
    for entry in toc:
        family, body = entry["family"], entry["body"]
        if family not in FAMILIES or not isinstance(body, str) or not body:
            raise ValueError("invalid TOC family or body")
        key = (family, body)
        if previous_key is not None and key < previous_key:
            raise ValueError("TOC not sorted by family/body")
        start, size, raw_len, count = (entry[name] for name in ("start", "length", "raw_length", "count"))
        if not all(type(i) is int for i in (start, size, raw_len, count)) or start != offset or not 0 < size <= MAX_BLOCK or not 8 <= raw_len <= MAX_RAW or not 0 < count <= MAX_COUNT:
            raise ValueError("unsafe block offset, size, or count")
        offset += size
        if base + offset > len(raw_file):
            raise ValueError("block extends beyond file")
        compressed = raw_file[base + start:base + offset]
        dec = zlib.decompressobj()
        block = dec.decompress(compressed, raw_len + 1)
        if len(block) != raw_len or not dec.eof or dec.unused_data or dec.unconsumed_tail or dec.flush():
            raise ValueError("truncated or trailing compressed stream")
        if hashlib.sha256(block).hexdigest() != entry["raw_sha256"]:
            raise ValueError("block SHA-256 mismatch")
        first, last, count = _decode(family, block, count, seen_source_ids[family])
        if str(first) != entry["min_us"] or str(last) != entry["max_us"]:
            raise ValueError("TOC timestamps do not match decoded records")
        if key == previous_key and previous_last >= first:
            raise ValueError("overlapping time blocks")
        previous_key, previous_last = key, last
        counts[family] += count
    if base + offset != len(raw_file):
        raise ValueError("unindexed bytes at end of archive")
    sections = doc.get("sections")
    if (
        not isinstance(sections, dict)
        or any(type(value) is not int or value < 0 for value in sections.values())
        or dict(counts) != sections
    ):
        raise ValueError("section record totals mismatch")
    return {"sha256": sha, "bytes": len(raw_file), "blocks": len(toc), "records": sum(counts.values()), "sections": dict(sorted(counts.items()))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="VSEPH-P0 file to audit")
    parser.add_argument("--sha256", help="optional expected complete-file SHA-256")
    args = parser.parse_args()
    print(json.dumps(audit_file(args.archive, args.sha256), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
