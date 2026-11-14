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
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

from audit_report_json import decode_audit_report

ROOT = Path(__file__).resolve().parent.parent
RUSTUP_URL = "https://rustup.rs/"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")


def validate_audit_shape(report: object, label: str) -> None:
    """Require actual scalar types and coherent counts before audit parity.

    Python's JSON values equate True with 1 and 1.0 with 1; checking only
    equality between two reports could accept matching malformed metadata.
    """
    if not isinstance(report, dict):
        raise RuntimeError(f"{label} P0 audit must report a JSON object")
    digest = report.get("sha256")
    if type(digest) is not str or SHA256.fullmatch(digest) is None:
        raise RuntimeError(f"{label} P0 audit must report a canonical SHA-256")
    if type(report.get("bytes")) is not int or report["bytes"] <= 0:
        raise RuntimeError(f"{label} P0 audit must report a positive file byte length")
    blocks = report.get("blocks")
    records = report.get("records")
    if type(blocks) is not int or type(records) is not int or not 0 <= blocks <= records:
        raise RuntimeError(f"{label} P0 audit has invalid block/record counts")
    if (blocks == 0) != (records == 0):
        raise RuntimeError(f"{label} P0 audit has inconsistent empty archive counts")
    sections = report.get("sections")
    if (
        not isinstance(sections, dict)
        or any(key not in ("synod", "motion", "ingress") for key in sections)
        or any(type(count) is not int or count < 0 for count in sections.values())
        or sum(sections.values()) != records
    ):
        raise RuntimeError(f"{label} P0 audit has invalid family section counts")

GATES = (
    # Abort before compilation if the reviewed Cargo.lock is stale. A normal
    # cargo invocation would otherwise silently rewrite the tracked lockfile.
    ("Cargo lockfile reproducibility", ("metadata", "--locked", "--no-deps", "--format-version", "1")),
    ("Rust format", ("fmt", "--all", "--", "--check")),
    ("Rust Clippy", ("clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings")),
    ("Rust tests", ("test", "--locked", "--workspace")),
    ("Rust release build", ("build", "--locked", "--workspace", "--release")),
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
        py_result = invoke(python_cmd, cwd=root, check=True, capture_output=True, text=True)
        # The Rust CLI performs complete block decoding AND full-file hashing.
        cli_cmd = [
            executable, "run", "--locked", "-p", "virgostars-events-p0", "--bin",
            "p0-verify", "--", str(archive), "--sha256", expected_sha256 or "",
        ]
        print("\n[Rust P0 complete archive audit]", flush=True)
        rust_result = invoke(cli_cmd, cwd=root, check=True, capture_output=True, text=True)

        # Fail before checking audit fields if JSON itself is ambiguous:
        # duplicate object keys and nonfinite numbers must never be silently
        # interpreted differently by a Python or Rust JSON parser.
        python_report = decode_audit_report(py_result.stdout, source="Python")
        rust_report = decode_audit_report(rust_result.stdout, source="Rust")
        validate_audit_shape(python_report, "Python")
        validate_audit_shape(rust_report, "Rust")
        if rust_report.get("format") != "VSEPH-P0-EXPERIMENTAL":
            raise RuntimeError("Rust P0 audit reported an unexpected archive format")
        fields = ("sha256", "bytes", "blocks", "records", "sections")
        differences = [key for key in fields if python_report.get(key) != rust_report.get(key)]
        if differences:
            raise RuntimeError(
                "Python/Rust archive audit mismatch in: " + ", ".join(differences)
            )
        # The two independent tools could agree on a bogus physical byte
        # count. Also compare against the actual file on disk.
        if rust_report["bytes"] != archive.stat().st_size:
            raise RuntimeError("P0 auditor physical byte length differs from source file")
        if rust_report.get("sha256") != expected_sha256:
            raise RuntimeError("Rust/Python digest differs from the trusted manifest")
        if rust_report.get("sha256_checked_against_expected") is not True:
            raise RuntimeError("Rust CLI did not confirm trusted digest verification")
        if rust_report.get("coverage_certified") is not False:
            raise RuntimeError("integrity audit must never claim astronomical coverage")
        print(
            "Rust/Python byte-audit parity: "
            + json.dumps({field: rust_report[field] for field in fields}, sort_keys=True),
            flush=True,
        )

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
