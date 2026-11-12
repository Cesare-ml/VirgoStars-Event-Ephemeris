# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Pure-Python fail-fast checks for the M0 native acceptance orchestrator.

These tests mock the test runner/native import. They do NOT prove that a
compiled Rust/PyO3 wheel or scientific archive can pass the native acceptance gate.
"""
from __future__ import annotations

import ast
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
import sys
import tempfile
from types import ModuleType
import unittest
from unittest.mock import patch

import validate_python_mvp as validator


def fake_native_modules() -> dict[str, ModuleType]:
    package = ModuleType("virgostars_events")
    package.__path__ = []
    native = ModuleType("virgostars_events._native")
    native.NativeReader = object
    return {"virgostars_events": package, "virgostars_events._native": native}


class NativeMvpGatePreflightTests(unittest.TestCase):
    def test_missing_owner_pair_and_csv_without_archive_fail_before_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp) / "events.p0"
            archive.write_bytes(b"fixture")
            csv = Path(temp) / "sinodi.csv"
            csv.write_text("fixture", encoding="utf-8")
            for args in (
                ["--archive", str(archive)],
                ["--sha256", "a" * 64],
                ["--synods-csv", str(csv)],
            ):
                with self.subTest(args=args), patch.object(
                    validator, "run_tests"
                ) as runner, redirect_stderr(StringIO()), self.assertRaises(SystemExit) as error:
                    validator.main(args)
                self.assertEqual(error.exception.code, 2)
                runner.assert_not_called()

    def test_malformed_or_noncanonical_sha_fails_before_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp) / "events.p0"
            archive.write_bytes(b"fixture")
            for sha in (
                "", "a" * 63, "a" * 65, "g" * 64,
                "A" * 64, "0" * 63 + " ", "0" * 63 + "\n",
            ):
                with self.subTest(sha=sha), patch.object(
                    validator, "run_tests"
                ) as runner, redirect_stderr(StringIO()), self.assertRaises(SystemExit) as error:
                    validator.main(["--archive", str(archive), "--sha256", sha])
                self.assertEqual(error.exception.code, 2)
                runner.assert_not_called()

    def test_missing_or_directory_archive_fails_before_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for path in (directory / "missing.p0", directory):
                with self.subTest(path=str(path)), patch.object(
                    validator, "run_tests"
                ) as runner, redirect_stderr(StringIO()), self.assertRaises(SystemExit) as error:
                    validator.main(["--archive", str(path), "--sha256", "a" * 64])
                self.assertEqual(error.exception.code, 2)
                runner.assert_not_called()

    def test_missing_or_directory_csv_fails_before_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            archive = directory / "events.p0"
            archive.write_bytes(b"fixture")
            for csv in (directory / "missing.csv", directory):
                with self.subTest(csv=str(csv)), patch.object(
                    validator, "run_tests"
                ) as runner, redirect_stderr(StringIO()), self.assertRaises(SystemExit) as error:
                    validator.main([
                        "--archive", str(archive), "--sha256", "a" * 64,
                        "--synods-csv", str(csv),
                    ])
                self.assertEqual(error.exception.code, 2)
                runner.assert_not_called()

    def test_no_dataset_baseline_retains_all_four_native_acceptance_suites(self):
        with patch.dict(sys.modules, fake_native_modules()), patch.object(
            validator, "run_tests", return_value=True
        ) as runner:
            self.assertEqual(validator.main([]), 0)
        self.assertEqual(
            [call.args[:2] for call in runner.call_args_list],
            [
                ("test_python_api.py", 17),
                ("test_cli.py", 8),
                ("test_verify_python_synods.py", 16),
                ("test_native_fixture.py", 9),
            ],
        )

    def test_mandatory_minima_match_the_authored_repository_suites(self):
        # Prevent parallel agents from adding coverage while leaving the
        # mandatory installed-wheel acceptance gate silently out of date.
        suites = (
            ("test_python_api.py", 17, validator.TESTS),
            ("test_cli.py", 8, validator.TESTS),
            ("test_verify_python_synods.py", 16, validator.TOOLS_TESTS),
            ("test_native_fixture.py", 9, validator.TESTS),
        )
        with patch.dict(sys.modules, fake_native_modules()), patch.object(
            validator, "run_tests", return_value=True
        ) as mocked:
            self.assertEqual(validator.main([]), 0)
        self.assertEqual(
            [call.args[:2] for call in mocked.call_args_list],
            [(pattern, count) for pattern, count, _ in suites],
        )
        for pattern, minimum, directory in suites:
            with self.subTest(suite=pattern):
                source = (directory / pattern).read_text(encoding="utf-8")
                tree = ast.parse(source, filename=str(directory / pattern))
                test_methods = [
                    method for cls in ast.walk(tree) if isinstance(cls, ast.ClassDef)
                    for method in cls.body
                    if isinstance(method, (ast.FunctionDef, ast.AsyncFunctionDef))
                    and method.name.startswith("test_")
                ]
                self.assertEqual(
                    len(test_methods), minimum,
                    f"{pattern}: update the required gate when adding/removing tests",
                )

    def test_valid_archive_path_allows_owner_smoke_after_baseline(self):
        with tempfile.TemporaryDirectory() as temp:
            archive = Path(temp) / "events.p0"
            archive.write_bytes(b"fixture")
            with patch.dict(sys.modules, fake_native_modules()), patch.dict(
                validator.os.environ, {}, clear=False
            ), patch.object(validator, "run_tests", return_value=True) as runner:
                self.assertEqual(
                    validator.main(["--archive", str(archive), "--sha256", "a" * 64]),
                    0,
                )
                self.assertEqual(validator.os.environ["VSEPH_P0_PATH"], str(archive.resolve()))
                self.assertEqual(validator.os.environ["VSEPH_P0_SHA256"], "a" * 64)
            self.assertEqual(runner.call_args_list[-1].args[:2], ("test_native_smoke.py", 3))

    def test_existing_csv_path_passes_preflight_without_executing_native_suite(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            archive, csv = directory / "events.p0", directory / "synodi.csv"
            archive.write_bytes(b"fixture")
            csv.write_text("fixture", encoding="utf-8")
            with patch.dict(sys.modules, fake_native_modules()), patch.object(
                validator, "run_tests", return_value=False
            ) as runner:
                self.assertEqual(
                    validator.main([
                        "--archive", str(archive), "--sha256", "a" * 64,
                        "--synods-csv", str(csv),
                    ]),
                    1,
                )
            runner.assert_called_once()

    def test_first_failing_suite_stops_without_false_success(self):
        with patch.dict(sys.modules, fake_native_modules()), patch.object(
            validator, "run_tests", side_effect=[True, False]
        ) as runner:
            self.assertEqual(validator.main([]), 1)
        self.assertEqual(runner.call_count, 2)


    def test_real_unittest_early_stop_is_not_a_false_success(self):
        # A real TextTestRunner can return wasSuccessful=True and no skips
        # despite executing only one of two discovered cases.
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            (directory / "test_m0_early_stop.py").write_text(
                "import unittest\n"
                "class EarlyStop(unittest.TestCase):\n"
                "    def test_01_stop(self):\n"
                "        self._outcome.result.stop()\n"
                "    def test_02_must_run(self):\n"
                "        self.assertTrue(True)\n",
                encoding="utf-8",
            )
            with patch("sys.stdout", new_callable=StringIO) as output, redirect_stderr(StringIO()) as error:
                self.assertFalse(validator.run_tests("test_m0_early_stop.py", 2, directory))
            self.assertIn("discovered=2, ran=1", error.getvalue())
            self.assertNotIn("PASS:", output.getvalue())

    def test_real_unittest_expected_failure_cannot_pass_native_gate(self):
        # unittest reports OK for @expectedFailure; acceptance cannot count
        # that as a genuinely passing Rust/Python integration test.
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            (directory / "test_m0_native_expected_failure.py").write_text(
                "import unittest\n"
                "class KnownFailure(unittest.TestCase):\n"
                "    @unittest.expectedFailure\n"
                "    def test_native_reader(self): self.assertEqual(1, 2)\n",
                encoding="utf-8",
            )
            with patch("sys.stdout", new_callable=StringIO) as output, redirect_stderr(StringIO()) as errors:
                self.assertFalse(validator.run_tests("test_m0_native_expected_failure.py", 1, directory))
            self.assertIn("expected_failures=1", errors.getvalue())
            self.assertNotIn("PASS:", output.getvalue())

    def test_real_unittest_unexpected_success_cannot_pass_native_gate(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            (directory / "test_m0_native_unexpected_success.py").write_text(
                "import unittest\n"
                "class Unexpected(unittest.TestCase):\n"
                "    @unittest.expectedFailure\n"
                "    def test_still_decorated(self): self.assertEqual(1, 1)\n",
                encoding="utf-8",
            )
            with patch("sys.stdout", new_callable=StringIO) as output, redirect_stderr(StringIO()) as errors:
                self.assertFalse(validator.run_tests("test_m0_native_unexpected_success.py", 1, directory))
            self.assertIn("unexpected_successes=1", errors.getvalue())
            self.assertNotIn("PASS:", output.getvalue())

    def test_real_unittest_complete_suite_is_accepted(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            (directory / "test_m0_complete_suite.py").write_text(
                "import unittest\n"
                "class Complete(unittest.TestCase):\n"
                "    def test_01(self): self.assertTrue(True)\n"
                "    def test_02(self): self.assertEqual(1 + 1, 2)\n",
                encoding="utf-8",
            )
            with patch("sys.stdout", new_callable=StringIO) as output, redirect_stderr(StringIO()):
                self.assertTrue(validator.run_tests("test_m0_complete_suite.py", 2, directory))
            self.assertIn("PASS: test_m0_complete_suite.py: 2/2", output.getvalue())


if __name__ == "__main__":
    unittest.main()
