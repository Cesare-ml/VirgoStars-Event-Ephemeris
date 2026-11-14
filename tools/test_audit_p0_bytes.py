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

from audit_p0_bytes import _varint, audit_file

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

    def test_unsafe_header_size_and_noninteger_json_metadata_rejected(self):
        corrupted = bytearray(self.good)
        struct.pack_into('<I', corrupted, 8, 0xffffffff)
        with self.assertRaisesRegex(ValueError, 'header size'):
            self.audit(corrupted)

        # Rust serde_json keeps bool/float/int distinct. Python's ordinary
        # equality does not: [False, False, True] == [0, 0, 1] and 3.0 == 3.
        # Keep the original compressed payload and block SHA-256 unchanged,
        # so only the malformed metadata can account for rejection.
        def rewrite_header(archive, new_document):
            length = struct.unpack_from('<I', archive, 8)[0]
            payload = archive[12 + length:]
            encoded = json.dumps(new_document, sort_keys=True, separators=(',', ':')).encode()
            return b'VSEPH-P0' + struct.pack('<I', len(encoded)) + encoded + payload

        length = struct.unpack_from('<I', self.good, 8)[0]
        original = json.loads(self.good[12:12 + length])
        cases = (
            ("not a JSON object", [0, 0, 1], 'JSON object'),
            ("boolean format version", dict(original, format_version=[False, False, True]),
             'version or codec'),
            ("floating format version", dict(original, format_version=[0.0, 0.0, 1.0]),
             'version or codec'),
            ("floating block record cap", dict(original, block_records_max=512.0),
             'version or codec'),
            ("floating section counts", dict(original, sections={
                name: float(count) for name, count in original['sections'].items()
            }), 'section record totals'),
            ("missing sections", {key: value for key, value in original.items() if key != 'sections'},
             'section record totals'),
        )
        for label, document, message in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(ValueError, message):
                    self.audit(rewrite_header(self.good, document))

        # With a one-record archive, True == 1 would have been another false
        # positive before the strict section-total type validation.
        tiny = self.build_semantic_fixture([
            ('ingress', 'Moon', 0, 0, 1, struct.pack('<q', 0) + bytes([4, 0, 0])),
        ])
        tiny_length = struct.unpack_from('<I', tiny, 8)[0]
        tiny_header = json.loads(tiny[12:12 + tiny_length])
        tiny_header['sections']['ingress'] = True
        with self.assertRaisesRegex(ValueError, 'section record totals'):
            self.audit(rewrite_header(tiny, tiny_header))

        # A checksum-valid header may still contain body names that the
        # Rust BodyId parser refuses; the independent auditor must agree.
        for bad_body in ("", " Moon", "Moon ", "Moon\t", "Moon\x00",
                         "Moon\x7f", "Moon\u009f", "Moon\ud800"):
            with self.subTest(invalid_body=repr(bad_body)):
                header = dict(original)
                header["table_of_contents"] = [
                    dict(item) for item in original["table_of_contents"]
                ]
                header["table_of_contents"][0]["body"] = bad_body
                with self.assertRaisesRegex(ValueError, "invalid TOC family or body"):
                    self.audit(rewrite_header(self.good, header))

        # serde_json requires UTF-8 and rejects NaN/Infinity, including in
        # otherwise-unused metadata. Keep payloads and block SHA intact so
        # only the header JSON parser can account for rejection.
        canonical_header = json.dumps(
            original, sort_keys=True, separators=(',', ':')
        ).encode('utf-8')
        original_length = struct.unpack_from('<I', self.good, 8)[0]
        original_payload = self.good[12 + original_length:]

        def replace_header(header_bytes):
            return (
                b'VSEPH-P0' + struct.pack('<I', len(header_bytes))
                + header_bytes + original_payload
            )

        for token in (b'NaN', b'Infinity', b'-Infinity', b'1e400', b'-1e400'):
            with self.subTest(nonfinite=token):
                malformed_header = (
                    canonical_header[:-1] + b',"_unused_number":'
                    + token + b'}'
                )
                with self.assertRaisesRegex(ValueError, 'non-finite P0 JSON'):
                    self.audit(replace_header(malformed_header))

        for encoding in ('utf-16-le', 'utf-16-be', 'utf-32-le', 'utf-32-be'):
            with self.subTest(non_utf8=encoding):
                malformed_header = canonical_header.decode('utf-8').encode(
                    encoding
                )
                with self.assertRaisesRegex(ValueError, 'UTF-8 JSON'):
                    self.audit(replace_header(malformed_header))

        # Rust rejects noncanonical SHA-256 TOC fields before reading blocks.
        # The compressed bytes and their actual uncompressed digests are
        # untouched, so malformed metadata must fail even if payloads match.
        for bad_digest in (
            "A" * 64, "g" * 64, "0" * 63, "0" * 65,
            "0" * 63 + " ", None, 0, True,
        ):
            with self.subTest(invalid_sha256=repr(bad_digest)):
                header = dict(original)
                header["table_of_contents"] = [
                    dict(item) for item in original["table_of_contents"]
                ]
                header["table_of_contents"][0]["raw_sha256"] = bad_digest
                with self.assertRaisesRegex(
                    ValueError, "invalid P0 block SHA-256 digest"
                ):
                    self.audit(rewrite_header(self.good, header))

        with self.subTest(missing_sha256=True):
            header = dict(original)
            header["table_of_contents"] = [
                dict(item) for item in original["table_of_contents"]
            ]
            del header["table_of_contents"][0]["raw_sha256"]
            with self.assertRaisesRegex(
                ValueError, "invalid P0 block SHA-256 digest"
            ):
                self.audit(rewrite_header(self.good, header))

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

    def test_unsigned_leb128_is_shortest_and_bounded(self):
        for encoded, expected in (
            (b"\x00", 0), (b"\x7f", 127),
            (b"\x80\x01", 128),
            (b"\xff\x7f", 16_383),
            (b"\xff" * 9 + b"\x01", (1 << 64) - 1),
        ):
            with self.subTest(encoded=encoded):
                self.assertEqual(_varint(encoded, 0), (expected, len(encoded)))
        for malformed in (
            b"\x80\x00", b"\x81\x00",
            b"\xff\x00", b"\x80\x80\x00",
        ):
            with self.subTest(malformed=malformed):
                with self.assertRaisesRegex(ValueError, "non-canonical unsigned varint"):
                    _varint(malformed, 0)
        for malformed in (b"\x80", b"\x80\x80", b"\xff" * 9 + b"\x02"):
            with self.subTest(malformed=malformed):
                with self.assertRaises(ValueError):
                    _varint(malformed, 0)

    def test_noncanonical_varints_are_rejected_with_valid_block_checksum(self):
        # Construct complete, self-consistent P0 files with an authentic
        # SHA-256 over their malformed uncompressed blocks. An integrity
        # checksum failure must not mask the canonical varint rule.
        cases = (
            ("ingress ID", "ingress", b"\x84\x00\x00\x00"),
            ("ingress delta", "ingress", b"\x04\x80\x00\x00"),
            ("synod phase count", "synod", b"\x04\x00\x81\x00"),
            ("synod phase offset", "synod", b"\x04\x00\x02\x81\x00"),
        )
        for label, family, payload in cases:
            with self.subTest(label=label):
                raw = struct.pack("<q", 0) + payload
                archive = self.build_semantic_fixture([
                    (family, "Moon", 0, 0, 1, raw),
                ])
                with self.assertRaisesRegex(ValueError, "non-canonical unsigned varint"):
                    self.audit(archive)
        # Control: the same source event encoded canonically remains readable.
        valid_raw = struct.pack("<q", 0) + b"\x04\x00\x00"
        archive = self.build_semantic_fixture([
            ("ingress", "Moon", 0, 0, 1, valid_raw),
        ])
        self.assertEqual(self.audit(archive)["records"], 1)

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
