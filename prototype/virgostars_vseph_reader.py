#!/usr/bin/env python3
"""VSEPH-P0 *experimental* read-only indexed query interface.

Prototype for working with private exported data, not a stable Rust/JS/Python
API. It validates requested blocks and does not recompute astronomy.
Copyright (c) 2026 Cesare-ml (VirgoStars project). CPAL-1.0.
Powered by VirgoStars: https://www.virgostars.it/
"""
from __future__ import annotations
import bisect
import functools
import hashlib
import json
import struct
from collections import defaultdict
from pathlib import Path
from virgostars_vseph_prototype import (
    MAGIC, MAX_HEADER_SIZE, MAX_COMPRESSED_BLOCK, decode_block,
    safe_decompress, to_us, us_to_iso
)

class VSEPHReader:
    def __init__(self, file: str | Path):
        self.path=Path(file)
        with self.path.open('rb') as f:
            head=f.read(12)
            if len(head)!=12 or head[:8]!=MAGIC:raise ValueError('not VSEPH-P0')
            size=struct.unpack_from('<I',head,8)[0]
            if size>MAX_HEADER_SIZE:raise ValueError('oversized header')
            data=f.read(size)
            if len(data)!=size:raise ValueError('truncated header')
        self.header=json.loads(data)
        if self.header.get('format')!='VSEPH-P0-EXPERIMENTAL' or self.header.get('format_version')!=[0,0,1]:
            raise ValueError('unsupported format')
        self.payload_start=12+size
        self._groups=defaultdict(list)
        self._starts={}
        expected_offset=0
        for entry in self.header['table_of_contents']:
            family,body=entry['family'],entry['body']
            if family not in ('synod','motion','ingress') or not body:raise ValueError('unknown TOC family/body')
            if entry['start']!=expected_offset or not 0<entry['length']<=MAX_COMPRESSED_BLOCK:
                raise ValueError('noncontiguous or invalid TOC')
            expected_offset+=entry['length']
            self._groups[family,body].append(entry)
        if self.path.stat().st_size != self.payload_start+expected_offset:raise ValueError('TOC/file length mismatch')
        for key,entries in self._groups.items():
            starts=[int(e['min_us']) for e in entries]
            ends=[int(e['max_us']) for e in entries]
            if starts!=sorted(starts) or any(a>=b for a,b in zip(ends,starts[1:])):
                raise ValueError('unsorted/overlapping block index for '+str(key))
            self._starts[key]=starts

    @functools.lru_cache(maxsize=24)
    def _block(self, family, body, index):
        entry=self._groups[family,body][index]
        with self.path.open('rb') as f:
            f.seek(self.payload_start+entry['start']);compressed=f.read(entry['length'])
        if len(compressed)!=entry['length']:raise ValueError('truncated block')
        raw=safe_decompress(compressed,entry['raw_length'])
        if hashlib.sha256(raw).hexdigest()!=entry['raw_sha256']:raise ValueError('SHA256 block mismatch')
        records=decode_block(family,raw,entry['count'])
        if not records or records[0][1]!=int(entry['min_us']) or records[-1][1]!=int(entry['max_us']):
            raise ValueError('block time index mismatch')
        return records

    @staticmethod
    def _time(instant):
        if isinstance(instant,int):return instant
        if isinstance(instant,str):
            if instant.endswith('Z'):return to_us(instant[:-1].replace('T',' '),False)
            if instant.endswith('+00'):return to_us(instant.replace('T',' '),True)
        raise ValueError('Pass UTC timestamp int micros or ISO string ending Z / +00')

    def at(self,family,body,instant):
        """Most recent event at or before t, or None (coverage is not guaranteed)."""
        key=(family,body);starts=self._starts.get(key)
        if not starts:return None
        t=self._time(instant)
        i=bisect.bisect_right(starts,t)-1
        if i<0:return None
        chunk=self._block(family,body,i)
        j=bisect.bisect_right([x[1] for x in chunk],t)-1
        return chunk[j] if j>=0 else None

    def next(self,family,body,instant):
        """First event strictly after time t; None if known indexed file has no later event."""
        key=(family,body);starts=self._starts.get(key)
        if not starts:return None
        t=self._time(instant)
        i=max(0,bisect.bisect_right(starts,t)-1)
        for index in range(i,len(starts)):
            chunk=self._block(family,body,index)
            j=bisect.bisect_right([x[1] for x in chunk],t)
            if j<len(chunk):return chunk[j]
        return None

    def previous(self,family,body,instant):
        """Most recent event strictly earlier than t."""
        key=(family,body);starts=self._starts.get(key)
        if not starts:return None
        t=self._time(instant)
        i=bisect.bisect_left(starts,t)-1
        # If t is after a block's start, that block must be searched first.
        if i+1<len(starts) and starts[i+1]<t:i+=1
        # Equivalent to bisect_left(starts,t)-1; if t>first min, current block's start <t.
        for index in range(i,-1,-1):
            chunk=self._block(family,body,index)
            j=bisect.bisect_left([x[1] for x in chunk],t)-1
            if j>=0:return chunk[j]
        return None

    def between(self,family,body,start,end):
        """[start,end) events for one family+body, fully sorted and checked."""
        a=self._time(start);b=self._time(end)
        if b<a:raise ValueError('end before start')
        if b==a:return []
        key=(family,body);starts=self._starts.get(key)
        if not starts:return []
        first=max(0,bisect.bisect_right(starts,a)-1)
        result=[]
        for index in range(first,len(starts)):
            if starts[index]>=b:break
            result.extend(rec for rec in self._block(family,body,index) if a<=rec[1]<b)
        return result

    def synod(self,body,at):
        rec=self.at('synod',body,at)
        return SynodHandle(self,body,rec) if rec is not None else None

    def motion_state(self,body,at):
        """Returns latest recorded direction; does NOT promise complete coverage."""
        rec=self.at('motion',body,at)
        return None if rec is None else {'source_id':rec[0],'at_us':rec[1],
                                        'direction':'retrograde' if rec[3]=='r' else 'direct','sign':rec[2]}

    def sign_state(self,body,at):
        """Latest *recorded* ingress; may be misleading across missing data ranges."""
        rec=self.at('ingress',body,at)
        return None if rec is None else {'source_id':rec[0],'at_us':rec[1],'sign':rec[2]}

