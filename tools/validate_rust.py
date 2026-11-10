#!/usr/bin/env python3
# The contents of this file are subject to the Common Public Attribution
# License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. All rights reserved.
# Powered by VirgoStars — https://www.virgostars.it/
"""Run mandatory Rust review gates without GitHub Actions (Windows/macOS/Linux).

Works with Python 3.10+. Does not install Rust, change Git branches, merge,
build releases for publication, or modify the astronomical dataset.
"""
from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUSTUP_URL = "https://rustup.rs/"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")

GATES = (
    ("Rust format", ("fmt", "--all", "--", "--check")),
    ("Rust Clippy", ("clippy", "--workspace", "--all-targets", "--", "-D", "warnings")),
    ("Rust tests", ("test", "--workspace")),
    ("Rust release build", ("build", "--workspace", "--release")),
)


def validate_archive_options(
    archive: str | None, expected_sha256: str | None, root: Path = ROOT,
) -> Path | None:
    """Reject partial or untrusted digest arguments before running Cargo."""
    if (archive is None) != (expected_sha256 is None):
        raise ValueError("--archive and --sha256 must be specified together")
    if archive is None:
        return None
    if not SHA256.fullmatch(expected_sha256 or ""):
        raise ValueError("--sha256 must be 64 lowercase hexadecimal characters")
    candidate = Path(archive).expanduser()
    resolved = candidate.resolve() if candidate.is_absolute() else (root / candidate).resolve()
    if not resolved.is_file():
        raise ValueError(f"the archive does not exist: {resolved}")
    return resolved


def run_gates(
    *, root: Path = ROOT, archive: Path | None = None,
    expected_sha256: str | None = None, cargo: str | None = None,
    invoke=subprocess.run,
) -> None:
    """Sequential mandatory gates; stop immediately on any failed command."""
    executable = cargo or shutil.which("cargo")
    if executable is None:
        raise RuntimeError(
            f"Cargo not found on PATH. Install the official Rust toolchain at {RUSTUP_URL}, "
            "restart the terminal, then rerun this command."
        )
    if not (root / "Cargo.toml").is_file():
        raise RuntimeError(f"Cargo workspace not found at {root}")
    if archive is not None and not expected_sha256:
        raise ValueError("cannot audit a real archive without a trusted --sha256")

    for label, args in GATES:
        cmd = [executable, *args]
        print(f"\n[{label}] {' '.join(cmd)}", flush=True)
        invoke(cmd, cwd=root, check=True)

    if archive is not None:
        # The independent Python byte-level validator must also pass.
        auditor = root / "tools" / "audit_p0_bytes.py"
        if not auditor.is_file():
            raise RuntimeError(f"independent byte-level auditor missing: {auditor}")
        python_cmd = [
            sys.executable, str(auditor), str(archive), "--sha256",
            expected_sha256 or "",
        ]
        print("\n[Independent P0 byte audit]", flush=True)
        invoke(python_cmd, cwd=root, check=True)
        # The Rust CLI performs complete block decoding AND full-file hashing.
        cli_cmd = [
            executable, "run", "-p", "virgostars-events-p0", "--bin",
            "p0-verify", "--", str(archive), "--sha256", expected_sha256 or "",
        ]
        print("\n[Experimental Rust/Python P0 archive parity gate]", flush=True)
        invoke(cli_cmd, cwd=root, check=True)

    print("\nAll requested commands completed successfully.", flush=True)
    print(
        "This is a build and byte-integrity check, NOT a scientific "
        "certification of missing astronomical events.",
        flush=True,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archive", help="optional assembled VSEPH-P0 file, absolute or repo-root relative"
    )
    parser.add_argument(
        "--sha256", help="mandatory trusted lowercase SHA-256 when --archive is given"
    )
    args = parser.parse_args(argv)
    try:
        archive = validate_archive_options(args.archive, args.sha256)
        run_gates(archive=archive, expected_sha256=args.sha256)
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as exc:
        print(f"Validation stopped: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
