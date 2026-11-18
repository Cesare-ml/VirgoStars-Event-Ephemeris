# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Configurable timeout for the two P0 audits without weakening Cargo gates."""
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from validate_rust import GATES, main, run_gates

DIGEST = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499"


class AuditTimeoutConfigTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        (self.root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (self.root / "tools").mkdir()
        (self.root / "tools" / "audit_p0_bytes.py").write_text("# stub\n", encoding="utf-8")
        self.archive = self.root / "fixture.vseph-p0"
        self.archive.write_bytes(b"synthetic-test-bytes")

    def audit_report(self, command, **kwargs):
        report = {"sha256": DIGEST, "bytes": 20, "blocks": 4, "records": 8,
                  "sections": {"synod": 3, "motion": 2, "ingress": 3}}
        if command[0] == "mock-cargo":
            report.update(format="VSEPH-P0-EXPERIMENTAL",
                          sha256_checked_against_expected=True, coverage_certified=False)
        return subprocess.CompletedProcess(command, 0, stdout=json.dumps(report))

    def test_explicit_timeout_is_forwarded_to_both_independent_auditors(self):
        with patch("validate_rust.subprocess.run", return_value=subprocess.CompletedProcess([], 0)) as cargo, \
             patch("validate_rust.run_bounded_audit", side_effect=self.audit_report) as auditor:
            run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                      cargo="mock-cargo", audit_timeout_seconds=3600)
        self.assertEqual(cargo.call_count, len(GATES))
        self.assertEqual(auditor.call_count, 2)
        for call in auditor.call_args_list:
            self.assertEqual(call.kwargs, {"cwd": self.root, "timeout_seconds": 3600})

    def test_omitted_timeout_uses_existing_runner_default(self):
        with patch("validate_rust.subprocess.run", return_value=subprocess.CompletedProcess([], 0)), \
             patch("validate_rust.run_bounded_audit", side_effect=self.audit_report) as auditor:
            run_gates(root=self.root, archive=self.archive, expected_sha256=DIGEST,
                      cargo="mock-cargo")
        self.assertEqual(auditor.call_count, 2)
        self.assertTrue(all(call.kwargs == {"cwd": self.root} for call in auditor.call_args_list))

    def test_invalid_timeout_is_rejected_before_cargo(self):
        for value in (0, -1, float("inf"), float("nan"), True, "60"):
            with self.subTest(value=value):
                invoke = Mock()
                with self.assertRaisesRegex(ValueError, "positive and finite"):
                    run_gates(root=self.root, cargo="mock-cargo", invoke=invoke,
                              audit_timeout_seconds=value)
                invoke.assert_not_called()

    def test_main_cli_passes_timeout_to_run_gates(self):
        with patch("validate_rust.run_gates") as gates:
            result = main(["--audit-timeout-seconds", "1800"])
        self.assertEqual(result, 0)
        gates.assert_called_once_with(
            archive=None, expected_sha256=None, audit_timeout_seconds=1800.0,
        )

    def test_main_cli_rejects_nan_without_running_cargo(self):
        with patch("validate_rust.shutil.which", return_value="mock-cargo"), \
             patch("validate_rust.run_gates", wraps=run_gates) as gates, \
             patch("validate_rust.subprocess.run") as cargo:
            code = main(["--audit-timeout-seconds", "nan"])
        self.assertEqual(code, 1)
        gates.assert_called_once()
        cargo.assert_not_called()


if __name__ == "__main__":
    unittest.main()
