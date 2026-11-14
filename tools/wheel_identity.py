#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Validate the identity of a prebuilt, offline VirgoStars Python wheel.

No third-party packaging library is needed, and this is NOT an installer.
The caller must have checked all ZIP paths and dataset exclusion first.
"""
from __future__ import annotations

from email import policy
from email.parser import BytesParser
from pathlib import Path
import re
import zipfile

PROJECT = "virgostars_event_ephemeris"
MAX_METADATA_BYTES = 256 * 1024


def _single_header(archive: zipfile.ZipFile, name: str, field: str) -> str:
    info = archive.getinfo(name)
    if info.file_size > MAX_METADATA_BYTES:
        raise ValueError(f"wheel metadata too large: {name!r}")
    with archive.open(info) as source:
        # Read no more than the permitted amount, even for malformed ZIPs.
        raw = source.read(MAX_METADATA_BYTES + 1)
    if len(raw) > MAX_METADATA_BYTES:
        raise ValueError(f"wheel metadata too large: {name!r}")
    try:
        message = BytesParser(policy=policy.default).parsebytes(raw, headersonly=True)
    except (ValueError, UnicodeError) as exc:
        raise ValueError(f"invalid wheel metadata headers: {name!r}") from exc
    matches = message.get_all(field, [])
    if len(matches) != 1:
        raise ValueError(f"wheel must contain exactly one {field} field in {name!r}")
    return str(matches[0]).strip()


def validate_wheel_identity(wheel: Path, archive: zipfile.ZipFile, dist_info: str) -> None:
    """Require filename, .dist-info, PEP 566 metadata and project identity to agree.

    This gate is intentionally project-specific: it verifies VirgoStars M0
    wheels rather than serving as a general-purpose PEP 427 wheel parser.
    """
    filename = wheel.name
    if not filename.endswith(".whl"):
        raise ValueError("invalid VirgoStars wheel filename")
    parts = filename[:-4].split("-")
    if len(parts) not in (5, 6):
        raise ValueError("invalid VirgoStars wheel filename")
    distribution, version = parts[:2]
    if (
        distribution != PROJECT
        or not re.fullmatch(r"[0-9][A-Za-z0-9_.+!]*", version)
        or any(not re.fullmatch(r"[A-Za-z0-9_.]+", tag) for tag in parts[-3:])
        or (len(parts) == 6 and re.fullmatch(r"[0-9][A-Za-z0-9_]*", parts[2]) is None)
    ):
        raise ValueError("invalid VirgoStars wheel filename or project distribution")
    expected = f"{distribution}-{version}.dist-info"
    if dist_info != expected:
        raise ValueError("wheel .dist-info does not match wheel filename")

    wheel_version = _single_header(archive, f"{dist_info}/WHEEL", "Wheel-Version")
    if wheel_version != "1.0":
        raise ValueError("unsupported or malformed wheel Wheel-Version")
    name = _single_header(archive, f"{dist_info}/METADATA", "Name")
    metadata_version = _single_header(archive, f"{dist_info}/METADATA", "Version")
    normalized = re.sub(r"[-_.]+", "_", name).lower()
    if normalized != PROJECT:
        raise ValueError("wheel METADATA Name does not identify VirgoStars Event Ephemeris")
    if metadata_version != version:
        raise ValueError("wheel METADATA Version does not match wheel filename")
