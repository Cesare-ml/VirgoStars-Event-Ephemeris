# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Orchestration regression tests; not a substitute for real Rust gates."""
from __future__ import annotations

from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
import os
import shutil
import tempfile
import unittest
from unittest.mock import patch

import build_python_mvp as build


class BuildTests(unittest.TestCase):
    def test_missing_maturin_fails_without_build_or_release(self):
        with patch.object(build, "ROOT", Path(__file__).resolve().parents[1]), patch.object(
            build, "maturin_executable", return_value=None
        ), patch.object(build, "run") as executed:
            with redirect_stderr(StringIO()) as errors:
                self.assertEqual(build.main([]), 1)
            executed.assert_not_called()
            self.assertIn("maturin missing", errors.getvalue())

    def test_incomplete_or_malformed_owner_inputs_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp)
            archive, csv = p / "source.p0", p / "sinodi.csv"
            archive.write_bytes(b"fixture")
            csv.write_text("fixtures", encoding="utf-8")
            for args in [
                ["--archive", str(archive)],
                ["--sha256", "a" * 64],
                ["--archive", str(archive), "--sha256", "q" * 64],
                ["--synods-csv", str(csv)],
                ["--archive", str(p / "missing.p0"), "--sha256", "a" * 64],
            ]:
                with self.subTest(args=args), redirect_stderr(StringIO()), self.assertRaises(SystemExit) as x:
                    build.main(args)
                self.assertEqual(x.exception.code, 2)

    def test_only_a_successful_real_wheel_gate_can_publish_local_artifact(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text("", encoding="utf-8")
            (root / "pyproject.toml").write_text("", encoding="utf-8")
            out = root / "dist"
            called = []
            def fake_run(command):
                command = [str(a) for a in command]
                called.append(command)
                if len(called) == 3:
                    wheel_dir = Path(command[command.index("--out") + 1])
                    wheel_dir.mkdir(parents=True)
                    (wheel_dir / "virgostars-0.1.0-cp310-abi3-win_amd64.whl").write_bytes(b"fixture-wheel")
            with patch.object(build, "ROOT", root), patch.object(build, "maturin_executable", return_value=Path("maturin")), patch.object(build, "run", side_effect=fake_run):
                result = build.main(["--out-dir", str(out)])
            self.assertEqual(result, 0)
            self.assertEqual(len(called), 4)
            self.assertIn("validate_python_sources.py", called[0][1])
            self.assertIn("validate_rust.py", called[1][1])
            self.assertIn("--release", called[2])
            self.assertIn("--locked", called[2])
            self.assertIn("verify_python_wheel.py", called[3][1])
            self.assertEqual([w.name for w in out.glob("*.whl")], ["virgostars-0.1.0-cp310-abi3-win_amd64.whl"])

    def test_gate_failure_does_not_leave_distributable_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").touch()
            (root / "pyproject.toml").touch()
            out = root / "dist"
            counter = 0
            def fake_run(command):
                nonlocal counter
                counter += 1
                if counter == 3:
                    args = list(map(str, command))
                    wheels = Path(args[args.index("--out") + 1])
                    wheels.mkdir()
                    (wheels / "fixture.whl").write_bytes(b"bad")
                if counter == 4:
                    raise RuntimeError("native tests failed")
            with patch.object(build, "ROOT", root), patch.object(build, "maturin_executable", return_value=Path("maturin")), patch.object(build, "run", side_effect=fake_run):
                with redirect_stderr(StringIO()):
                    self.assertEqual(build.main(["--out-dir", str(out)]), 1)
            self.assertFalse(out.exists())

    def test_python_source_preflight_failure_prevents_cargo_and_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").touch()
            (root / "pyproject.toml").touch()
            out = root / "dist"
            commands = []

            def fail_source(command):
                commands.append([str(item) for item in command])
                raise RuntimeError("source-only Python tests failed")

            with patch.object(build, "ROOT", root), patch.object(
                build, "maturin_executable", return_value=Path("maturin")
            ), patch.object(build, "run", side_effect=fail_source), redirect_stderr(StringIO()):
                self.assertEqual(build.main(["--out-dir", str(out)]), 1)
            self.assertEqual(len(commands), 1)
            self.assertIn("validate_python_sources.py", commands[0][1])
            self.assertFalse(out.exists())

    def test_cargo_failure_after_python_gate_prevents_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").touch()
            (root / "pyproject.toml").touch()
            out = root / "dist"
            commands = []

            def fail_rust(command):
                commands.append([str(item) for item in command])
                if len(commands) == 2:
                    raise RuntimeError("Cargo lockfile stale")

            with patch.object(build, "ROOT", root), patch.object(
                build, "maturin_executable", return_value=Path("maturin")
            ), patch.object(build, "run", side_effect=fail_rust), redirect_stderr(StringIO()):
                self.assertEqual(build.main(["--out-dir", str(out)]), 1)
            self.assertEqual(len(commands), 2)
            self.assertIn("validate_python_sources.py", commands[0][1])
            self.assertIn("validate_rust.py", commands[1][1])
            self.assertFalse(out.exists())

    def test_existing_dist_file_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "a.whl"
            source.write_bytes(b"new")
            dest = root / "dist"
            dest.mkdir()
            existing = dest / source.name
            existing.write_bytes(b"old")
            with self.assertRaises(FileExistsError):
                build.save_verified_wheel(source, dest)
            self.assertEqual(existing.read_bytes(), b"old")
            source.write_bytes(b"old")
            self.assertEqual(build.save_verified_wheel(source, dest), existing)

    def test_atomic_staging_hides_incomplete_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original" * 65536)
            out = root / "dist"
            original_copy = shutil.copyfileobj

            def observe_copy(src, dst):
                self.assertEqual(list(out.glob("*.whl")), [])
                original_copy(src, dst)
                self.assertEqual(list(out.glob("*.whl")), [])

            with patch.object(shutil, "copyfileobj", side_effect=observe_copy):
                result = build.save_verified_wheel(source, out)
            self.assertEqual(result.read_bytes(), source.read_bytes())
            self.assertEqual(list(out.iterdir()), [result])

    def test_interrupted_copy_leaves_no_partial_wheel(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original")
            out = root / "dist"

            def interrupted(src, dst):
                dst.write(b"partial")
                raise OSError("interrupted during copy")

            with patch.object(shutil, "copyfileobj", side_effect=interrupted):
                with self.assertRaises(OSError):
                    build.save_verified_wheel(source, out)
            self.assertEqual(list(out.iterdir()), [])

    def test_exclusive_publish_race_keeps_existing_dist_artifact(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original")
            out = root / "dist"
            output = out / source.name

            def racing_link(staged, destination):
                self.assertEqual(Path(destination), output)
                output.write_bytes(b"concurrent-job")
                raise FileExistsError("another process created the artifact")

            with patch.object(os, "link", side_effect=racing_link):
                with self.assertRaises(FileExistsError):
                    build.save_verified_wheel(source, out)
            self.assertEqual(output.read_bytes(), b"concurrent-job")
            self.assertEqual(list(out.iterdir()), [output])

    def test_source_mutation_during_copy_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original")
            out = root / "dist"
            original_copy = shutil.copyfileobj

            def changing_source(src, dst):
                original_copy(src, dst)
                source.write_bytes(b"changed")

            with patch.object(shutil, "copyfileobj", side_effect=changing_source):
                with self.assertRaises(OSError):
                    build.save_verified_wheel(source, out)
            self.assertEqual(list(out.iterdir()), [])

    def test_unsupported_hard_link_fails_closed_without_artifact(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original")
            out = root / "dist"
            with patch.object(os, "link", side_effect=OSError("hard links unavailable")):
                with self.assertRaises(OSError):
                    build.save_verified_wheel(source, out)
            self.assertEqual(list(out.iterdir()), [])

    def test_real_subprocess_environment_strips_python_overrides(self):
        with patch.dict(build.os.environ, {"PYTHONPATH": "shadow", "PYTHONHOME": "shadow", "PYTHONUSERBASE": "shadow"}), patch.object(build.subprocess, "run") as invoked:
            build.run(["python", "-V"])
        kwargs = invoked.call_args.kwargs
        for name in ("PYTHONPATH", "PYTHONHOME", "PYTHONUSERBASE"):
            self.assertNotIn(name, kwargs["env"])
        self.assertTrue(kwargs["check"])


if __name__ == "__main__":
    unittest.main()
