# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Run independent P0 audit commands with bounded stdout/stderr memory.

subprocess.run(capture_output=True) accumulates every emitted byte before a
JSON parser can reject oversized reports. Drain both pipes concurrently and
terminate the subprocess once either fixed output allowance is exceeded.
"""
from __future__ import annotations

import math
import os
import subprocess
import threading
import time
from pathlib import Path
from typing import Sequence

from audit_report_json import MAX_AUDIT_REPORT_BYTES

MAX_AUDIT_STDERR_BYTES = 8 * 1024
DEFAULT_AUDIT_TIMEOUT_SECONDS = 15 * 60
_READ_SIZE = 4096


def run_bounded_audit(
    command: Sequence[str], *, cwd: Path, timeout_seconds: float = DEFAULT_AUDIT_TIMEOUT_SECONDS
) -> subprocess.CompletedProcess[str]:
    """Return UTF-8 stdout from a successful bounded subprocess.

    Neither stdout nor stderr can grow past their budget in memory. Any
    overrun kills the child and fails closed, rather than silently accepting
    a truncated JSON audit. A deadline also guards silent child processes or
    inherited pipes that never reach EOF. The helper does not kill process trees.
    """
    if (
        type(timeout_seconds) not in (int, float)
        or not math.isfinite(timeout_seconds)
        or timeout_seconds <= 0
    ):
        raise ValueError("audit timeout_seconds must be positive and finite")
    deadline = time.monotonic() + timeout_seconds
    stdout = bytearray()
    stderr = bytearray()
    violations: list[str] = []
    errors: list[OSError] = []
    with subprocess.Popen(
        command, cwd=cwd, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    ) as process:
        assert process.stdout is not None and process.stderr is not None

        def collect(pipe, output: bytearray, limit: int, label: str) -> None:
            try:
                while True:
                    chunk = os.read(pipe.fileno(), _READ_SIZE)
                    if not chunk:
                        return
                    remaining = max(0, limit - len(output))
                    output.extend(chunk[:remaining])
                    if len(chunk) > remaining:
                        violations.append(f"{label} exceeds {limit} bytes")
                        try:
                            process.kill()
                        except OSError:
                            pass  # It may have already exited after writing.
                        return
            except OSError as exc:
                errors.append(exc)
                try:
                    process.kill()
                except OSError:
                    pass

        workers = [
            threading.Thread(target=collect, args=(process.stdout, stdout, MAX_AUDIT_REPORT_BYTES, "stdout"), daemon=True),
            threading.Thread(target=collect, args=(process.stderr, stderr, MAX_AUDIT_STDERR_BYTES, "stderr"), daemon=True),
        ]
        for worker in workers:
            worker.start()
        timed_out = False
        try:
            process.wait(timeout=max(0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            timed_out = True
            process.kill()
            process.wait()
        for worker in workers:
            worker.join(timeout=max(0, deadline - time.monotonic()))
        pipe_timed_out = any(worker.is_alive() for worker in workers)
        returncode = process.returncode

    if timed_out or pipe_timed_out:
        raise RuntimeError(f"P0 audit subprocess exceeded {timeout_seconds:g} seconds")
    if violations:
        raise RuntimeError("P0 audit subprocess output " + violations[0])
    if errors:
        raise RuntimeError(f"P0 audit pipe read failed: {errors[0]}") from errors[0]
    try:
        out_text = stdout.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise RuntimeError("P0 audit stdout is not UTF-8") from exc
    if returncode != 0:
        raise subprocess.CalledProcessError(
            returncode, command, output=out_text,
            stderr=stderr.decode("utf-8", errors="replace"),
        )
    return subprocess.CompletedProcess(command, returncode, stdout=out_text)
