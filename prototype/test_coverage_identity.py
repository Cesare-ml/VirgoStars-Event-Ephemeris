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

from coverage_policy import (CoverageAwareReader, CoveragePolicy, SCHEMA,
                             MAX_POLICY_BYTES, MAX_REVIEWED_WINDOWS)


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

    def test_oversized_policy_rejected_before_parsing(self):
        self.policy_file.write_bytes(b" " * (MAX_POLICY_BYTES + 1))
        with self.assertRaisesRegex(ValueError, "exceeds maximum size"):
            self.load()

    def test_exact_policy_size_limit_is_allowed(self):
        # JSON permits trailing whitespace, testing the true byte limit.
        original = self.policy_file.read_bytes()
        self.policy_file.write_bytes(original + b" " * (MAX_POLICY_BYTES - len(original)))
        self.assertEqual(len(self.load().windows), 1)

    def test_too_many_reviewed_windows_rejected_before_sort(self):
        entry = {"family": "ingress", "body": "Moon", "start_us": "0",
                 "end_us_exclusive": "1", "reviewer": "r", "evidence": "e"}
        self.write_policy([entry] * (MAX_REVIEWED_WINDOWS + 1))
        self.assertLess(self.policy_file.stat().st_size, MAX_POLICY_BYTES)
        with self.assertRaisesRegex(ValueError, "too many coverage windows"):
            self.load()

    def test_non_ascii_decimal_microseconds_rejected(self):
        # Arabic-Indic digit isdecimal() is True, unlike Rust's ASCII parser.
        window = {"family": "ingress", "body": "Moon", "start_us": "\u0661",
                  "end_us_exclusive": "20", "reviewer": "r", "evidence": "e"}
        self.write_policy([window])
        with self.assertRaisesRegex(ValueError, "invalid coverage microseconds"):
            self.load()

    def test_duplicate_top_level_policy_keys_rejected(self):
        original = self.policy_file.read_text(encoding="utf-8")
        # Even if the duplicate values agree, the policy must be unambiguous.
        self.policy_file.write_text(original.replace('"schema":', '"schema": "conflict", "schema":', 1), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate coverage policy JSON key: schema"):
            self.load()

    def test_duplicate_nested_window_keys_rejected(self):
        original = self.policy_file.read_text(encoding="utf-8")
        self.policy_file.write_text(original.replace('"start_us":', '"start_us": "99", "start_us":', 1), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate coverage policy JSON key: start_us"):
            self.load()


if __name__ == "__main__":
    unittest.main()
