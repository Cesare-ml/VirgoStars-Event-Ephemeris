# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Unit tests for the opt-in Cargo.lock preparation helper; no real Cargo needed."""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import Mock

from prepare_cargo_lock import missing_packages, prepare


class CargoLockPreparationTests(unittest.TestCase):
    def workspace(self, base: Path, lock: str = "") -> Path:
        (base / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        binding = base / "crates" / "python-bindings"
        binding.mkdir(parents=True)
        (binding / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
        (base / "Cargo.lock").write_text(lock, encoding="utf-8")
        return base

    @staticmethod
    def packages(*names: str) -> str:
        return "".join(f'[[package]]\nname = "{name}"\n' for name in names)

    def test_lock_parser_ignores_incidental_dep_names(self):
        with tempfile.TemporaryDirectory() as folder:
            root = self.workspace(Path(folder), '[dependencies]\npyo3 = "yes"\n')
            self.assertEqual(missing_packages(root / "Cargo.lock"),
                             {"pyo3", "tempfile", "virgostars-events-python"})
            (root / "Cargo.lock").write_text(self.packages("pyo3", "tempfile"), encoding="utf-8")
            self.assertEqual(missing_packages(root / "Cargo.lock"), {"virgostars-events-python"})

    def test_read_only_stale_does_not_invoke_cargo_or_git(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stdout(StringIO()) as output:
            root = self.workspace(Path(folder), self.packages("pyo3"))
            run = Mock(side_effect=AssertionError("must not run commands"))
            self.assertEqual(prepare(root, run=run), 1)
            run.assert_not_called()
            self.assertIn("tempfile", output.getvalue())

    def test_read_only_complete_is_not_native_acceptance(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stdout(StringIO()) as output:
            root = self.workspace(Path(folder), self.packages("pyo3", "tempfile", "virgostars-events-python"))
            self.assertEqual(prepare(root), 0)
            self.assertIn("does not prove Cargo", output.getvalue())

    def test_missing_workspace_fails_closed(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            self.assertEqual(prepare(Path(folder)), 1)

    def test_apply_requires_git_and_cargo(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            self.assertEqual(prepare(root, apply=True, which=lambda name: None), 1)

    def test_dirty_tree_never_regenerates_lock(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            called = []
            def run(cmd, **kwargs):
                called.append(cmd)
                return SimpleNamespace(returncode=0, stdout=" M Cargo.lock\n", stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)
            self.assertEqual(len(called), 1)
            self.assertIn("status", called[0])

    def test_apply_writes_generated_lock_and_verifies_metadata(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stdout(StringIO()):
            root = self.workspace(Path(folder))
            calls = []
            def run(cmd, **kwargs):
                calls.append(cmd)
                if "generate-lockfile" in cmd:
                    (root / "Cargo.lock").write_text(
                        self.packages("pyo3", "tempfile", "virgostars-events-python"),
                        encoding="utf-8")
                return SimpleNamespace(returncode=0,
                    stdout="Cargo.lock\n" if "diff" in cmd else "", stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 0)
            self.assertEqual([cmd[1] for cmd in calls],
                             ["status", "generate-lockfile", "metadata", "diff"])
            self.assertIn("--locked", calls[2])

    def test_generation_without_deps_never_claims_success(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            def run(cmd, **kwargs):
                return SimpleNamespace(returncode=0, stdout="", stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)

    def test_cargo_metadata_failure_propagates(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            def run(cmd, **kwargs):
                if "generate-lockfile" in cmd:
                    (root / "Cargo.lock").write_text(
                        self.packages("pyo3", "tempfile", "virgostars-events-python"), encoding="utf-8")
                return SimpleNamespace(returncode=7 if "metadata" in cmd else 0,
                                       stdout="", stderr="Cargo metadata error")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)

    def test_unexpected_tracked_files_never_get_reverted(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            def run(cmd, **kwargs):
                if "generate-lockfile" in cmd:
                    (root / "Cargo.lock").write_text(
                        self.packages("pyo3", "tempfile", "virgostars-events-python"), encoding="utf-8")
                return SimpleNamespace(returncode=0,
                                       stdout="Cargo.lock\nREADME.md\n" if "diff" in cmd else "",
                                       stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)
            self.assertTrue((root / "Cargo.lock").is_file())


if __name__ == "__main__":
    unittest.main()
