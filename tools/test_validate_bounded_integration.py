# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Guard the production audit subprocess path (Cargo commands are mocked).

Unlike tests that inject run_gates(invoke=...), these exercise its default
route. They do not compile Rust or verify any real scientific archive.
"""
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from validate_rust import GATES, run_gates
from bounded_audit import run_bounded_audit

DIGEST = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499"


class BoundedAuditIntegrationTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "Cargo.toml").write_text("[workspace]\nmembers=[]\n", encoding="utf-8")
        (self.root / "tools").mkdir()
        (self.root / "tools" / "audit_p0_bytes.py").write_text(
            "# synthetic audit placeholder\n", encoding="utf-8"
        )
        self.archive = self.root / "fixture.p0"
        self.archive.write_bytes(b"synthetic-test-bytes")
        self.assertEqual(self.archive.stat().st_size, 20)
        self.base = {
            "sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
            "sections": {"synod": 3, "motion": 2, "ingress": 3},
        }

    def mock_cargo(self, command, **kwargs):
        self.assertEqual(command[0], "mock-cargo")
        self.assertTrue(kwargs.get("check"))
        self.assertFalse(kwargs.get("capture_output", False))
        return subprocess.CompletedProcess(command, 0)

    def audit_report(self, command, *, cwd):
        self.assertEqual(cwd, self.root)
        report = dict(self.base)
        if command[0] == "mock-cargo":
            self.assertEqual(command[1:3], ["run", "--locked"])
            report.update(format="VSEPH-P0-EXPERIMENTAL",
                          sha256_checked_against_expected=True,
                          coverage_certified=False)
        else:
            self.assertEqual(command[0:2], [__import__("sys").executable,
                                          str(self.root / "tools" / "audit_p0_bytes.py")])
        return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))

    def test_default_path_uses_bounded_auditor_twice_not_capture_output(self):
        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit", side_effect=self.audit_report) as bounded:
            run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                      cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        self.assertEqual(bounded.call_count, 2)
        self.assertEqual(bounded.call_args_list[0].args[0][0], __import__("sys").executable)
        self.assertEqual(bounded.call_args_list[1].args[0][0], "mock-cargo")

    def test_python_audit_output_limit_stops_before_rust_audit(self):
        def oversized(command, *, cwd):
            raise RuntimeError("P0 audit subprocess output stdout exceeds 65536 bytes")

        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit", side_effect=oversized) as bounded:
            with self.assertRaisesRegex(RuntimeError, "stdout exceeds 65536 bytes"):
                run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                          cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        bounded.assert_called_once()

    def test_rust_audit_timeout_fails_closed_after_python_report(self):
        def audit(command, *, cwd):
            if command[0] == "mock-cargo":
                raise RuntimeError("P0 audit subprocess exceeded 900 seconds")
            return self.audit_report(command, cwd=cwd)

        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit", side_effect=audit) as bounded:
            with self.assertRaisesRegex(RuntimeError, "exceeded 900 seconds"):
                run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                          cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        self.assertEqual(bounded.call_count, 2)

    def test_actual_python_auditor_stdout_overflow_stops_pipeline(self):
        # Cargo is a mock, but the Python auditor is a real spawned process.
        (self.root / "tools" / "audit_p0_bytes.py").write_text(
            "import sys\nsys.stdout.write('x' * 1000000)\n", encoding="utf-8"
        )
        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit", wraps=run_bounded_audit) as bounded:
            with self.assertRaisesRegex(RuntimeError, "stdout exceeds 65536 bytes"):
                run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                          cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        bounded.assert_called_once()  # The fake Rust CLI must never execute.

    def test_actual_silent_python_auditor_timeout_stops_pipeline(self):
        # Use a short deadline ONLY in the synthetic test; production is 900s.
        (self.root / "tools" / "audit_p0_bytes.py").write_text(
            "import time\ntime.sleep(20)\n", encoding="utf-8"
        )
        def short_deadline(command, *, cwd):
            return run_bounded_audit(command, cwd=cwd, timeout_seconds=0.2)
        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit", side_effect=short_deadline) as bounded:
            with self.assertRaisesRegex(RuntimeError, "exceeded 0.2 seconds"):
                run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                          cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        bounded.assert_called_once()

    def test_no_archive_runs_only_cargo_gates(self):
        with patch("validate_rust.subprocess.run", side_effect=self.mock_cargo) as cargo, \
             patch("validate_rust.run_bounded_audit") as bounded:
            run_gates(root=self.root, cargo="mock-cargo")
        self.assertEqual(cargo.call_count, len(GATES))
        bounded.assert_not_called()


if __name__ == "__main__":
    unittest.main()
