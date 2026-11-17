# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
from __future__ import annotations
import tempfile
from pathlib import Path
import unittest
import check_m0_build_env as check

LOCK = ''.join(f'[[package]]\nname = "{p}"\nversion = "0.1.0"\n\n' for p in ("pyo3", "tempfile", "virgostars-events-python"))

class TestReadiness(unittest.TestCase):
    def create(self, root: Path, lock: str | None = LOCK) -> None:
        (root / "Cargo.toml").write_text('[workspace]\n', encoding='utf8')
        (root / "pyproject.toml").write_text('[build-system]\n', encoding='utf8')
        (root / "crates/python-bindings").mkdir(parents=True)
        (root / "crates/python-bindings/Cargo.toml").write_text('[package]\n', encoding='utf8')
        if lock is not None:
            (root / 'Cargo.lock').write_text(lock,encoding='utf8')

    def test_prerequisites_pass_without_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); self.create(root)
            original=(root/'Cargo.lock').read_bytes()
            got=check.problems(root, which=lambda x:'/fake/'+x, scripts_dir=root/'missing',python_version=(3,13),python_impl='cpython')
            self.assertEqual(got,[])
            self.assertEqual((root/'Cargo.lock').read_bytes(),original)

    def test_missing_toolchain_and_stale_lock_give_actionable_failures(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); self.create(root, '[[package]]\nname = "pyo3"\n')
            got=check.problems(root,which=lambda x:None,scripts_dir=root/'missing',python_version=(3,13),python_impl='cpython')
            text='\n'.join(got)
            for word in ('tempfile','virgostars-events-python','prepare_cargo_lock.py --apply','rustc','cargo not found','maturin'):
                self.assertIn(word,text)
            self.assertNotIn('pyo3,',text)

    def test_missing_manifest_and_missing_lock_are_blockers(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            got=check.problems(root,which=lambda x:'/fake/'+x,scripts_dir=root/'missing',python_version=(3,13),python_impl='cpython')
            text='\n'.join(got)
            self.assertIn('Cargo.toml',text)
            self.assertIn('pyproject.toml',text)
            self.assertIn('Cargo.lock missing',text)

    def test_unsupported_python_and_symlinked_lock_are_blockers(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp); self.create(root, lock=None)
            outside=root/'external.lock'; outside.write_text(LOCK,encoding='utf8')
            link=root/'Cargo.lock'
            try: link.symlink_to(outside)
            except (NotImplementedError,OSError): self.skipTest('symlinks unavailable')
            got=check.problems(root,which=lambda x:'/fake/'+x,scripts_dir=root/'missing',python_version=(3,14),python_impl='pypy')
            self.assertTrue(any('symlink' in item for item in got))
            self.assertTrue(any('CPython 3.10' in item for item in got))
            self.assertEqual(outside.read_text(),LOCK)

if __name__ == '__main__':unittest.main()
