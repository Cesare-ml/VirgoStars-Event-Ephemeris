# CPAL-1.0 original source code; see LICENSE / NOTICE.
# Copyright (c) 2026 Cesare-ml (VirgoStars project).
# Powered by VirgoStars https://www.virgostars.it/
"""Offline smoke tests for the deterministic dataset assembler."""
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from assemble_dataset import file_chunks, rebuild


class DatasetAssemblyTests(unittest.TestCase):
    def test_reassemble_and_validate_direct_and_split_file(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            target = base / "out"
            data = b"id,segno,dataora_utc,elemento_celeste\n1,0,2026-01-01 00:00:00+00,Moon\n"
            source_dir = base / "ingresso_segni.csv.parts"
            source_dir.mkdir()
            (source_dir / "part-000").write_bytes(data[:12])
            (source_dir / "part-001").write_bytes(data[12:])
            meta = {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data), "rows": 1}
            self.assertEqual(rebuild(base, target, "ingresso_segni.csv", meta).read_bytes(), data)
            self.assertEqual(len(file_chunks(base, "ingresso_segni.csv")), 2)

    def test_reject_tampering_and_missing_parts(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            folder = base / "sinodi.csv.parts"
            folder.mkdir()
            (folder / "part-000").write_bytes(b"wrong")
            good = {"sha256": hashlib.sha256(b"other").hexdigest(), "size": 5, "rows": 0}
            with self.assertRaisesRegex(ValueError, "Integrity mismatch"):
                rebuild(base, base / "out", "sinodi.csv", good)
            (folder / "part-002").write_bytes(b"gap")
            with self.assertRaisesRegex(ValueError, "non-contiguous"):
                file_chunks(base, "sinodi.csv")

    def test_no_path_traversal(self):
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaisesRegex(ValueError, "Unexpected dataset file"):
                file_chunks(Path(root), "../../secret.env")


if __name__ == "__main__":
    unittest.main()
