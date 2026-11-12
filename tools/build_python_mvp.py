#!/usr/bin/env python3
# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Build, verify and retain one local Python M0 wheel (never publish it).

Run on a machine with Rust/Cargo and maturin installed. Fail closed: first
validate Rust, then build into an isolated temporary directory, then install
and exercise the *real* wheel via verify_python_wheel.py. Only a wheel that
passes all gates is copied to dist/, without replacing existing artifacts.
"""
from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import sysconfig
import tempfile
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]


def maturin_executable() -> Path | None:
    """Locate the maturin executable belonging to this Python first."""
    scripts = Path(sysconfig.get_path("scripts"))
    local = scripts / ("maturin.exe" if os.name == "nt" else "maturin")
    if local.is_file():
        return local
    found = shutil.which("maturin")
    return Path(found) if found else None


def run(command: Sequence[str | Path]) -> None:
    print("\n[M0 build]", " ".join(str(v) for v in command), flush=True)
    sanitized_env = os.environ.copy()
    for name in ("PYTHONPATH", "PYTHONHOME", "PYTHONUSERBASE"):
        sanitized_env.pop(name, None)
    subprocess.run(
        [str(value) for value in command], cwd=ROOT, env=sanitized_env, check=True
    )


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def save_verified_wheel(wheel: Path, out_dir: Path) -> Path:
    """Publish a fully copied and hash-verified wheel atomically, without overwrite."""
    out_dir.mkdir(parents=True, exist_ok=True)
    output = out_dir / wheel.name
    source_digest = digest(wheel)
    if output.exists() or output.is_symlink():
        if output.is_symlink() or not output.is_file() or digest(output) != source_digest:
            raise FileExistsError(f"existing wheel differs; refusing overwrite: {output}")
        return output

    staged: Path | None = None
    try:
        # Same-directory staging guarantees that publishing by hard link is
        # atomic and cannot expose a half-copied wheel under its final name.
        with tempfile.NamedTemporaryFile(
            mode="wb", prefix=".virgostars-m0-", suffix=".part",
            dir=out_dir, delete=False,
        ) as destination:
            staged = Path(destination.name)
            with wheel.open("rb") as source:
                shutil.copyfileobj(source, destination)
            destination.flush()
            os.fsync(destination.fileno())
        if digest(staged) != source_digest or digest(wheel) != source_digest:
            raise OSError("copied wheel checksum mismatch or source changed during copy")
        try:
            # No-clobber publication: unlike os.replace, os.link never
            # overwrites an artifact another builder has just produced.
            os.link(staged, output)
        except FileExistsError:
            if output.is_symlink() or not output.is_file() or digest(output) != source_digest:
                raise FileExistsError(f"concurrent wheel differs; refusing overwrite: {output}")
        if digest(output) != source_digest:
            raise OSError("published wheel checksum mismatch")
        return output
    finally:
        if staged is not None:
            staged.unlink(missing_ok=True)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build and gate a local Rust/PyO3 Python M0 wheel")
    parser.add_argument("--out-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--archive", type=Path, help="Optional local authorized P0 archive")
    parser.add_argument("--sha256", help="Trusted SHA-256 for --archive")
    parser.add_argument("--synods-csv", type=Path, help="Optional source CSV parity")
    args = parser.parse_args(argv)
    if bool(args.archive) != bool(args.sha256):
        parser.error("--archive and --sha256 must be given together")
    if args.sha256 and re.fullmatch(r"[a-fA-F0-9]{64}", args.sha256) is None:
        parser.error("--sha256 must be 64 hexadecimal characters")
    if args.archive and not args.archive.is_file():
        parser.error("--archive must be an existing file")
    if args.synods_csv and (not args.archive or not args.synods_csv.is_file()):
        parser.error("--synods-csv requires --archive and an existing CSV")
    if not (ROOT / "Cargo.toml").is_file() or not (ROOT / "pyproject.toml").is_file():
        print("FAIL: run from the source checkout containing Cargo.toml and pyproject.toml", file=sys.stderr)
        return 1
    maturin = maturin_executable()
    if maturin is None:
        print("FAIL: maturin missing. Install it for this Python interpreter first.", file=sys.stderr)
        return 1

    try:
        # Catch Python facade, CLI and independent CSV-oracle regressions
        # before the much slower locked Cargo build. This source gate is
        # strictly non-native; successful wheels still require the entire
        # real PyO3/50-case installed test suite below.
        run([sys.executable, ROOT / "tools" / "validate_python_sources.py"])
        run([sys.executable, ROOT / "tools" / "validate_rust.py"])
        with tempfile.TemporaryDirectory(prefix="virgostars-m0-build-") as temp:
            wheels_dir = Path(temp) / "wheels"
            run([maturin, "build", "--locked", "--release", "--out", wheels_dir, "--interpreter", sys.executable])
            wheels = sorted(wheels_dir.glob("*.whl"))
            if len(wheels) != 1:
                raise RuntimeError(f"expected exactly one new wheel, found {len(wheels)}")
            command: list[str | Path] = [sys.executable, ROOT / "tools" / "verify_python_wheel.py", "--wheel", wheels[0]]
            if args.archive:
                command.extend(["--archive", args.archive.resolve(), "--sha256", args.sha256.lower()])
            if args.synods_csv:
                command.extend(["--synods-csv", args.synods_csv.resolve()])
            run(command)
            saved = save_verified_wheel(wheels[0], args.out_dir.resolve())
            print(f"\nPASS: verified wheel retained at {saved} (SHA-256 {digest(saved)})")
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(f"FAIL: Rust/Python MVP build or verification failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
