# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Fail-closed limits for audit JSON transport; synthetic-only tests."""
import unittest

from audit_report_json import MAX_AUDIT_REPORT_BYTES, decode_audit_report


class AuditReportLimitTests(unittest.TestCase):
    def test_utf8_report_limit_checks_actual_bytes_before_json_decode(self):
        # A short character count is not a short UTF-8 payload.
        for payload in (
            '{"note":"' + "a" * MAX_AUDIT_REPORT_BYTES + '"}',
            '{"note":"' + "é" * (MAX_AUDIT_REPORT_BYTES // 2) + '"}',
        ):
            with self.subTest(size=len(payload)), self.assertRaisesRegex(
                RuntimeError, "exceeds 65536 UTF-8 bytes"
            ):
                decode_audit_report(payload, source="Python")
        prefix = '{"note":"'
        suffix = '"}'
        accepted = prefix + "a" * (MAX_AUDIT_REPORT_BYTES - len(prefix) - len(suffix)) + suffix
        self.assertEqual(len(accepted.encode("utf-8")), MAX_AUDIT_REPORT_BYTES)
        self.assertEqual(len(decode_audit_report(accepted, source="Python")["note"]),
                         MAX_AUDIT_REPORT_BYTES - len(prefix) - len(suffix))

    def test_lone_surrogate_in_json_key_or_value_is_rejected(self):
        for payload in (
            '{"note":"\\ud800"}',
            '{"\\udfff":0}',
            '{"note":"' + chr(0xD800) + '"}',
        ):
            with self.subTest(payload=repr(payload)), self.assertRaisesRegex(
                RuntimeError, "invalid or ambiguous JSON"
            ):
                decode_audit_report(payload, source="Rust")
        good_pair = '{"note":"\\ud83d\\ude00"}'
        self.assertEqual(decode_audit_report(good_pair, source="Rust"), {"note": "😀"})

    def test_excessive_nesting_fails_as_runtime_error(self):
        payload = '{"x":' + '[' * 2000 + '0' + ']' * 2000 + '}'
        with self.assertRaisesRegex(RuntimeError, "invalid or ambiguous JSON"):
            decode_audit_report(payload, source="Rust")


if __name__ == "__main__":
    unittest.main()
