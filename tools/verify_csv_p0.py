#!/usr/bin/env python3
# Copyright (c) 2026 Cesare-ml. CPAL-1.0; see LICENSE/NOTICE.
# Powered by VirgoStars — https://www.virgostars.it/
"""Verify EVERY normalized scientific CSV field against VSEPH-P0 records.

This is a read-only development oracle, not VSEPH v1 or astronomical proof.
Requires Python 3.10+ standard library. The independently encoded P0 header,
blocks and checksums are first checked by audit_p0_bytes.py.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import json
import re
import sqlite3
import struct
import tempfile
import zlib
from pathlib import Path

from audit_p0_bytes import audit_file

EPOCH = dt.date(1970, 1, 1).toordinal()
DAY_US = 86_400_000_000
I64_MIN, I64_MAX = -(1 << 63), (1 << 63) - 1
DATE = re.compile(r"(\d{4})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)(?:\.(\d{1,6}))?\+00\Z")
SIGN_INDEX_TEXT = frozenset(str(i) for i in range(12))
HEADERS = {
    "synod": ("sinodi.csv", ("id", "elemento", "dataora_utc", "dataora_extra_utc_json")),
    "motion": ("moto_pianeti.csv", ("id", "moto", "segno", "dataora_utc", "elemento_celeste")),
    "ingress": ("ingresso_segni.csv", ("id", "segno", "dataora_utc", "elemento_celeste")),
}


def utc_microseconds(value: str) -> int:
    """Exact UTC Gregorian date conversion, including years 0001–0099."""
    match = DATE.fullmatch(value)
    if not match:
        raise ValueError(f"invalid UTC timestamp {value!r}")
    year, month, day, hour, minute, second = map(int, match.groups()[:6])
    micro = int((match.group(7) or "").ljust(6, "0"))
    day_number = dt.date(year, month, day).toordinal()
    if hour > 23 or minute > 59 or second > 59:
        raise ValueError(f"invalid UTC civil time {value!r}")
    t = (day_number - EPOCH) * DAY_US + ((hour * 60 + minute) * 60 + second) * 1_000_000 + micro
    if not I64_MIN <= t <= I64_MAX:
        raise ValueError("timestamp outside signed i64")
    return t


def positive_id(value: str) -> int:
    if not value.isascii() or not value.isdecimal() or value.startswith("0"):
        raise ValueError(f"invalid source ID {value!r}")
    out = int(value)
    if not 0 < out <= I64_MAX:
        raise ValueError("source ID outside supported source-table range")
    return out


def sign_index(value: str) -> int:
    if value not in SIGN_INDEX_TEXT:
        raise ValueError(f"invalid sign index {value!r}")
    return int(value)


def normalized_row(family: str, row: dict) -> tuple[str, int, list]:
    source_id = positive_id(row["id"])
    body = row["elemento"] if family == "synod" else row["elemento_celeste"]
    if not body or body.strip() != body:
        raise ValueError(f"invalid body name {body!r}")
    t = utc_microseconds(row["dataora_utc"])
    if family == "synod":
        raw = row["dataora_extra_utc_json"]
        # PostgreSQL NULL is an empty field in the public scientific CSV;
        # JSON [] is distinct and is retained as an explicitly empty array.
        extras = None if raw == "" else json.loads(raw)
        if extras is not None:
            if not isinstance(extras, list) or any(not isinstance(x, str) for x in extras):
                raise ValueError("invalid synod phase JSON")
            extras = [utc_microseconds(x) for x in extras]
            if any(a >= b for a, b in zip([t] + extras[:-1], extras)):
                raise ValueError("synod phases must increase after anchor")
        fields = [t, extras]
    elif family == "motion":
        direction = row["moto"]
        if direction not in ("d", "r"):
            raise ValueError(f"invalid motion direction {direction!r}")
        fields = [t, sign_index(row["segno"]), direction]
    else:
        fields = [t, sign_index(row["segno"])]
    return body, source_id, fields


def uvar(data: bytes, pos: int) -> tuple[int, int]:
    value = 0
    for step in range(10):
        if pos >= len(data):
            raise ValueError("truncated varint")
        b = data[pos]
        pos += 1
        if step == 9 and b > 1:
            raise ValueError("varint exceeds unsigned 64-bit")
        value |= (b & 0x7f) << (step * 7)
        if not b & 0x80:
            # Match Rust take_varint(): redundant top zero groups are not
            # canonical LEB128 and must never be accepted as equivalent bytes.
            if step > 0 and b == 0:
                raise ValueError("non-canonical unsigned varint")
            return value, pos
    raise ValueError("varint too long")


def decoded_block(family: str, plain: bytes, count: int):
    if len(plain) < 8:
        raise ValueError("truncated P0 base timestamp")
    previous = struct.unpack_from("<q", plain)[0]
    pos = 8
    for n in range(count):
        source_id, pos = uvar(plain, pos)
        delta, pos = uvar(plain, pos)
        instant = previous + delta
        if not I64_MIN <= instant <= I64_MAX or (n == 0 and delta != 0) or (n > 0 and instant <= previous):
            raise ValueError("invalid P0 record ordering")
        previous = instant
        if family == "synod":
            code, pos = uvar(plain, pos)
            if code > 1024:
                raise ValueError("synod phase count excessive")
            if code == 0:
                extras = None
            else:
                extras = []
                phase = instant
                for _ in range(code - 1):
                    extra_delta, pos = uvar(plain, pos)
                    phase += extra_delta
                    if extra_delta == 0 or phase > I64_MAX:
                        raise ValueError("invalid P0 phase time")
                    extras.append(phase)
            fields = [instant, extras]
        elif family == "motion":
            if pos + 2 > len(plain):
                raise ValueError("truncated P0 station fields")
            sign, direction = plain[pos], plain[pos + 1]
            pos += 2
            if sign > 11 or direction > 1:
                raise ValueError("invalid station sign/direction")
            fields = [instant, sign, "r" if direction else "d"]
        elif family == "ingress":
            if pos >= len(plain) or plain[pos] > 11:
                raise ValueError("invalid P0 ingress sign")
            fields = [instant, plain[pos]]
            pos += 1
        else:
            raise ValueError(f"unknown event family {family!r}")
        yield source_id, fields
    if pos != len(plain):
        raise ValueError("trailing P0 record bytes")


def verify(csv_dir: Path, archive_path: Path, expected_sha256: str | None = None) -> dict:
    """Compare each record by exact (family, body, original row ID)."""
    checked = audit_file(archive_path, expected_sha256)
    counts = {family: 0 for family in HEADERS}
    digests = {}
    with tempfile.TemporaryDirectory(prefix="vseph-parity-") as tmp:
        connection = sqlite3.connect(str(Path(tmp) / "expected.sqlite"))
        try:
            connection.execute("CREATE TABLE expected(family TEXT, body TEXT, source_id INTEGER, fields TEXT NOT NULL, PRIMARY KEY(family,body,source_id))")
            with connection:
                for family, (filename, columns) in HEADERS.items():
                    path = csv_dir / filename
                    csv_hash = hashlib.sha256()
                    with path.open("rb") as binary:
                        for chunk in iter(lambda: binary.read(1024 * 1024), b""):
                            csv_hash.update(chunk)
                    digests[filename] = csv_hash.hexdigest()
                    with path.open(encoding="utf-8-sig", newline="") as src:
                        records = csv.DictReader(src)
                        if records.fieldnames != list(columns):
                            raise ValueError(f"unexpected normalized CSV columns: {filename}: {records.fieldnames!r}")
                        for line, row in enumerate(records, 2):
                            try:
                                if set(row) != set(columns) or any(value is None for value in row.values()):
                                    raise ValueError("unexpected or missing CSV fields")
                                body, source_id, fields = normalized_row(family, row)
                                canonical = json.dumps(fields, separators=(",", ":"), ensure_ascii=False)
                                connection.execute("INSERT INTO expected VALUES(?,?,?,?)", (family, body, source_id, canonical))
                            except (ValueError, TypeError, sqlite3.IntegrityError, KeyError) as exc:
                                raise ValueError(f"{filename}:{line}: {exc}") from exc
                            counts[family] += 1
            content = archive_path.read_bytes()   # prototype max-file guard already enforced by audit_file
            length = struct.unpack_from("<I", content, 8)[0]
            header = json.loads(content[12:12 + length])
            base = 12 + length
            compared = 0
            with connection:
                for item in header["table_of_contents"]:
                    family, body = item["family"], item["body"]
                    begin = base + item["start"]
                    block = zlib.decompress(content[begin:begin + item["length"]])
                    for source_id, fields in decoded_block(family, block, item["count"]):
                        row = connection.execute(
                            "SELECT fields FROM expected WHERE family=? AND body=? AND source_id=?",
                            (family, body, source_id),
                        ).fetchone()
                        if row is None:
                            raise ValueError(f"binary event absent from CSV: {(family, body, source_id)!r}")
                        actual = json.dumps(fields, separators=(",", ":"), ensure_ascii=False)
                        if row[0] != actual:
                            raise ValueError(f"scientific field mismatch for {(family, body, source_id)!r}: csv={row[0]} archive={actual}")
                        connection.execute(
                            "DELETE FROM expected WHERE family=? AND body=? AND source_id=?",
                            (family, body, source_id),
                        )
                        compared += 1
            leftover = connection.execute("SELECT family,body,source_id FROM expected LIMIT 1").fetchone()
            if leftover is not None:
                raise ValueError(f"CSV event absent from binary: {leftover!r}")
        finally:
            connection.close()
    if counts != checked["sections"] or sum(counts.values()) != compared:
        raise ValueError(f"CSV and binary section counts disagree: {counts!r}")
    return {"result": "exact_scientific_field_parity", "records_compared": compared,
            "sections": counts, "blocks_checked": checked["blocks"],
            "archive_sha256": checked["sha256"], "archive_sha256_checked": expected_sha256 is not None,
            "csv_sha256": digests, "coverage_certified": False}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv_dir", type=Path, help="folder with normalized sinodi.csv, moto_pianeti.csv and ingresso_segni.csv")
    parser.add_argument("archive", type=Path, help="experimental VSEPH-P0 binary")
    parser.add_argument("--sha256", help="optional 64-char SHA-256 from an independent trusted manifest")
    args = parser.parse_args()
    if args.sha256 and not re.fullmatch(r"[0-9a-f]{64}", args.sha256):
        parser.error("--sha256 must be 64 lowercase hex digits")
    print(json.dumps(verify(args.csv_dir, args.archive, args.sha256), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
