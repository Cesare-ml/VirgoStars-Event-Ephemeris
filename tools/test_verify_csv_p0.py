# Copyright (c) 2026 Cesare-ml. CPAL-1.0; see LICENSE/NOTICE.
# Powered by VirgoStars — https://www.virgostars.it/
"""Synthetic normalized CSV ↔ P0 parity and adversarial edits (no real dataset)."""
from __future__ import annotations
import csv
import datetime as dt
import hashlib
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from verify_csv_p0 import decoded_block, utc_microseconds, verify, uvar

DAY_US=86_400_000_000
EPOCH=dt.date(1970,1,1)


def iso(us):
    days,remainder=divmod(us,DAY_US)
    day=EPOCH+dt.timedelta(days=days)
    hh,remainder=divmod(remainder,3_600_000_000)
    mm,remainder=divmod(remainder,60_000_000)
    ss,micro=divmod(remainder,1_000_000)
    return f"{day.year:04d}-{day.month:02d}-{day.day:02d} {hh:02d}:{mm:02d}:{ss:02d}"+(f".{micro:06d}" if micro else "")+"+00"


def uvar(n):
    result=bytearray()
    while n>127:
        result.append((n&127)|128);n>>=7
    result.append(n)
    return bytes(result)


class CsvP0ParityTest(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        self.folder=Path(temp.name)
        self.rows={
            "synod":{"Moon":[(10,-876544,None),(12,1500000,[1700000,2000000,2300000])],"Mercury":[(21,2000000,[])]},
            "motion":{"Mars":[(31,0,4,"r"),(32,2200000,3,"d")]},
            "ingress":{"Moon":[(41,-876544,0),(42,3000000,1),(43,5000000,3)]},
        }
        self.archive=self.folder/"synthetic.vseph-p0"
        self.write_public_csv()
        self.build_p0()

    def write_public_csv(self):
        with (self.folder/"sinodi.csv").open("w",encoding="utf-8",newline="") as f:
            w=csv.writer(f);w.writerow(["id","elemento","dataora_utc","dataora_extra_utc_json"])
            for body,records in self.rows["synod"].items():
                for ident,at,extras in records:
                    w.writerow([ident,body,iso(at),"" if extras is None else json.dumps([iso(x) for x in extras])])
        with (self.folder/"moto_pianeti.csv").open("w",encoding="utf-8",newline="") as f:
            w=csv.writer(f);w.writerow(["id","moto","segno","dataora_utc","elemento_celeste"])
            for body,records in self.rows["motion"].items():
                for ident,at,sign,direction in records:w.writerow([ident,direction,sign,iso(at),body])
        with (self.folder/"ingresso_segni.csv").open("w",encoding="utf-8",newline="") as f:
            w=csv.writer(f);w.writerow(["id","segno","dataora_utc","elemento_celeste"])
            for body,records in self.rows["ingress"].items():
                for ident,at,sign in records:w.writerow([ident,sign,iso(at),body])

    def build_p0(self):
        chunks=[];toc=[];offset=0;sections={}
        for family,bodies in sorted(self.rows.items()):
            sections[family]=0
            for body,events in sorted(bodies.items()):
                binary=bytearray(struct.pack('<q',events[0][1]));last=events[0][1]
                for item in events:
                    ident,at,*extra=item
                    binary+=uvar(ident)+uvar(at-last)
                    if family=='synod':
                        phases=extra[0]
                        binary+=uvar(0 if phases is None else len(phases)+1)
                        last_phase=at
                        for phase in (phases or []):
                            binary+=uvar(phase-last_phase);last_phase=phase
                    elif family=='motion':
                        sign,direction=extra
                        binary+=bytes([sign,1 if direction=='r' else 0])
                    else:binary+=bytes([extra[0]])
                    last=at
                raw=bytes(binary);compressed=zlib.compress(raw,9)
                toc.append(dict(body=body,family=family,min_us=str(events[0][1]),max_us=str(events[-1][1]),count=len(events),start=offset,length=len(compressed),raw_length=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest()))
                chunks.append(compressed);offset+=len(compressed);sections[family]+=len(events)
        header={'format':'VSEPH-P0-EXPERIMENTAL','format_version':[0,0,1],'codec':'zlib','block_records_max':512,'sections':sections,'table_of_contents':toc}
        header_bytes=json.dumps(header,sort_keys=True,separators=(',',':')).encode()
        self.archive.write_bytes(b'VSEPH-P0'+struct.pack('<I',len(header_bytes))+header_bytes+b''.join(chunks))

    def edit_csv(self,filename,old,new):
        path=self.folder/filename
        text=path.read_text(encoding='utf-8')
        self.assertIn(old,text)
        path.write_text(text.replace(old,new,1),encoding='utf-8')

    def test_exact_scientific_parity(self):
        report=verify(self.folder,self.archive,hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.assertEqual(report['records_compared'],8)
        self.assertEqual(report['sections'],{'ingress':3,'motion':2,'synod':3})
        self.assertTrue(report['archive_sha256_checked'])
        self.assertFalse(report['coverage_certified'])

    def test_pre_epoch_and_early_year_without_float_conversion(self):
        self.assertEqual(utc_microseconds(iso(-876544)),-876544)
        self.assertEqual(utc_microseconds('0002-01-30 05:04:51+00'),
                         (dt.date(2,1,30)-EPOCH).days*DAY_US+(5*3600+4*60+51)*1000000)

    def test_varint_canonical_encoding_matches_rust(self):
        # Rust's take_varint rejects overlong LEB128 spellings, even when
        # their numeric value equals a valid source ID or timestamp delta.
        for encoded, number in (
            (b"\x00", 0),
            (b"\x01", 1),
            (b"\x7f", 127),
            (b"\x80\x01", 128),
            (b"\xff" * 9 + b"\x01", (1 << 64) - 1),
        ):
            with self.subTest(valid=encoded):
                self.assertEqual(uvar(encoded, 0), (number, len(encoded)))
        for encoded in (
            b"\x80\x00", b"\x81\x00", b"\xff\x00",
            b"\x80\x80\x00", b"\x80" * 9 + b"\x00",
            b"\x80", b"\xff" * 9 + b"\x02",
        ):
            with self.subTest(invalid=encoded):
                with self.assertRaisesRegex(
                    ValueError, "non-canonical|truncated|unsigned 64-bit"
                ):
                    uvar(encoded, 0)

    def test_csv_timestamps_require_ascii_decimal_digits(self):
        # Python \d and int() accept Unicode numerals, but a normalized
        # scientific UTC CSV timestamp must use unambiguous ASCII digits.
        self.assertEqual(
            utc_microseconds("0002-01-30 05:04:51+00"),
            (dt.date(2, 1, 30) - EPOCH).days * DAY_US
            + (5 * 3600 + 4 * 60 + 51) * 1_000_000,
        )
        for value in (
            "٢٠٢٦-11-14 12:30:01+00",
            "２０２６-11-14 12:30:01+00",
            "2026-١١-14 12:30:01+00",
            "2026-11-14 12:30:٠١+00",
        ):
            with self.subTest(timestamp=value), self.assertRaisesRegex(
                ValueError, "invalid UTC timestamp"
            ):
                utc_microseconds(value)

    def test_decoded_p0_block_requires_unique_nonzero_source_ids(self):
        # Call the independent block decoder directly: checksum validation
        # and SQLite parity must not be relied upon for these invariants.
        first = struct.pack("<q", 0)
        for payload, count in (
            (first + b"\x00\x00\x00", 1),  # ID 0
            (first + b"\x07\x00\x00\x07\x01\x00", 2),  # repeated ID 7
        ):
            with self.subTest(payload=payload), self.assertRaisesRegex(
                ValueError, "zero or duplicate P0 source ID"
            ):
                list(decoded_block("synod", payload, count))

        valid = first + b"\x07\x00\x00\x08\x01\x00"
        self.assertEqual(
            list(decoded_block("synod", valid, 2)),
            [(7, [0, None]), (8, [1, None])],
        )

    def test_rejects_changed_ingress_sign(self):
        path=self.folder/'ingresso_segni.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        data[-1][1]='8'
        with path.open('w',encoding='utf-8',newline='') as f:csv.writer(f).writerows(data)
        with self.assertRaisesRegex(ValueError,'scientific field mismatch'):verify(self.folder,self.archive)

    def test_rejects_changed_motion_direction(self):
        path=self.folder/'moto_pianeti.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        data[-1][1]='r'
        with path.open('w',encoding='utf-8',newline='') as f:csv.writer(f).writerows(data)
        with self.assertRaisesRegex(ValueError,'scientific field mismatch'):verify(self.folder,self.archive)

    def test_null_synod_extras_are_not_empty_json_list(self):
        path=self.folder/'sinodi.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        self.assertEqual(data[1][3],'')
        data[1][3]='[]'
        with path.open('w',encoding='utf-8',newline='') as f:csv.writer(f).writerows(data)
        with self.assertRaisesRegex(ValueError,'scientific field mismatch'):verify(self.folder,self.archive)

    def test_missing_csv_row_is_rejected(self):
        path=self.folder/'sinodi.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        with path.open('w',encoding='utf-8',newline='') as f:csv.writer(f).writerows(data[:-1])
        with self.assertRaisesRegex(ValueError,'binary event absent from CSV'):verify(self.folder,self.archive)

    def test_duplicate_source_id_is_rejected(self):
        path=self.folder/'ingresso_segni.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        data[-1][0]=data[1][0]
        with path.open('w',encoding='utf-8',newline='') as f:csv.writer(f).writerows(data)
        with self.assertRaisesRegex(ValueError,'UNIQUE constraint failed'):verify(self.folder,self.archive)

    def test_corrupted_archive_block_is_rejected(self):
        b=bytearray(self.archive.read_bytes());b[-1]^=1;self.archive.write_bytes(b)
        with self.assertRaises(Exception):verify(self.folder,self.archive)

    def test_wrong_trusted_sha_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'(?i)sha'):verify(self.folder,self.archive,'0'*64)

    def test_changed_synod_phase_is_rejected(self):
        path=self.folder/'sinodi.csv'
        with path.open(encoding='utf-8',newline='') as source:
            data=list(csv.reader(source))
        original=json.loads(data[2][3])
        original[0]=iso(1750000)
        data[2][3]=json.dumps(original)
        with path.open('w',encoding='utf-8',newline='') as destination:csv.writer(destination).writerows(data)
        with self.assertRaisesRegex(ValueError,'scientific field mismatch'):verify(self.folder,self.archive)

    def test_wrong_csv_header_is_rejected(self):
        self.edit_csv('sinodi.csv','dataora_extra_utc_json','wrong_column')
        with self.assertRaisesRegex(ValueError,'columns'):verify(self.folder,self.archive)

if __name__=='__main__':unittest.main(verbosity=2)
