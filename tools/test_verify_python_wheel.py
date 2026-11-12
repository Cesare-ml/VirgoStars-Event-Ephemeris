# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""No-Rust regression tests for the isolated offline wheel validation gate."""
from __future__ import annotations

from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_python_wheel as wheel_check


class WheelGateTests(unittest.TestCase):
    def make_wheel(self, directory: Path) -> Path:
        result = directory / "virgostars_event_ephemeris-0.1.0-py3-none-any.whl"
        result.write_bytes(b"placeholder; never actually installed during unit tests")
        return result

    def assert_rejected(self, args: list[str]) -> None:
        with redirect_stderr(StringIO()), self.assertRaises(SystemExit) as caught:
            wheel_check.main(args)
        self.assertEqual(caught.exception.code, 2)

    def test_requires_actual_wheel_not_missing_or_arbitrary_path(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.assert_rejected(["--wheel", str(root / "missing.whl")])
            wrong = root / "archive.zip"
            wrong.write_bytes(b"not a wheel")
            self.assert_rejected(["--wheel", str(wrong)])

    def test_trusted_sha_and_data_arguments_are_fail_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            wheel = self.make_wheel(root)
            archive = root / "archive.p0"
            archive.write_bytes(b"synthetic")
            csv = root / "synods.csv"
            csv.write_text("synthetic\n", encoding="utf-8")
            self.assert_rejected(["--wheel", str(wheel), "--archive", str(archive)])
            self.assert_rejected(["--wheel", str(wheel), "--sha256", "a" * 64])
            self.assert_rejected(["--wheel", str(wheel), "--archive", str(archive), "--sha256", "g" * 64])
            self.assert_rejected(["--wheel", str(wheel), "--synods-csv", str(csv)])
            self.assert_rejected(["--wheel", str(wheel), "--archive", str(root / "missing.p0"), "--sha256", "b" * 64])

    def test_offline_install_and_real_native_gate_are_both_required(self):
        with tempfile.TemporaryDirectory() as temp:
            wheel = self.make_wheel(Path(temp))
            with patch.object(wheel_check.venv, "EnvBuilder") as builder, patch.object(
                wheel_check, "run_checked"
            ) as runner:
                self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 0)
            builder.assert_called_once_with(with_pip=True, clear=True)
            commands = [list(call.args[0]) for call in runner.call_args_list]
            self.assertEqual(len(commands), 5)
            self.assertIn("--no-index", commands[0])
            self.assertIn("--no-deps", commands[0])
            self.assertEqual(str(commands[0][-1]), str(wheel.resolve()))
            self.assertIn("from virgostars_events._native import NativeReader", commands[1][-1])
            self.assertIn("--version", commands[2])
            self.assertTrue(str(commands[3][0]).endswith("virgostars-events.exe" if wheel_check.os.name == "nt" else "virgostars-events"))
            self.assertEqual(commands[3][1], "--version")
            self.assertTrue(str(commands[4][2]).endswith("tools/validate_python_mvp.py"))
            self.assertTrue(all("-I" in c for c in (commands[1], commands[2], commands[4])))

    def test_native_probe_requires_compiled_extension_within_venv(self):
        probe = wheel_check.INSTALLED_PROBE
        self.assertIn("EXTENSION_SUFFIXES", probe)
        self.assertIn("extension.is_relative_to(root)", probe)
        self.assertIn("_native is not a compiled extension", probe)
        self.assertIn("version('virgostars-event-ephemeris')", probe)

    def test_owner_snapshot_and_csv_paths_are_forwarded_to_validator(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            wheel = self.make_wheel(root)
            archive = root / "authorized archive.p0"
            csv = root / "sinodi.csv"
            archive.write_bytes(b"synthetic")
            csv.write_text("synthetic\n", encoding="utf-8")
            with patch.object(wheel_check.venv, "EnvBuilder"), patch.object(
                wheel_check, "run_checked"
            ) as runner:
                self.assertEqual(wheel_check.main([
                    "--wheel", str(wheel), "--archive", str(archive),
                    "--sha256", "A" * 64, "--synods-csv", str(csv)
                ]), 0)
            last = list(runner.call_args_list[-1].args[0])
            self.assertEqual(last[last.index("--sha256") + 1], "a" * 64)
            self.assertEqual(str(last[last.index("--archive") + 1]), str(archive.resolve()))
            self.assertEqual(str(last[last.index("--synods-csv") + 1]), str(csv.resolve()))

    def test_any_failed_native_validation_makes_the_gate_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            wheel = self.make_wheel(Path(temp))
            with patch.object(wheel_check.venv, "EnvBuilder"), patch.object(
                wheel_check, "run_checked", side_effect=RuntimeError("native failure")
            ):
                with redirect_stderr(StringIO()) as stderr:
                    self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 1)
                self.assertIn("native failure", stderr.getvalue())


    def test_venv_bootstrap_failure_returns_nonzero_not_success(self):
        with tempfile.TemporaryDirectory() as temp:
            wheel = self.make_wheel(Path(temp))
            with patch.object(wheel_check.venv, "EnvBuilder") as builder:
                builder.return_value.create.side_effect = OSError("ensurepip missing")
                with redirect_stderr(StringIO()) as stderr:
                    self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 1)
                self.assertIn("ensurepip missing", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
