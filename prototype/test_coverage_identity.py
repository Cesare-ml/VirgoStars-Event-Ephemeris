# The contents of this file are subject to the Common Public Attribution
# License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Powered by VirgoStars — https://www.virgostars.it/
"""Synthetic archive identity tests (no scientific dataset or external packages)."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from coverage_policy import CoverageAwareReader, CoveragePolicy, SCHEMA


class DummyReader:
    def __init__(self, path, on_at=None):
        self.path = path
        self.on_at = on_at

    @staticmethod
    def _time(value):
        return value

    def at(self, family, body, instant):
        if self.on_at:
            self.on_at()
        return (12, 10, 3) if family == "ingress" and body == "Moon" else None


class ArchiveBindingTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.archive_a = self.directory / "a.p0"
        self.archive_b = self.directory / "b.p0"
        self.archive_a.write_bytes(b"synthetic A")
        self.archive_b.write_bytes(b"synthetic B")
        self.policy_file = self.directory / "policy.json"
        self.write_policy()

    def write_policy(self, windows=None):
        if windows is None:
            windows = [{"family": "ingress", "body": "Moon", "start_us": "0",
                        "end_us_exclusive": "20", "reviewer": "synthetic test",
                        "evidence": "synthetic-only explicit interval"}]
        self.policy_file.write_text(json.dumps({"schema": SCHEMA,
            "verification": "human-reviewed-only",
            "archive_sha256": hashlib.sha256(self.archive_a.read_bytes()).hexdigest(),
            "reviewed_windows": windows}), encoding="utf-8")

    def load(self):
        return CoveragePolicy.load(self.policy_file, self.archive_a)

    def test_matching_file_keeps_reviewed_boundary_semantics(self):
        safe = CoverageAwareReader(DummyReader(self.archive_a), self.load())
        self.assertEqual(safe.sign_at("Moon", 10)["state"]["sign"], 3)
        self.assertEqual(safe.sign_at("Moon", 20)["status"], "unknown_coverage")

    def test_reader_cannot_point_at_different_archive(self):
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            CoverageAwareReader(DummyReader(self.archive_b), self.load())

    def test_file_changed_after_initial_binding_fails_closed(self):
        safe = CoverageAwareReader(DummyReader(self.archive_a), self.load())
        self.archive_a.write_bytes(b"changed bytes")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            safe.sign_at("Moon", 10)

    def test_file_changed_during_read_fails_closed(self):
        def mutate():
            self.archive_a.write_bytes(b"changed bytes")
        safe = CoverageAwareReader(DummyReader(self.archive_a, mutate), self.load())
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            safe.sign_at("Moon", 10)

    def test_unreviewed_state_does_not_bypass_file_identity(self):
        self.write_policy([])
        safe = CoverageAwareReader(DummyReader(self.archive_a), self.load())
        self.assertEqual(safe.sign_at("Moon", 10)["status"], "unknown_coverage")
        self.archive_a.write_bytes(b"changed bytes")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            safe.sign_at("Moon", 10)

    def test_reader_must_have_real_path(self):
        reader = DummyReader(None)
        with self.assertRaisesRegex(TypeError, "file-backed"):
            CoverageAwareReader(reader, self.load())


if __name__ == "__main__":
    unittest.main()
