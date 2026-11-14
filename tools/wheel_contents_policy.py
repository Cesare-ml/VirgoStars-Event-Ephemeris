#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed offline check of prebuilt M0 wheel ZIP contents.

The event dataset is deliberately NOT distributed inside the Python wheel.
This is a packaging safety check, not scientific or legal certification.
"""
from __future__ import annotations

from pathlib import Path
import re
import stat
import zipfile

from wheel_identity import validate_wheel_identity

# These are all source/archive formats used by the project's owner dataset.
# A compressed archive with an unrelated extension cannot be classified by
# filename alone; a packaging review is still mandatory before any release.
DATA_SUFFIXES = (
    ".csv", ".vseph", ".vseph-p0", ".p0", ".sqlite", ".sqlite3", ".db",
    ".csv.gz", ".csv.zst", ".p0.gz", ".p0.zst", ".vseph-p0.gz",
    ".vseph-p0.zst", ".sqlite.gz", ".sqlite3.zst", ".db.gz",
)
P0_MAGIC = b"VSEPH-P0"
MAX_MEMBERS = 4096
MAX_EXPANSION_RATIO = 1000
MAX_UNCOMPRESSED_MEMBER = 64 * 1024 * 1024
MAX_UNCOMPRESSED_TOTAL = 128 * 1024 * 1024

# ZIP member names may be accepted on Linux but redirect or conflict with
# special Windows names when the same wheel is installed on Windows.
_WINDOWS_INVALID = frozenset('<>:"|?*')
_WINDOWS_DEVICE = re.compile(
    r"(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?\Z",
    re.IGNORECASE,
)


def unsafe_windows_component(part: str) -> bool:
    """Reject Win32 forbidden components without consulting host OS rules."""
    return (
        not part or part[-1] in (".", " ")
        or any(ord(ch) < 32 or ch in _WINDOWS_INVALID for ch in part)
        or _WINDOWS_DEVICE.fullmatch(part) is not None
    )


def inspect_wheel_contents(wheel: Path) -> None:
    """Reject malformed/unsafe ZIP members and accidental scientific datasets.

    Do this before running pip, and never extract an untrusted wheel directly.
    """
    try:
        with zipfile.ZipFile(wheel) as archive:
            names: set[str] = set()
            # Windows wheels must also be safe on case-insensitive filesystems:
            # distinct ZIP names can otherwise extract over the same path.
            portable_paths: set[str] = set()
            # Track both explicit and implicit directories. A ZIP may contain
            # a regular file used as a parent directory of a later member,
            # which cannot be extracted consistently on any target OS.
            portable_files: set[str] = set()
            portable_directories: set[str] = set()
            total = 0
            infos = archive.infolist()
            if len(infos) > MAX_MEMBERS:
                raise ValueError("wheel contains too many ZIP members")
            for info in infos:
                name = info.filename
                parts = name.split("/")
                if (
                    not name or name.startswith("/") or "\\" in name
                    or re.match(r"^[A-Za-z]:", name)
                    or any(part in ("", ".", "..") for part in parts[:-1])
                    or (not info.is_dir() and parts[-1] in ("", ".", ".."))
                    or any(unsafe_windows_component(part) for part in parts if part)
                ):
                    raise ValueError(f"unsafe wheel member path: {name!r}")
                if not info.is_dir() and (not parts[-1] or name in names):
                    raise ValueError(f"empty or duplicate wheel member: {name!r}")
                # ZIP entry names are case-sensitive, but the installed wheel
                # may land on a case-insensitive Windows filesystem. Reject a
                # second name resolving to an already seen destination, even
                # when both entries contain individually valid source files.
                portable = name.rstrip("/").casefold()
                if portable in portable_paths:
                    raise ValueError(
                        f"case-insensitive wheel path collision: {name!r}"
                    )
                # A filename can be unique yet conflict with an implicit
                # directory created for another member. Check both orders
                # and use the same Windows case-folding as duplicate names.
                components = portable.split("/")
                parents = {"/".join(components[:i]) for i in range(1, len(components))}
                if parents & portable_files or (
                    not info.is_dir() and portable in portable_directories
                ):
                    raise ValueError(f"wheel file/directory path collision: {name!r}")
                portable_paths.add(portable)
                portable_directories.update(parents)
                if info.is_dir():
                    portable_directories.add(portable)
                else:
                    portable_files.add(portable)
                if (info.external_attr >> 16) & 0o170000 == stat.S_IFLNK:
                    raise ValueError(f"wheel contains a symbolic link: {name!r}")
                if info.file_size > MAX_UNCOMPRESSED_MEMBER:
                    raise ValueError(f"wheel member exceeds the safe size limit: {name!r}")
                # A small compressed ZIP payload that expands thousands of
                # times can consume large amounts of CPU/memory during pip
                # installation even when under the absolute size ceilings.
                if (info.file_size > 1024 * 1024 and
                        info.file_size > max(info.compress_size, 1) * MAX_EXPANSION_RATIO):
                    raise ValueError(f"wheel member has excessive ZIP expansion: {name!r}")
                total += info.file_size
                if total > MAX_UNCOMPRESSED_TOTAL:
                    raise ValueError("wheel uncompressed size exceeds the safe limit")
                if not info.is_dir():
                    names.add(name)
                    lower = name.lower()
                    if (any(part.lower() in {"dataset", "datasets"} for part in parts[:-1])
                            or lower.endswith(DATA_SUFFIXES)):
                        raise ValueError(f"wheel must not bundle scientific data: {name!r}")
                    # A renamed raw VSEPH-P0 archive is still owner data.
                    # Sniff the exact eight-byte framing magic, never unzip
                    # arbitrary nested archives or infer legal permission.
                    with archive.open(info) as member:
                        if member.read(len(P0_MAGIC)) == P0_MAGIC:
                            raise ValueError(f"wheel contains renamed VSEPH-P0 data: {name!r}")

            dist_info = {name.split("/", 1)[0] for name in names if ".dist-info/" in name}
            if len(dist_info) != 1:
                raise ValueError("wheel must contain exactly one .dist-info directory")
            metadata = next(iter(dist_info))
            if not {f"{metadata}/WHEEL", f"{metadata}/METADATA"}.issubset(names):
                raise ValueError("wheel lacks mandatory WHEEL or METADATA")
            # Verify that this is the *intended* VirgoStars distribution,
            # with matching PEP 427 filename and PEP 566 Name/Version headers.
            # This runs on the same private wheel snapshot pip will install.
            validate_wheel_identity(wheel, archive, metadata)
            bad_member = archive.testzip()
            if bad_member is not None:
                raise ValueError(f"wheel has a corrupt ZIP member: {bad_member!r}")
    except (zipfile.BadZipFile, zipfile.LargeZipFile) as exc:
        raise ValueError(f"invalid Python wheel ZIP: {exc}") from exc
