"""Runnable self-tests on synthetic data only; no private CSV fixture embedded."""
from __future__ import annotations
import tempfile
import unittest
from pathlib import Path
from virgostars_vseph_prototype import (
    MAGIC, pack_dataset, unpack_dataset, digest_records, to_us, us_to_iso,
    verify_against_sources
)
from virgostars_vseph_reader import VSEPHReader

class TestPrototype(unittest.TestCase):
    @staticmethod
    def sample():
        return {
            'synod': {
                'Moon': [(3,-876544, (10,20,30)),(8,100,None),(9,300,())],
                'Mercury': [(13,200,(210,220,230))]
            },
            'motion': {
                'Mars': [(10,100,4,'r'),(12,200,4,'d')],
            },
            'ingress': {
                'Mercury': [(1,0,0),(2,10,11),(3,20,0)],
            },
        }

    def build(self,dir):
        path=Path(dir)/'synthetic.vseph-p0'
        pack_dataset(self.sample(),{},path)
        return path

    def test_round_trip_all_fields(self):
        with tempfile.TemporaryDirectory() as d:
            path=self.build(d)
            decoded,header=unpack_dataset(path)
            self.assertEqual(decoded,self.sample())
            self.assertEqual(digest_records(decoded),digest_records(self.sample()))
            self.assertEqual(header['format'],'VSEPH-P0-EXPERIMENTAL')

    def test_navigation_boundaries(self):
        with tempfile.TemporaryDirectory() as d:
            reader=VSEPHReader(self.build(d))
            self.assertIsNone(reader.at('ingress','Mercury',-1))
            self.assertEqual(reader.at('ingress','Mercury',10),(2,10,11))
            self.assertEqual(reader.previous('ingress','Mercury',10),(1,0,0))
            self.assertEqual(reader.next('ingress','Mercury',10),(3,20,0))
            self.assertIsNone(reader.next('ingress','Mercury',20))
            self.assertEqual(reader.between('ingress','Mercury',0,20),[(1,0,0),(2,10,11)])
            self.assertEqual(reader.sign_state('Mercury',19)['sign'],11)
            self.assertEqual(reader.motion_state('Mars',199)['direction'],'retrograde')
            self.assertEqual(reader.motion_state('Mars',200)['direction'],'direct')

    def test_synod_handle_navigation_and_phases(self):
        with tempfile.TemporaryDirectory() as d:
            reader=VSEPHReader(self.build(d))
            synod=reader.synod('Moon',0)
            self.assertEqual(synod.at_us,-876544)
            self.assertEqual(synod.phase_index_at(10),1)
            self.assertEqual(synod.phase_index_at(30),3)
            self.assertEqual(synod.next().at_us,100)
            self.assertEqual(synod.next().previous().at_us,-876544)
            self.assertIsNone(synod.next().phase_index_at(110))
            self.assertEqual(synod.next().next().phases_us,())

    def test_corrupted_block_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=self.build(d)
            original=p.read_bytes()
            p.write_bytes(original[:-1]+bytes([original[-1]^0x80]))
            with self.assertRaisesRegex(ValueError,'|'.join(['block', 'compressed','stream'])):
                unpack_dataset(p)

    def test_truncated_and_invalid_magic(self):
        with tempfile.TemporaryDirectory() as d:
            p=self.build(d)
            p.write_bytes(b'BADMAGIC')
            with self.assertRaises(ValueError):VSEPHReader(p)
            with self.assertRaises(ValueError):unpack_dataset(p)

    def test_instant_round_trip(self):
        self.assertEqual(to_us('1969-12-31 23:59:59.123456',False),-876544)
        self.assertEqual(us_to_iso(-876544,False),'1969-12-31 23:59:59.123456')
        self.assertEqual(to_us('0002-01-16 05:59:33+00',True),to_us(us_to_iso(to_us('0002-01-16 05:59:33+00',True),True),True))
        self.assertEqual(to_us('4999-12-20 07:33:49',False),to_us(us_to_iso(to_us('4999-12-20 07:33:49',False)),False))

if __name__ == '__main__':
    unittest.main(verbosity=2)
