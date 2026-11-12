# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Runner regressions (mocks are NOT evidence of compiled native acceptance)."""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import validate_python_sources as gate


def fake_suite(n: int):
    return SimpleNamespace(countTestCases=lambda: n)


def fake_result(*, passed=True, skipped=None, ran=15):
    return SimpleNamespace(
        testsRun=ran, failures=[] if passed else [(None, "failed")],
        errors=[], skipped=[] if skipped is None else skipped,
        wasSuccessful=lambda: passed,
    )


class PythonSourceGateTests(unittest.TestCase):
    def test_source_gate_excludes_native_and_owner_data(self):
        self.assertEqual(gate.SOURCE_SUITES, (
            ("test_python_api.py", 16, "python/tests"),
            ("test_cli.py", 8, "python/tests"),
            ("test_verify_python_synods.py", 16, "tools"),
        ))
        self.assertEqual(sum(item[1] for item in gate.SOURCE_SUITES), 40)
        self.assertNotIn("test_native_fixture.py", str(gate.SOURCE_SUITES))

    def test_missing_suite_case_fails_before_running_tests(self):
        with patch.object(gate.unittest.defaultTestLoader, "discover", return_value=fake_suite(14)), patch.object(
            gate.unittest, "TextTestRunner"
        ) as runner, redirect_stderr(StringIO()):
            self.assertFalse(gate.run_suite("test_python_api.py", 15, Path("fixture")))
        runner.assert_not_called()

    def test_exact_suite_without_skips_passes(self):
        fake = fake_result(ran=15)
        with patch.object(gate.unittest.defaultTestLoader, "discover", return_value=fake_suite(15)), patch.object(
            gate.unittest, "TextTestRunner"
        ) as runner:
            runner.return_value.run.return_value = fake
            self.assertTrue(gate.run_suite("test_python_api.py", 15, Path("fixture")))
        runner.return_value.run.assert_called_once()

    def test_silent_skip_is_rejected(self):
        fake = fake_result(skipped=[(None, "native dependency missing")], ran=15)
        with patch.object(gate.unittest.defaultTestLoader, "discover", return_value=fake_suite(15)), patch.object(
            gate.unittest, "TextTestRunner"
        ) as runner, redirect_stderr(StringIO()):
            runner.return_value.run.return_value = fake
            self.assertFalse(gate.run_suite("test_python_api.py", 15, Path("fixture")))

    def test_failure_or_incomplete_execution_is_rejected(self):
        for result in (fake_result(passed=False), fake_result(ran=14)):
            with self.subTest(result=result), patch.object(
                gate.unittest.defaultTestLoader, "discover", return_value=fake_suite(15)
            ), patch.object(gate.unittest, "TextTestRunner") as runner, redirect_stderr(StringIO()):
                runner.return_value.run.return_value = result
                self.assertFalse(gate.run_suite("test_python_api.py", 15, Path("fixture")))

    def test_missing_source_checkout_returns_error(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(
            gate, "ROOT", Path(temp)
        ), redirect_stderr(StringIO()):
            self.assertEqual(gate.main(), 1)

    def test_local_source_import_and_all_suites_are_required(self):
        with patch.object(gate, "run_suite", return_value=True) as runner, redirect_stdout(StringIO()) as output:
            self.assertEqual(gate.main(), 0)
        self.assertEqual(runner.call_count, 3)
        self.assertIn("40/40 source-only tests", output.getvalue())
        self.assertIn("NOT validated", output.getvalue())

    def test_first_failing_suite_stops_the_source_gate(self):
        with patch.object(gate, "run_suite", side_effect=[True, False]) as runner, redirect_stdout(StringIO()):
            self.assertEqual(gate.main(), 1)
        self.assertEqual(runner.call_count, 2)


if __name__ == "__main__":
    unittest.main()
