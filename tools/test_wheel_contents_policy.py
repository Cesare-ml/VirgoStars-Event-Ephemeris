# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""No Cargo required: pre-install wheel packaging and dataset exclusion tests."""
from __future__ import annotations

from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

import wheel_contents_policy as policy
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

    def test_known_compressed_owner_archive_suffixes_are_rejected(self):
        for name in ("owner.csv.gz", "owner.csv.zst", "owner.p0.gz",
                     "owner.p0.zst", "owner.vseph-p0.zst", "owner.sqlite3.zst"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as folder:
                wheel = self.wheel(Path(folder), [(f"virgostars_events/{name}", b"example")])
                with self.assertRaisesRegex(ValueError, "scientific data"):
                    inspect_wheel_contents(wheel)

    def test_renamed_raw_p0_magic_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            wheel = self.wheel(Path(folder), [
                ("virgostars_events/snapshot.bin", b"VSEPH-P0" + b"\x00" * 32)
            ])
            with self.assertRaisesRegex(ValueError, "renamed VSEPH-P0"):
                inspect_wheel_contents(wheel)

    def test_excessive_member_count_fails_before_reading_zip_members(self):
        with tempfile.TemporaryDirectory() as folder:
            wheel = self.wheel(Path(folder), [("virgostars_events/extra.txt", b"x")])
            with patch.object(policy, "MAX_MEMBERS", 3):
                with self.assertRaisesRegex(ValueError, "too many ZIP members"):
                    inspect_wheel_contents(wheel)

    def test_extreme_zip_expansion_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            wheel = self.wheel(Path(folder))
            with zipfile.ZipFile(wheel, "a", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("virgostars_events/repetitive.txt", b"x" * (2 * 1024 * 1024))
            with self.assertRaisesRegex(ValueError, "excessive ZIP expansion"):
                inspect_wheel_contents(wheel)

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

    def test_case_insensitive_windows_wheel_paths_must_not_collide(self):
        # The archive is valid ZIP and names are unique in the central
        # directory, but installation to Windows could overwrite a file.
        for member in (
            "VirgoStars_Events/__INIT__.PY",
            "virgostars_events/__INIT__.py",
        ):
            with self.subTest(member=member), tempfile.TemporaryDirectory() as folder:
                wheel = self.wheel(Path(folder), [(member, "untrusted-shadow")])
                with self.assertRaisesRegex(ValueError, "case-insensitive wheel path collision"):
                    inspect_wheel_contents(wheel)

        # Different basenames in the same package remain valid.
        with tempfile.TemporaryDirectory() as folder:
            wheel = self.wheel(Path(folder), [
                ("virgostars_events/README.txt", "documentation"),
            ])
            self.assertIsNone(inspect_wheel_contents(wheel))

    def test_windows_device_name_regex_directly_matches_with_extensions(self):
        # Keep the regex itself under test: a doubly escaped \Z or \.
        # would silently accept Windows device basenames even though the
        # surrounding invalid-character checks continue to pass.
        for name in ("CON", "con.txt", "NUL.TXT", "AUX", "PRN.csv",
                     "com1.pyd", "COM9.dll", "lPt9.log", "lpt1"):
            with self.subTest(reserved=name):
                self.assertIsNotNone(policy._WINDOWS_DEVICE.fullmatch(name))
                self.assertTrue(policy.unsafe_windows_component(name))

        for name in ("console.py", "com0.pyd", "com10.py",
                     "lpt0.dll", "lpt10.dll", "file.con", "printer.txt"):
            with self.subTest(allowed=name):
                self.assertIsNone(policy._WINDOWS_DEVICE.fullmatch(name))
                self.assertFalse(policy.unsafe_windows_component(name))

    def test_windows_reserved_names_and_ntfs_stream_paths_rejected(self):
        # These names are legal ZIP member strings but are unsafe or
        # non-portable when pip installs a wheel on Windows.
        for member in (
            "virgostars_events/CON.py",
            "virgostars_events/nul.txt",
            "virgostars_events/COM1.pyd",
            "virgostars_events/lPt9.dll",
            "virgostars_events/aux",
            "virgostars_events/PRN/child.py",
            "virgostars_events/data.txt:alternate",
            "virgostars_events/trailing.",
            "virgostars_events/trailing ",
            "virgostars_events/bad?.py",
            "virgostars_events/control\\x01.txt",
        ):
            with self.subTest(member=member), tempfile.TemporaryDirectory() as folder:
                wheel = self.wheel(Path(folder), [(member, b"fake")])
                with self.assertRaisesRegex(ValueError, "unsafe wheel member path"):
                    inspect_wheel_contents(wheel)

        # Similar ordinary wheel member names are valid and must not be
        # rejected just because they contain the device-name prefix.
        for member in ("virgostars_events/console.py", "virgostars_events/com10.py"):
            with self.subTest(valid=member), tempfile.TemporaryDirectory() as folder:
                self.assertIsNone(inspect_wheel_contents(
                    self.wheel(Path(folder), [(member, b"valid")])
                ))

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
