# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Orchestration regression tests; not a substitute for real Rust gates."""
from __future__ import annotations

from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
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
                if len(called) == 2:
                    wheel_dir = Path(command[command.index("--out") + 1])
                    wheel_dir.mkdir(parents=True)
                    (wheel_dir / "virgostars-0.1.0-cp310-abi3-win_amd64.whl").write_bytes(b"fixture-wheel")
            with patch.object(build, "ROOT", root), patch.object(build, "maturin_executable", return_value=Path("maturin")), patch.object(build, "run", side_effect=fake_run):
                result = build.main(["--out-dir", str(out)])
            self.assertEqual(result, 0)
            self.assertEqual(len(called), 3)
            self.assertIn("validate_rust.py", called[0][1])
            self.assertIn("--release", called[1])
            self.assertIn("verify_python_wheel.py", called[2][1])
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
                if counter == 2:
                    args = list(map(str, command))
                    wheels = Path(args[args.index("--out") + 1])
                    wheels.mkdir()
                    (wheels / "fixture.whl").write_bytes(b"bad")
                if counter == 3:
                    raise RuntimeError("native tests failed")
            with patch.object(build, "ROOT", root), patch.object(build, "maturin_executable", return_value=Path("maturin")), patch.object(build, "run", side_effect=fake_run):
                with redirect_stderr(StringIO()):
                    self.assertEqual(build.main(["--out-dir", str(out)]), 1)
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

    def test_exclusive_copy_race_keeps_existing_dist_artifact(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "artifact.whl"
            source.write_bytes(b"original")
            dest = root / "dist"
            dest.mkdir()
            output = dest / source.name
            actual_open = Path.open
            def racing_open(path, mode="r", *args, **kwargs):
                if str(path) == str(output) and mode == "xb":
                    output.write_bytes(b"concurrent-job")
                    raise FileExistsError("another process created the artifact")
                return actual_open(path, mode, *args, **kwargs)
            with patch.object(Path, "open", autospec=True, side_effect=racing_open):
                with self.assertRaises(FileExistsError):
                    build.save_verified_wheel(source, dest)
            self.assertEqual(output.read_bytes(), b"concurrent-job")

    def test_real_subprocess_environment_strips_python_overrides(self):
        with patch.dict(build.os.environ, {"PYTHONPATH": "shadow", "PYTHONHOME": "shadow", "PYTHONUSERBASE": "shadow"}), patch.object(build.subprocess, "run") as invoked:
            build.run(["python", "-V"])
        kwargs = invoked.call_args.kwargs
        for name in ("PYTHONPATH", "PYTHONHOME", "PYTHONUSERBASE"):
            self.assertNotIn(name, kwargs["env"])
        self.assertTrue(kwargs["check"])


if __name__ == "__main__":
    unittest.main()
