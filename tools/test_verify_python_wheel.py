# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""No-Rust regression tests for the isolated offline wheel validation gate."""
from __future__ import annotations

from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import verify_python_wheel as wheel_check


class WheelGateTests(unittest.TestCase):
    def make_wheel(self, directory: Path, *, project_name: str = "virgostars-event-ephemeris") -> Path:
        result = directory / "virgostars_event_ephemeris-0.1.0-py3-none-any.whl"
        with zipfile.ZipFile(result, "w") as package:
            metadata = "virgostars_event_ephemeris-0.1.0.dist-info"
            package.writestr(f"{metadata}/WHEEL", "Wheel-Version: 1.0\n")
            package.writestr(f"{metadata}/METADATA", f"Name: {project_name}\nVersion: 0.1.0\n")
            package.writestr("virgostars_events/__init__.py", "__version__ = '0.1.0'\n")
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
            self.assertEqual(Path(commands[0][-1]).name, wheel.name)
            self.assertNotEqual(Path(commands[0][-1]), wheel.resolve())
            self.assertIn("from virgostars_events._native import NativeReader", commands[1][-1])
            self.assertIn("--version", commands[2])
            self.assertTrue(str(commands[3][0]).endswith("virgostars-events.exe" if wheel_check.os.name == "nt" else "virgostars-events"))
            self.assertEqual(commands[3][1], "--version")
            self.assertTrue(str(commands[4][2]).endswith("tools/validate_python_mvp.py"))
            self.assertTrue(all("-I" in c for c in (commands[1], commands[2], commands[4])))

    def test_source_wheel_swap_after_preflight_cannot_change_installed_bytes(self):
        # Simulate a replacement of the original wheel path between the
        # pre-install ZIP checks and pip's install subprocess.
        with tempfile.TemporaryDirectory() as folder:
            wheel = self.make_wheel(Path(folder))
            initial = wheel.read_bytes()
            copied = []

            def swap_original_during_venv_creation(_directory):
                wheel.write_bytes(b"tampered-after-preflight")

            def observe_install(command, *, cwd):
                args = list(map(str, command))
                if "pip" in args and "install" in args:
                    staged = Path(args[-1])
                    self.assertTrue(staged.is_file())
                    self.assertNotEqual(staged, wheel.resolve())
                    self.assertEqual(staged.read_bytes(), initial)
                    self.assertEqual(wheel.read_bytes(), b"tampered-after-preflight")
                    copied.append(staged)

            with patch.object(wheel_check.venv, "EnvBuilder") as builder, patch.object(
                wheel_check, "run_checked", side_effect=observe_install
            ):
                builder.return_value.create.side_effect = swap_original_during_venv_creation
                self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 0)
            self.assertEqual(len(copied), 1)
            self.assertFalse(copied[0].exists())  # private temp dir cleaned up

    def test_native_probe_requires_compiled_extension_within_venv(self):
        probe = wheel_check.INSTALLED_PROBE
        self.assertIn("EXTENSION_SUFFIXES", probe)
        self.assertIn("extension.is_relative_to(root)", probe)
        self.assertIn("_native is not a compiled extension", probe)
        self.assertIn("version('virgostars-event-ephemeris')", probe)

    def test_subprocesses_do_not_inherit_python_import_overrides(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.dict(wheel_check.os.environ, {
                "PYTHONPATH": "unsafe-source-checkout",
                "PYTHONHOME": "unsafe-python-home",
                "PYTHONUSERBASE": "unsafe-user-site",
            }), patch.object(
                wheel_check.subprocess, "run", return_value=SimpleNamespace(returncode=0)
            ) as invoked:
                wheel_check.run_checked(["fake-command", "--version"], cwd=Path(temp))
            sanitized = invoked.call_args.kwargs["env"]
            for name in ("PYTHONPATH", "PYTHONHOME", "PYTHONUSERBASE"):
                self.assertNotIn(name, sanitized)
            self.assertEqual(invoked.call_args.kwargs["cwd"], temp)

    def test_real_subprocess_strips_inherited_pip_target_and_config(self):
        # Unlike a mocked subprocess, this actually launches CPython with
        # malicious PIP_* overrides to ensure the temporary wheel verifier
        # never installs into a caller-specified global/external directory.
        import json
        import os
        import sys

        script = (
            "import json,os,pathlib;"
            "pathlib.Path('child-env.json').write_text("
            "json.dumps({k:v for k,v in os.environ.items() "
            "if k.startswith('PIP_') or k == 'PYTHONPATH'}))"
        )
        with tempfile.TemporaryDirectory() as temp:
            overrides = {
                "PIP_TARGET": str(Path(temp) / "outside"),
                "PIP_PREFIX": str(Path(temp) / "alternate"),
                "PIP_USER": "true",
                "PIP_INDEX_URL": "https://untrusted.example.invalid/simple",
                "PIP_CONFIG_FILE": str(Path(temp) / "untrusted-pip.ini"),
                "PYTHONPATH": "shadow-installed-package",
            }
            with patch.dict(wheel_check.os.environ, overrides):
                wheel_check.run_checked([sys.executable, "-c", script], cwd=Path(temp))
            found = json.loads((Path(temp) / "child-env.json").read_text(encoding="utf-8"))
            self.assertEqual(found, {"PIP_CONFIG_FILE": os.devnull})

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

    def test_malformed_prebuilt_wheel_is_rejected_before_creating_venv(self):
        with tempfile.TemporaryDirectory() as temp:
            wheel = self.make_wheel(Path(temp))
            wheel.write_bytes(b"bad ZIP")
            with patch.object(wheel_check.venv, "EnvBuilder") as builder:
                with redirect_stderr(StringIO()) as errors:
                    self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 1)
                builder.assert_not_called()
            self.assertIn("pre-install wheel content check", errors.getvalue())

    def test_unrelated_wheel_metadata_rejected_before_creating_venv(self):
        with tempfile.TemporaryDirectory() as temp:
            wheel = self.make_wheel(Path(temp), project_name="unrelated-package")
            with patch.object(wheel_check.venv, "EnvBuilder") as builder:
                with redirect_stderr(StringIO()) as errors:
                    self.assertEqual(wheel_check.main(["--wheel", str(wheel)]), 1)
                builder.assert_not_called()
            self.assertIn("METADATA Name", errors.getvalue())

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
