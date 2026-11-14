# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Run actual local P0 source tests and check no optional dataset skip can go green."""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import validate_p0_synthetic as gate


class SyntheticP0GateTest(unittest.TestCase):
    def test_executes_all_14_real_synthetic_audits_without_owner_data(self):
        with redirect_stdout(StringIO()) as output, redirect_stderr(StringIO()):
            self.assertEqual(gate.main(), 0)
        self.assertIn("14/14 independent synthetic P0 audit tests", output.getvalue())

    def test_missing_fixture_fails_closed(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(
            gate, "ROOT", Path(folder)
        ), redirect_stderr(StringIO()) as error:
            self.assertEqual(gate.main(), 1)
        self.assertIn("missing synthetic P0 fixture", error.getvalue())

    def test_unexpected_test_count_stops_before_any_run(self):
        from test_audit_p0_bytes import AuditP0BytesTest
        with patch.object(gate, "REQUIRED_SYNTHETIC", 13), patch.object(
            gate.unittest, "TextTestRunner"
        ) as runner, redirect_stderr(StringIO()) as error:
            self.assertFalse(gate.run_synthetic(AuditP0BytesTest))
        self.assertIn("14 != 13", error.getvalue())
        runner.assert_not_called()

    def test_skips_expected_failures_and_interruption_are_rejected(self):
        from test_audit_p0_bytes import AuditP0BytesTest
        for kwargs in (
            {"skipped": [(None, "regression silently skipped")]},
            {"expectedFailures": [(None, "test marked expected failure")]},
            {"testsRun": 13},
        ):
            with self.subTest(kwargs=kwargs), patch.object(
                gate.unittest, "TextTestRunner"
            ) as runner, redirect_stderr(StringIO()):
                result = dict(testsRun=14, skipped=[], expectedFailures=[],
                              unexpectedSuccesses=[], errors=[], failures=[])
                result.update(kwargs)
                runner.return_value.run.return_value = SimpleNamespace(
                    **result, wasSuccessful=lambda: True
                )
                self.assertFalse(gate.run_synthetic(AuditP0BytesTest))


if __name__ == "__main__":
    unittest.main()
