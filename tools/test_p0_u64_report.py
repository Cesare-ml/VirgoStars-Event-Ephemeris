# SPDX-License-Identifier: CPAL-1.0
# Powered by VirgoStars — https://www.virgostars.it/
"""Bound P0 audit counters to the Rust JSON u64 representable domain."""
from __future__ import annotations

import unittest

from validate_rust import _is_u64, validate_audit_shape

MAX_U64 = (1 << 64) - 1
DIGEST = 'a' * 64


class AuditU64Test(unittest.TestCase):
    def valid(self):
        return {
            'sha256': DIGEST, 'bytes': 20, 'blocks': 1, 'records': 3,
            'sections': {'synod': 1, 'motion': 1, 'ingress': 1},
        }

    def test_u64_scalar_boundary_rejects_boolean_float_and_overflow(self):
        for value, valid in ((-1, False), (0, True), (1, True),
                             (MAX_U64, True), (MAX_U64 + 1, False),
                             (10 ** 100, False), (True, False), (1.0, False)):
            with self.subTest(value=value):
                self.assertIs(_is_u64(value), valid)

    def test_u64_maximum_file_length_counts_and_sections_are_representable(self):
        report = self.valid()
        report.update(bytes=MAX_U64, blocks=MAX_U64, records=MAX_U64,
                      sections={'synod': MAX_U64})
        validate_audit_shape(report, 'Rust')

    def test_above_u64_file_length_rejected(self):
        report = self.valid()
        for count in (MAX_U64 + 1, 10 ** 100):
            with self.subTest(value=count), self.assertRaisesRegex(RuntimeError, 'byte length'):
                report['bytes'] = count
                validate_audit_shape(report, 'Python')

    def test_above_u64_block_or_record_count_rejected(self):
        for key in ('blocks', 'records'):
            report = self.valid()
            report[key] = MAX_U64 + 1
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, 'block/record counts'):
                validate_audit_shape(report, 'Rust')

    def test_above_u64_section_count_rejected(self):
        report = self.valid()
        report['sections'] = {'synod': MAX_U64 + 1, 'motion': 0, 'ingress': 0}
        with self.assertRaisesRegex(RuntimeError, 'family section counts'):
            validate_audit_shape(report, 'Python')


if __name__ == '__main__':
    unittest.main()
