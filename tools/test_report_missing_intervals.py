"""Synthetic tests: human-readable gap inventory never claims coverage."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
import json


SPEC = importlib.util.spec_from_file_location(
    "report_missing_intervals", Path(__file__).with_name("report_missing_intervals.py")
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def stage(start, end, spans=None, **extra):
    return {
        "kind": "unverified_sparse_interval_staging",
        "coverage_certified": False, "generation_complete": False,
        "dataset": {"sha256": "a" * 64}, "mode": "plan",
        "family": "synods", "body": "Moon",
        "requested_window": {"start_us": start, "end_us": end},
        "sparse_threshold_us": 1000000, "sparse_spans": spans or [], "candidates": [],
        **extra,
    }


def gap(a, b, kind="between_recorded_anchors"):
    return {"start_us": a, "end_us": b, "kind": kind,
            "previous_original_source_id": 0, "next_original_source_id": 12,
            "previous_recorded_us": a, "next_recorded_us": b}


class MissingIntervalsReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def save(self, name, data):
        path = self.root / name
        path.write_text(json.dumps(data), encoding="utf-8")
        return str(path)

    def test_utc_dates_sparse_flags_and_unchecked_windows(self):
        day = 86_400_000_000
        a = self.save("a.json", stage(0, 3 * day, [gap(day, 3 * day)]))
        b = self.save("b.json", stage(5 * day, 6 * day))
        result = MODULE.aggregate([b, a])
        text = MODULE.markdown(result)
        self.assertIn("1970-01-02T00:00:00.000000Z", text)
        self.assertIn("1970-01-04T00:00:00.000000Z", text)
        self.assertIn("1970-01-06T00:00:00.000000Z", text)
        self.assertIn("0 / 12", text)
        self.assertIn("non_verificato", text)
        self.assertIn("copertura NON certificata", text)
        self.assertFalse(result["coverage_certified"])
        self.assertEqual(len(result["unchecked_between_windows"]), 1)

    def test_no_flags_does_not_claim_completeness(self):
        path = self.save("a.json", stage(0, 2000000))
        text = MODULE.markdown(MODULE.aggregate([path]))
        self.assertIn("Nessun intervallo", text)
        self.assertIn("copertura NON dimostrata", text)

    def test_refuses_forged_verified_staging(self):
        path = self.save("forged.json", stage(0, 2000000, coverage_certified=True))
        with self.assertRaisesRegex(ValueError, "certification"):
            MODULE.aggregate([path])

    def test_refuses_unordered_or_outside_spans(self):
        with self.assertRaisesRegex(ValueError, "unsorted/overlapping"):
            MODULE.aggregate([self.save("bad.json", stage(0, 8000000,
                [gap(3000000, 5000000), gap(4000000, 7000000)]))])

    def test_refuses_mismatched_archives_and_overlapping_windows(self):
        a = self.save("a.json", stage(0, 2000000))
        b = self.save("b.json", stage(1000000, 3000000))
        with self.assertRaisesRegex(ValueError, "overlapping audit windows"):
            MODULE.aggregate([a, b])
        c = self.save("c.json", stage(3000000, 5000000, dataset={"sha256": "b" * 64}))
        with self.assertRaisesRegex(ValueError, "sha256 differs"):
            MODULE.aggregate([a, c])

    def test_rejects_bool_timestamp_and_plan_candidates(self):
        path = self.save("bool.json", stage(True, 2000000))
        with self.assertRaisesRegex(ValueError, "expected integer"):
            MODULE.aggregate([path])
        path = self.save("plan.json", stage(0, 2000000, candidates=[{}]))
        with self.assertRaisesRegex(ValueError, "plan mode"):
            MODULE.aggregate([path])

    def test_negative_us_and_exclusive_end_format(self):
        self.assertEqual(MODULE.instant(-1), "1969-12-31T23:59:59.999999Z")
        self.assertEqual(MODULE.instant(0), "1970-01-01T00:00:00.000000Z")


if __name__ == "__main__":
    unittest.main()
