# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Pure-Python regressions for the mandatory pre-Cargo wheel-policy gate."""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import validate_wheel_preflight as gate


class WheelPreflightGateTests(unittest.TestCase):
    @staticmethod
    def suite(case: type[unittest.TestCase]) -> unittest.TestSuite:
        return unittest.defaultTestLoader.loadTestsFromTestCase(case)

    def check(self, case: type[unittest.TestCase], *, minimum: int) -> bool:
        with patch.object(gate.unittest.defaultTestLoader, "discover",
                          return_value=self.suite(case)):
            with redirect_stdout(StringIO()), redirect_stderr(StringIO()):
                return gate.run_suite("synthetic.py", minimum)

    def test_four_required_wheel_suites_have_nonzero_baselines(self):
        self.assertEqual(
            gate.REQUIRED_SUITES,
            (("test_wheel_contents_policy.py", 16),
             ("test_wheel_identity.py", 9),
             ("test_verify_python_wheel.py", 12),
             ("test_build_python_mvp.py", 17)),
        )

    def test_missing_suite_source_fails_before_discovery(self):
        with tempfile.TemporaryDirectory() as folder:
            with patch.object(gate, "TOOLS", Path(folder)), patch.object(
                gate, "run_suite"
            ) as runner, redirect_stderr(StringIO()):
                self.assertEqual(gate.main(), 1)
            runner.assert_not_called()

    def test_complete_actual_unittest_suite_passes(self):
        class RealPass(unittest.TestCase):
            def test_first(self):
                self.assertTrue(True)

            def test_second(self):
                self.assertEqual(3, 1 + 2)

        self.assertTrue(self.check(RealPass, minimum=2))

    def test_removed_test_fails_before_execution(self):
        class RealPass(unittest.TestCase):
            def test_only(self):
                self.assertTrue(True)

        with patch.object(gate.unittest.defaultTestLoader, "discover",
                          return_value=self.suite(RealPass)), patch.object(
            gate.unittest, "TextTestRunner"
        ) as runner, redirect_stderr(StringIO()), redirect_stdout(StringIO()):
            self.assertFalse(gate.run_suite("synthetic.py", 2))
        runner.assert_not_called()

    def test_real_skips_and_expected_failures_are_never_green(self):
        class Skipped(unittest.TestCase):
            @unittest.skip("not acceptable for release")
            def test_skipped(self):
                pass

        class ExpectedFailure(unittest.TestCase):
            @unittest.expectedFailure
            def test_expected_failure(self):
                self.fail("not actually passing")

        self.assertFalse(self.check(Skipped, minimum=1))
        self.assertFalse(self.check(ExpectedFailure, minimum=1))

    def test_real_early_stopped_suite_is_not_reported_as_pass(self):
        class Aborted(unittest.TestCase):
            def test_first(self):
                self._outcome.result.stop()

            def test_second(self):
                self.assertTrue(True)

        self.assertFalse(self.check(Aborted, minimum=2))

    def test_fourth_orchestration_suite_failure_fails_closed(self):
        # The builder's 17 independent orchestration tests are mandatory,
        # not merely discoverable by a developer running tests manually.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, _ in gate.REQUIRED_SUITES:
                (root / name).touch()
            with patch.object(gate, "TOOLS", root), patch.object(
                gate, "run_suite", side_effect=[True, True, True, False]
            ) as runner, redirect_stdout(StringIO()):
                self.assertEqual(gate.main(), 1)
            self.assertEqual(runner.call_count, 4)
            self.assertEqual(runner.call_args.args, ("test_build_python_mvp.py", 17))

    def test_main_checks_all_four_and_stops_on_first_failure(self):
        # Use dummy source paths because run_suite is deliberately mocked;
        # this orchestration check must not depend on a populated checkout.
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, _ in gate.REQUIRED_SUITES:
                (root / name).touch()
            with patch.object(gate, "TOOLS", root):
                with patch.object(gate, "run_suite", side_effect=[True, True, True, True]) as runner:
                    with redirect_stdout(StringIO()):
                        self.assertEqual(gate.main(), 0)
                self.assertEqual(
                    [call.args[0] for call in runner.call_args_list],
                    [name for name, _ in gate.REQUIRED_SUITES],
                )

                with patch.object(gate, "run_suite", side_effect=[True, False]) as runner:
                    with redirect_stdout(StringIO()):
                        self.assertEqual(gate.main(), 1)
                self.assertEqual(runner.call_count, 2)


if __name__ == "__main__":
    unittest.main()
