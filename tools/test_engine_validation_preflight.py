# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Synthetic-only tests: declarations are not scientific evidence."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from engine_validation_preflight import MAX_BYTES, check_plan_bytes


def synthetic_plan():
    # All names, hashes, tolerances and review records below are invented.
    return {
        "schema": "engine-validation-plan-v1",
        "scope": {
            "bodies": ["SyntheticA", "SyntheticB"],
            "start_us": -10, "end_us": 10,
            "quantities": ["longitude_degrees", "latitude_speed_degrees_per_day"],
            "center": "synthetic center", "frame": "synthetic frame",
            "corrections": "synthetic corrections", "time_convention": "synthetic time",
        },
        "engine": {
            "source_revision": "a" * 40, "binary_sha256": "b" * 64,
            "binding_sha256": "c" * 64, "kernel_sha256": "d" * 64,
            "build_attestation": "invented build record",
            "kernel_provenance": "invented kernel source",
            "time_data_identity": "invented time inputs",
        },
        "reference": {
            "implementation": "invented reference", "version": "synthetic",
            "provenance": "invented reference record", "shared_dependencies": "declared none",
            "independence_rationale": "invented separation",
        },
        "cases": {"selection": "invented grid", "holdout": "invented holdout", "expected_count": 8},
        "tolerances": [
            {"body": body, "quantity": quantity, "max_absolute_error": 0.1,
             "unit": unit, "rationale": "invented test threshold"}
            for body in ("SyntheticA", "SyntheticB")
            for quantity, unit in (("longitude_degrees", "degrees"),
                                   ("latitude_speed_degrees_per_day", "degrees/day"))
        ],
        "review": {
            "reviewer": "invented reviewer", "relationship": "invented relationship",
            "preapproval_record": "invented preregistration",
        },
    }


def payload(plan):
    return json.dumps(plan, allow_nan=False).encode("utf-8")


class EnginePlanPreflightTest(unittest.TestCase):
    def assert_invalid(self, plan):
        with self.assertRaises(ValueError):
            check_plan_bytes(payload(plan))

    def test_well_formed_synthetic_plan_does_not_authorize_any_claim(self):
        with patch("builtins.open", side_effect=AssertionError("artifact opened")):
            report = check_plan_bytes(payload(synthetic_plan()))
        self.assertTrue(report["declarations_well_formed"])
        for key in ("engine_independently_validated", "astronomical_accuracy_verified",
                    "coverage_certified", "safe_to_import"):
            self.assertIs(report[key], False)
        self.assertEqual(len(report["plan_sha256"]), 64)

    def test_packaged_example_is_synthetic_and_matches_test_fixture(self):
        example = Path(__file__).with_name("fixtures") / "engine_validation_plan.synthetic.json"
        raw = example.read_bytes()
        self.assertEqual(json.loads(raw), synthetic_plan())
        self.assertIs(check_plan_bytes(raw)["engine_independently_validated"], False)

    def test_missing_unknown_and_placeholder_fields_rejected(self):
        for mutation in ("missing", "unknown", "placeholder"):
            with self.subTest(mutation=mutation):
                plan = synthetic_plan()
                if mutation == "missing":
                    del plan["engine"]["build_attestation"]
                elif mutation == "unknown":
                    plan["engine"]["extra"] = "unexpected"
                else:
                    plan["review"]["reviewer"] = " TBD "
                self.assert_invalid(plan)

    def test_invalid_digests_and_scope_times_rejected(self):
        for digest in ("a" * 63, "A" * 64, "g" * 64, 123):
            plan = synthetic_plan()
            plan["engine"]["kernel_sha256"] = digest
            self.assert_invalid(plan)
        for start, end in ((True, 2), (0, 0), (1, -1), (-(2**63)-1, 0), (0, 2**63)):
            plan = synthetic_plan()
            plan["scope"]["start_us"], plan["scope"]["end_us"] = start, end
            self.assert_invalid(plan)

    def test_each_body_quantity_requires_one_matching_unit_and_threshold(self):
        for mutation in ("missing", "duplicate", "wrong_unit", "bool", "negative", "overflow"):
            with self.subTest(mutation=mutation):
                plan = synthetic_plan()
                if mutation == "missing":
                    plan["tolerances"].pop()
                elif mutation == "duplicate":
                    plan["tolerances"][-1] = dict(plan["tolerances"][0])
                elif mutation == "wrong_unit":
                    plan["tolerances"][0]["unit"] = "degrees/day"
                else:
                    plan["tolerances"][0]["max_absolute_error"] = {
                        "bool": True, "negative": -1, "overflow": 10**400,
                    }[mutation]
                self.assert_invalid(plan)

    def test_duplicate_bodies_and_case_count_are_checked(self):
        for value in (0, True, 1_000_001):
            plan = synthetic_plan()
            plan["cases"]["expected_count"] = value
            self.assert_invalid(plan)
        plan = synthetic_plan()
        plan["scope"]["bodies"] = ["SyntheticA", "synthetica"]
        self.assert_invalid(plan)

    def test_ambiguous_json_nonfinite_unicode_and_oversize_rejected(self):
        valid = payload(synthetic_plan())
        samples = [
            valid.replace(b'"schema":', b'"schema":"engine-validation-plan-v1","schema":', 1),
            valid.replace(b"0.1", b"NaN", 1),
            valid.replace(b"0.1", b"1e999", 1),
            valid.replace(b"0.1", b"1e-999", 1),
            valid.replace(b"invented reviewer", b"\\ud800", 1),
            b'\xff', b" " * (MAX_BYTES + 1),
        ]
        for item in samples:
            with self.subTest(item=item[:40]), self.assertRaises(ValueError):
                check_plan_bytes(item)

    def test_cli_emits_no_report_on_failure(self):
        script = Path(__file__).with_name("engine_validation_preflight.py")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.json"
            path.write_bytes(payload(synthetic_plan()))
            success = subprocess.run([sys.executable, str(script), str(path)], capture_output=True)
            self.assertEqual(success.returncode, 0)
            self.assertIs(json.loads(success.stdout)["coverage_certified"], False)
            path.write_text('{"schema":"broken"}', encoding="utf-8")
            failure = subprocess.run([sys.executable, str(script), str(path)], capture_output=True)
            self.assertNotEqual(failure.returncode, 0)
            self.assertEqual(failure.stdout, b"")


if __name__ == "__main__":
    unittest.main()
