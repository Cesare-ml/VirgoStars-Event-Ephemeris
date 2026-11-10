# The contents of this file are subject to CPAL-1.0. See LICENSE and NOTICE.
# Original Developer: Cesare-ml (VirgoStars project).
# Copyright (c) 2026 Cesare-ml. Powered by VirgoStars — https://www.virgostars.it/
"""Independent VSEPH-P0 integrity tests (synthetic fixture, no real data needed)."""
import base64
import hashlib
import json
import os
import struct
import zlib
import tempfile
import unittest
from pathlib import Path

from audit_p0_bytes import audit_file

FIXTURE_B64 = (Path(__file__).resolve().parent.parent /
               'crates/events-p0/tests/fixtures/synthetic.p0.base64')
SHA = '4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499'


class AuditP0BytesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.good = base64.b64decode(FIXTURE_B64.read_text().strip(), validate=True)
        cls.scratch = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.scratch.cleanup)

    def audit(self, content, sha=None):
        path = Path(self.scratch.name) / 'test.p0'
        path.write_bytes(content)
        return audit_file(path, sha)

    def test_golden_counts_and_hash(self):
        result = self.audit(self.good, SHA)
        self.assertEqual(result['records'], 8)
        self.assertEqual(result['blocks'], 4)
        self.assertEqual(result['sections'], {'ingress': 3, 'motion': 2, 'synod': 3})

    def test_wrong_expected_archive_hash_rejected(self):
        with self.assertRaisesRegex(ValueError, 'SHA-256'):
            self.audit(self.good, '0' * 64)

    def test_magic_corruption_and_short_input_rejected(self):
        with self.assertRaises(ValueError):
            self.audit(b'')
        with self.assertRaises(ValueError):
            self.audit(b'VSEPH-P0')
        corrupted = bytearray(self.good)
        corrupted[0] ^= 1
        with self.assertRaises(ValueError):
            self.audit(corrupted)

    def test_unsafe_header_size_rejected(self):
        corrupted = bytearray(self.good)
        struct.pack_into('<I', corrupted, 8, 0xffffffff)
        with self.assertRaisesRegex(ValueError, 'header size'):
            self.audit(corrupted)

    def test_compressed_block_corruption_detected(self):
        corrupted = bytearray(self.good)
        corrupted[-6] ^= 0x7f
        with self.assertRaises((ValueError, zlib.error)):
            self.audit(corrupted)

    def test_appended_and_truncated_payload_rejected(self):
        with self.assertRaises(ValueError):
            self.audit(self.good + b'extra')
        with self.assertRaises((ValueError, zlib.error)):
            self.audit(self.good[:-1])

    def test_unqueried_block_corruption_detected_by_full_audit(self):
        # Damage the ingress block while leaving the independently indexed
        # Mercury synod block intact. An exhaustive audit must still fail.
        altered = bytearray(self.good)
        header_length = struct.unpack_from('<I', altered, 8)[0]
        altered[12 + header_length] ^= 1
        with self.assertRaises((ValueError, zlib.error)):
            self.audit(altered)

    @staticmethod
    def build_semantic_fixture(chunks):
        """Construct deliberately synthetic, checksum-valid P0 blocks.

        Each chunk: (family, body, min_us, max_us, count, uncompressed bytes).
        This prevents integrity/hash failures from masking the source-ID bug.
        """
        toc = []
        payload = bytearray()
        sections = {}
        for family, body, first, last, count, raw in sorted(chunks):
            compressed = zlib.compress(raw)
            toc.append({
                'family': family, 'body': body, 'start': len(payload),
                'length': len(compressed), 'raw_length': len(raw),
                'count': count, 'min_us': str(first), 'max_us': str(last),
                'raw_sha256': hashlib.sha256(raw).hexdigest()
            })
            payload.extend(compressed)
            sections[family] = sections.get(family, 0) + count
        header = json.dumps({
            'format': 'VSEPH-P0-EXPERIMENTAL',
            'format_version': [0, 0, 1],
            'codec': 'zlib',
            'block_records_max': 512,
            'sections': sections,
            'table_of_contents': toc,
        }, sort_keys=True, separators=(',', ':')).encode()
        return b'VSEPH-P0' + struct.pack('<I', len(header)) + header + payload

    def test_zero_original_source_id_rejected_even_with_valid_checksum(self):
        raw = struct.pack('<q', 0) + bytes([0, 0, 1])
        archive = self.build_semantic_fixture([('ingress', 'Moon', 0, 0, 1, raw)])
        with self.assertRaisesRegex(ValueError, 'zero or duplicate original source ID'):
            self.audit(archive)

    def test_reused_source_id_inside_one_block_rejected(self):
        raw = struct.pack('<q', 0) + bytes([4, 0, 0, 4, 10, 1])
        archive = self.build_semantic_fixture([('ingress', 'Moon', 0, 10, 2, raw)])
        with self.assertRaisesRegex(ValueError, 'duplicate original source ID'):
            self.audit(archive)

    def test_reused_source_id_across_different_blocks_rejected(self):
        first = struct.pack('<q', 0) + bytes([4, 0, 0])
        second = struct.pack('<q', 10) + bytes([4, 0, 1])
        archive = self.build_semantic_fixture([
            ('ingress', 'Moon', 0, 0, 1, first),
            ('ingress', 'Moon', 10, 10, 1, second),
        ])
        with self.assertRaisesRegex(ValueError, 'duplicate original source ID'):
            self.audit(archive)

    def test_reused_source_id_across_bodies_of_same_family_rejected(self):
        raw = struct.pack('<q', 0) + bytes([4, 0, 0])
        archive = self.build_semantic_fixture([
            ('ingress', 'Moon', 0, 0, 1, raw),
            ('ingress', 'Venus', 0, 0, 1, raw),
        ])
        with self.assertRaisesRegex(ValueError, 'duplicate original source ID'):
            self.audit(archive)

    def test_same_source_id_in_independent_family_is_allowed(self):
        ingress = struct.pack('<q', 0) + bytes([4, 0, 0])
        motion = struct.pack('<q', 0) + bytes([4, 0, 1, 0])
        archive = self.build_semantic_fixture([
            ('ingress', 'Moon', 0, 0, 1, ingress),
            ('motion', 'Mars', 0, 0, 1, motion),
        ])
        report = self.audit(archive)
        self.assertEqual(report['records'], 2)
        self.assertEqual(report['sections'], {'ingress': 1, 'motion': 1})

    @unittest.skipUnless(os.environ.get('VSEPH_P0_PATH'), 'optional reconstructed public dataset')
    def test_real_snapshot_full_block_audit(self):
        path = Path(os.environ['VSEPH_P0_PATH'])
        result = audit_file(path, 'c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e')
        self.assertEqual(result['records'], 380_844)
        self.assertEqual(result['blocks'], 761)


if __name__ == '__main__':
    unittest.main(verbosity=2)
