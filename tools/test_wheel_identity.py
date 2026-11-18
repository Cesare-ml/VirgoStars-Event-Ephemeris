# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Dependency-free synthetic regressions for VirgoStars wheel metadata identity."""
from __future__ import annotations

from pathlib import Path
import tempfile
import unittest
import zipfile

from wheel_identity import validate_wheel_identity


DIST = "virgostars_event_ephemeris"
VERSION = "0.1.0"
DIST_INFO = f"{DIST}-{VERSION}.dist-info"


class WheelIdentityTests(unittest.TestCase):
    def wheel(self, root: Path, *, filename: str | None = None,
              dist_info: str = DIST_INFO, name: str = "virgostars-event-ephemeris",
              version: str = VERSION, wheel_version: str = "1.0",
              extra_metadata: str = "") -> Path:
        result = root / (filename or f"{DIST}-{VERSION}-cp310-abi3-win_amd64.whl")
        with zipfile.ZipFile(result, "w") as package:
            package.writestr(f"{dist_info}/WHEEL", f"Wheel-Version: {wheel_version}\n")
            package.writestr(
                f"{dist_info}/METADATA",
                f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n{extra_metadata}",
            )
        return result

    def check(self, wheel: Path, dist_info: str = DIST_INFO) -> None:
        with zipfile.ZipFile(wheel) as package:
            validate_wheel_identity(wheel, package, dist_info)

    def test_valid_native_and_pure_python_tags(self):
        for filename in (f"{DIST}-{VERSION}-cp310-abi3-win_amd64.whl",
                         f"{DIST}-{VERSION}-py3-none-any.whl",
                         f"{DIST}-{VERSION}-1-cp310-abi3-manylinux_2_17_x86_64.whl"):
            with self.subTest(filename=filename), tempfile.TemporaryDirectory() as folder:
                self.check(self.wheel(Path(folder), filename=filename))

    def test_rejects_unrelated_wheel_disguised_as_virgostars(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), filename="other-0.1.0-py3-none-any.whl")
            with self.assertRaisesRegex(ValueError, "project distribution"):
                self.check(archive)

    def test_rejects_dist_info_unrelated_to_wheel_filename(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), dist_info="other-0.1.0.dist-info")
            with self.assertRaisesRegex(ValueError, "does not match wheel filename"):
                self.check(archive, "other-0.1.0.dist-info")

    def test_rejects_metadata_name_mismatch(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), name="malicious-wheel")
            with self.assertRaisesRegex(ValueError, "METADATA Name"):
                self.check(archive)

    def test_rejects_metadata_version_mismatch(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), version="9.9.9")
            with self.assertRaisesRegex(ValueError, "METADATA Version"):
                self.check(archive)

    def test_rejects_ambiguous_duplicate_metadata_headers(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), extra_metadata="Name: virgostars-event-ephemeris\n")
            with self.assertRaisesRegex(ValueError, "exactly one Name"):
                self.check(archive)

    def test_rejects_unsupported_wheel_version(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), wheel_version="2.0")
            with self.assertRaisesRegex(ValueError, "Wheel-Version"):
                self.check(archive)

    def test_rejects_oversized_metadata(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), extra_metadata="Description: " + "x" * (260 * 1024))
            with self.assertRaisesRegex(ValueError, "metadata too large"):
                self.check(archive)

    def test_accepts_normalized_project_name_variants(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.wheel(Path(folder), name="VirgoStars.Event_Ephemeris")
            self.check(archive)


if __name__ == "__main__":
    unittest.main()
