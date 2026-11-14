# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Exact-helper tests; no Cargo or dataset required."""
import json
import unittest

from audit_report_json import decode_audit_report


class StrictAuditReportTests(unittest.TestCase):
    def test_valid_p0_audit_report_preserves_nested_counts(self):
        original = {
            "sha256": "a" * 64, "bytes": 20, "blocks": 4, "records": 8,
            "sections": {"synod": 3, "motion": 2, "ingress": 3},
            "coverage_certified": False,
        }
        self.assertEqual(decode_audit_report(json.dumps(original), source="Rust"), original)

    def test_duplicate_top_level_nested_and_escaped_keys_fail_closed(self):
        for payload in (
            '{"records":8,"records":8}',
            '{"sections":{"synod":3,"synod":3}}',
            '{"sections":{},"\\u0073ections":{}}',
            '{"metadata":{"a":{"id":1,"id":1}}}',
        ):
            with self.subTest(payload=payload), self.assertRaisesRegex(
                RuntimeError, "ambiguous JSON.*duplicate JSON object key"
            ):
                decode_audit_report(payload, source="Python")

    def test_nonfinite_and_nonstandard_json_numbers_fail_closed(self):
        for invalid in ("NaN", "Infinity", "-Infinity", "1e400", "-1e400"):
            with self.subTest(value=invalid), self.assertRaisesRegex(
                RuntimeError, "invalid or ambiguous JSON"
            ):
                decode_audit_report('{"unused":' + invalid + '}', source="Rust")

    def test_nonobjects_nontext_and_invalid_syntax_are_rejected(self):
        for payload in (None, False, b"{}", "[]", "null", "true", "42", "{"):
            with self.subTest(payload=payload), self.assertRaises(RuntimeError):
                decode_audit_report(payload, source="Rust")

    def test_nested_lists_and_valid_finite_fractions_are_preserved(self):
        payload = '{"unused":[{"detail":[true,null,1,1.5,-0.25]}]}'
        actual = decode_audit_report(payload, source="Python")
        self.assertEqual(actual, {"unused": [{"detail": [True, None, 1, 1.5, -0.25]}]})


if __name__ == "__main__":
    unittest.main()
