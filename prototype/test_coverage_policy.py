"""Tests use deliberately fabricated event data, not astronomical claims."""
from __future__ import annotations
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from virgostars_vseph_prototype import pack_dataset
from virgostars_vseph_reader import VSEPHReader
from coverage_policy import CoverageAwareReader, CoveragePolicy, SCHEMA


class CoverageSafetyTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "synthetic.vseph-p0"
        records = {"synod": {}, "motion": {"Mars": [(1, 0, 0, "r"), (2, 20, 1, "d")]},
                   "ingress": {"Moon": [(1, 0, 0), (2, 10, 1), (3, 1000, 0)]}}
        pack_dataset(records, {}, self.path)
        self.reader = VSEPHReader(self.path)
        self.policy_file = Path(self.directory.name) / "coverage.json"

    def policy(self, windows):
        self.policy_file.write_text(json.dumps({"schema": SCHEMA, "verification": "human-reviewed-only",
             "archive_sha256": hashlib.sha256(self.path.read_bytes()).hexdigest(),
             "reviewed_windows": windows}), encoding="utf-8")
        return CoveragePolicy.load(self.policy_file, self.path)

    @staticmethod
    def window(family, body, start, end):
        return {"family": family, "body": body, "start_us": str(start),
                "end_us_exclusive": str(end), "reviewer": "synthetic-test",
                "evidence": "fabricated events, explicit completeness for test only"}

    def test_empty_policy_never_claims_verified_state(self):
        safe = CoverageAwareReader(self.reader, self.policy([]))
        for moment in [0, 5, 10, 11, 500, 1000, 2000]:
            answer = safe.sign_at("Moon", moment)
            self.assertEqual(answer["status"], "unknown_coverage")
            self.assertIsNone(answer["state"])
        self.assertEqual(safe.sign_at("Moon", 500)["last_recorded_at_us"], 10)

    def test_explicit_reviewed_window_includes_start_excludes_end(self):
        safe = CoverageAwareReader(self.reader, self.policy([
            self.window("ingress", "Moon", 0, 20)]))
        self.assertEqual(safe.sign_at("Moon", 0)["state"]["sign"], 0)
        self.assertEqual(safe.sign_at("Moon", 10)["state"]["sign"], 1)
        self.assertEqual(safe.sign_at("Moon", 19)["status"], "verified")
        self.assertEqual(safe.sign_at("Moon", 20)["status"], "unknown_coverage")
        self.assertEqual(safe.sign_at("Moon", 1000)["status"], "unknown_coverage")

    def test_reviewed_window_without_initial_event_stays_unknown(self):
        safe = CoverageAwareReader(self.reader, self.policy([
            self.window("ingress", "Moon", 11, 2000)]))
        self.assertEqual(safe.sign_at("Moon", 500)["status"], "unknown_initial_state")
        self.assertIsNone(safe.sign_at("Moon", 500)["state"])
        self.assertEqual(safe.sign_at("Moon", 1000)["state"]["sign"], 0)

    def test_direction_state_does_not_use_ingress_coverage(self):
        safe = CoverageAwareReader(self.reader, self.policy([
            self.window("ingress", "Moon", 0, 20),
            self.window("motion", "Mars", 0, 21)]))
        self.assertEqual(safe.direction_at("Mars", 0)["state"]["direction"], "retrograde")
        self.assertEqual(safe.direction_at("Mars", 20)["state"]["direction"], "direct")
        self.assertEqual(safe.direction_at("Mars", 21)["status"], "unknown_coverage")

    def test_wrong_snapshot_hash_is_rejected(self):
        self.policy([])
        self.policy_file.write_text(self.policy_file.read_text().replace(
            hashlib.sha256(self.path.read_bytes()).hexdigest(), "0" * 64), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            CoveragePolicy.load(self.policy_file, self.path)

    def test_missing_evidence_overlap_and_invalid_time_are_rejected(self):
        w = self.window("ingress", "Moon", 0, 30)
        bad = dict(w, evidence="")
        with self.assertRaisesRegex(ValueError, "evidence"):
            self.policy([bad])
        with self.assertRaisesRegex(ValueError, "overlapping"):
            self.policy([w, self.window("ingress", "Moon", 20, 50)])
        with self.assertRaisesRegex(ValueError, "i64"):
            self.policy([dict(w, start_us=str(1 << 64))])
        with self.assertRaisesRegex(ValueError, "invalid coverage"):
            self.policy([dict(w, end_us_exclusive="not-an-instant")])

    def test_sign_at_with_missing_body_stays_unknown(self):
        safe = CoverageAwareReader(self.reader, self.policy([
            self.window("ingress", "Moon", 0, 50)]))
        self.assertEqual(safe.sign_at("Venus", 10)["status"], "unknown_coverage")


if __name__ == "__main__":
    unittest.main(verbosity=2)