class SynodHandle:
    def __init__(self,reader,body,record):self._reader=reader;self.body=body;self.record=record
    @property
    def at_us(self):return self.record[1]
    @property
    def phases_us(self):return self.record[2]
    def next(self):
        next_record=self._reader.next('synod',self.body,self.at_us)
        return SynodHandle(self._reader,self.body,next_record) if next_record is not None else None
    def previous(self):
        previous_record=self._reader.previous('synod',self.body,self.at_us)
        return SynodHandle(self._reader,self.body,previous_record) if previous_record is not None else None
    def phase_index_at(self,t):
        """Number of recorded phase thresholds crossed (no assumed astrological label)."""
        instant=self._reader._time(t)
        if instant<self.at_us or self.phases_us is None:return None
        next_event=self.next()
        if next_event and instant>=next_event.at_us:return None
        return bisect.bisect_right(self.phases_us,instant)

def main():
    import argparse
    ap=argparse.ArgumentParser(description='Indexed read-only VSEPH-P0 prototype')
    ap.add_argument('binary',type=Path)
    ap.add_argument('family',choices=['synod','motion','ingress'])
    ap.add_argument('body')
    ap.add_argument('date_utc',help='YYYY-MM-DDTHH:MM:SSZ or UTC +00')
    args=ap.parse_args()
    ep=VSEPHReader(args.binary)
    def fmt(rec):
        if rec is None:return None
        values={'source_id':rec[0],'at_utc':us_to_iso(rec[1],True)}
        if args.family=='synod':values['phases_utc']=None if rec[2] is None else [us_to_iso(x,True) for x in rec[2]]
        elif args.family=='motion':values.update(sign=rec[2],motion=rec[3])
        else:values['sign']=rec[2]
        return values
    print(json.dumps({'at':fmt(ep.at(args.family,args.body,args.date_utc)),
                      'next':fmt(ep.next(args.family,args.body,args.date_utc)),
                      'previous':fmt(ep.previous(args.family,args.body,args.date_utc))},indent=2))

if __name__=='__main__':main()
