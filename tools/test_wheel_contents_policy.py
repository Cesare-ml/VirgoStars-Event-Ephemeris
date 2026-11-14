# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""No Cargo required: pre-install wheel packaging and dataset exclusion tests."""
from __future__ import annotations

from pathlib import Path
import stat
import tempfile
import unittest
import zipfile

from wheel_contents_policy import inspect_wheel_contents

META = "virgostars_event_ephemeris-0.1.0.dist-info"


class WheelContentsPolicyTests(unittest.TestCase):
    def wheel(self, root: Path, extras=()) -> Path:
        result = root / "virgostars_event_ephemeris-0.1.0-py3-none-any.whl"
        with zipfile.ZipFile(result, "w") as package:
            package.writestr(f"{META}/WHEEL", "Wheel-Version: 1.0\n")
            package.writestr(f"{META}/METADATA", "Name: virgostars-event-ephemeris\nVersion: 0.1.0\n")
            package.writestr("virgostars_events/__init__.py", "__version__ = '0.1.0'\n")
            for key, value in extras:
                package.writestr(key, value)
        return result

    def test_minimal_python_wheel_accepted(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertIsNone(inspect_wheel_contents(self.wheel(Path(folder))))

    def test_owner_dataset_formats_and_nested_directories_rejected(self):
        for path in (
            "datasets/2026-11-10/sinodi.csv",
            "virgostars_events/dataset/records.bin",
            "virgostars_events/owner.VSEPH-P0",
            "virgostars_events/owner.p0",
            "virgostars_events/catalog.sqlite3",
        ):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as folder:
                with self.assertRaisesRegex(ValueError, "scientific data"):
                    inspect_wheel_contents(self.wheel(Path(folder), [(path, "owner-data")]))

    def test_zip_path_traversal_and_windows_separators_rejected(self):
        for path in ("../outside.txt", "dir/../escape.txt", "/absolute.txt", "C:/windows/file.txt", "dir\\escape.txt"):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as folder:
                with self.assertRaisesRegex(ValueError, "unsafe wheel member path"):
                    inspect_wheel_contents(self.wheel(Path(folder), [(path, "x")]))

    def test_duplicate_members_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertWarns(UserWarning):
                wheel = self.wheel(Path(folder), [("virgostars_events/__init__.py", "duplicate")])
            with self.assertRaisesRegex(ValueError, "duplicate wheel member"):
                inspect_wheel_contents(wheel)

    def test_symlink_member_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            result = self.wheel(Path(folder))
            info = zipfile.ZipInfo("virgostars_events/owner")
            info.create_system = 3
            info.external_attr = (stat.S_IFLNK | 0o777) << 16
            with zipfile.ZipFile(result, "a") as package:
                package.writestr(info, "../../outside")
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                inspect_wheel_contents(result)

    def test_non_zip_file_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            result = Path(folder) / "bad.whl"
            result.write_bytes(b"not a ZIP")
            with self.assertRaisesRegex(ValueError, "invalid Python wheel ZIP"):
                inspect_wheel_contents(result)

    def test_missing_metadata_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            result = Path(folder) / "bad.whl"
            with zipfile.ZipFile(result, "w") as archive:
                archive.writestr("virgostars_events/__init__.py", "")
            with self.assertRaisesRegex(ValueError, "dist-info"):
                inspect_wheel_contents(result)

    def test_zip_bomb_declared_uncompressed_size_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            result = self.wheel(Path(folder))
            # The policy limits real unpacked size before pip is ever called.
            with zipfile.ZipFile(result, "a", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("virgostars_events/large.txt", b"x" * (65 * 1024 * 1024))
            with self.assertRaisesRegex(ValueError, "safe size limit"):
                inspect_wheel_contents(result)


if __name__ == "__main__":
    unittest.main()
