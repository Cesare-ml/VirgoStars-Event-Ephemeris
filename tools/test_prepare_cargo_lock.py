# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Unit tests for the opt-in Cargo.lock preparation helper; no real Cargo needed."""
from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
import os
from types import SimpleNamespace
import tempfile
import subprocess
import shutil
import unittest
from unittest.mock import Mock, patch

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

    def test_symlink_lock_is_refused_before_tools_or_git_commands(self):
        # Cross-platform check: simulate a tracked symlink, even on Windows
        # machines where creating symlinks requires additional privileges.
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()) as errors:
            root = self.workspace(Path(folder))
            run = Mock(side_effect=AssertionError("no Git or Cargo commands may run"))
            original = Path.is_symlink
            with patch.object(Path, "is_symlink", autospec=True,
                              side_effect=lambda path: path.name == "Cargo.lock"):
                self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)
            run.assert_not_called()
            self.assertIn("not a symlink", errors.getvalue())

    @unittest.skipUnless(shutil.which("git") and hasattr(os, "symlink"), "requires Git and symlinks")
    def test_real_git_symlink_lock_cannot_redirect_external_file(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            outside = base / "external-Cargo.lock"
            outside.write_bytes(b"unchanged-external-lock")
            root = base / "repo"
            root.mkdir()
            self.workspace(root)
            (root / "Cargo.lock").unlink()
            try:
                (root / "Cargo.lock").symlink_to(outside)
            except OSError as error:
                self.skipTest(f"symlinks unavailable: {error}")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "Cargo.toml", "Cargo.lock", "crates/python-bindings/Cargo.toml"],
                           cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-q", "-m", "tracked symlink"], cwd=root, check=True)
            calls = []
            def run(cmd, **kwargs):
                calls.append(cmd)
                if cmd[0] == "cargo-stub":
                    raise AssertionError("Cargo must not run on symlinked Cargo.lock")
                return subprocess.run(cmd, **kwargs)
            with redirect_stderr(StringIO()) as errors:
                self.assertEqual(prepare(root, apply=True, which=lambda n: "cargo-stub" if n == "cargo" else "git",
                                         run=run), 1)
            self.assertEqual(calls, [])
            self.assertEqual(outside.read_bytes(), b"unchanged-external-lock")
            self.assertIn("not a symlink", errors.getvalue())

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
                    stdout=(str(root) + "\n" if "rev-parse" in cmd else "Cargo.lock\n" if "diff" in cmd else ""), stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 0)
            self.assertEqual([cmd[1] for cmd in calls],
                             ["status", "rev-parse", "ls-files", "generate-lockfile", "metadata", "diff"])
            self.assertIn("--locked", calls[4])

    @unittest.skipUnless(shutil.which("git"), "requires Git for real tracking test")
    def test_real_git_checkout_requires_tracked_cargo_lock(self):
        # Execute Git itself, not a mock: Cargo is simulated by a test-run
        # callback so the test stays independent of Rust and of crates.io.
        with tempfile.TemporaryDirectory() as folder:
            root = self.workspace(Path(folder))
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "Cargo.toml", "crates/python-bindings/Cargo.toml"],
                           cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-q", "-m", "initial"], cwd=root, check=True)
            calls = []
            def run(cmd, **kwargs):
                calls.append(cmd)
                if cmd[0] == "cargo-stub":
                    raise AssertionError("untracked Cargo.lock must prevent Cargo execution")
                return subprocess.run(cmd, **kwargs)
            with redirect_stderr(StringIO()):
                self.assertEqual(prepare(root, apply=True, which=lambda n: "cargo-stub" if n == "cargo" else "git", run=run), 1)
            self.assertEqual([c[1] for c in calls], ["status", "rev-parse", "ls-files"])

    @unittest.skipUnless(shutil.which("git"), "requires Git for real tracking test")
    def test_real_git_tracked_lock_only_changes_unstaged_lock(self):
        # Exercise actual Git cleanliness and diff detection on a temporary
        # committed repository. Only Cargo responses are simulated.
        with tempfile.TemporaryDirectory() as folder, redirect_stdout(StringIO()):
            root = self.workspace(Path(folder), "# original\n")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "Cargo.toml", "Cargo.lock", "crates/python-bindings/Cargo.toml"],
                           cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.org",
                            "commit", "-q", "-m", "initial"], cwd=root, check=True)
            def run(cmd, **kwargs):
                if cmd[0] == "cargo-stub":
                    if cmd[1] == "generate-lockfile":
                        (root / "Cargo.lock").write_text(
                            self.packages("pyo3", "tempfile", "virgostars-events-python"), encoding="utf-8")
                    return SimpleNamespace(returncode=0, stdout="{}", stderr="")
                return subprocess.run(cmd, **kwargs)
            self.assertEqual(prepare(root, apply=True, which=lambda n: "cargo-stub" if n == "cargo" else "git", run=run), 0)
            changes = subprocess.run(["git", "status", "--porcelain=v1"], cwd=root,
                                     capture_output=True, text=True, check=True).stdout
            self.assertEqual(changes.strip(), "M Cargo.lock")
            staged = subprocess.run(["git", "diff", "--cached", "--name-only"], cwd=root,
                                    capture_output=True, text=True, check=True).stdout
            self.assertEqual(staged, "")

    @unittest.skipUnless(shutil.which("git"), "requires Git for real root test")
    def test_nested_cargo_workspace_must_not_use_parent_repository(self):
        # A nested source directory is not its own Git checkout. Never let
        # cargo generate-lockfile run under the accidental parent Git tree.
        with tempfile.TemporaryDirectory() as folder:
            parent = Path(folder)
            subprocess.run(["git", "init", "-q"], cwd=parent, check=True)
            root = parent / "nested"
            root.mkdir()
            root = self.workspace(root)
            cargo_calls = []
            def run(cmd, **kwargs):
                if cmd[0] == "cargo-stub":
                    cargo_calls.append(cmd)
                    raise AssertionError("Cargo must never run in a nested non-root checkout")
                return subprocess.run(cmd, **kwargs)
            with redirect_stderr(StringIO()) as error:
                self.assertEqual(prepare(
                    root, apply=True,
                    which=lambda name: "cargo-stub" if name == "cargo" else "git",
                    run=run,
                ), 1)
            self.assertEqual(cargo_calls, [])
            self.assertIn("not the Git repository root", error.getvalue())

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
                                       stdout=str(root) if "rev-parse" in cmd else "", stderr="Cargo metadata error")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)

    def test_unexpected_tracked_files_never_get_reverted(self):
        with tempfile.TemporaryDirectory() as folder, redirect_stderr(StringIO()):
            root = self.workspace(Path(folder))
            def run(cmd, **kwargs):
                if "generate-lockfile" in cmd:
                    (root / "Cargo.lock").write_text(
                        self.packages("pyo3", "tempfile", "virgostars-events-python"), encoding="utf-8")
                return SimpleNamespace(returncode=0,
                                       stdout=(str(root) if "rev-parse" in cmd else "Cargo.lock\nREADME.md\n" if "diff" in cmd else ""),
                                       stderr="")
            self.assertEqual(prepare(root, apply=True, which=lambda name: name, run=run), 1)
            self.assertTrue((root / "Cargo.lock").is_file())


if __name__ == "__main__":
    unittest.main()
