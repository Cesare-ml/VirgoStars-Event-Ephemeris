#!/usr/bin/env python3
"""VirgoStars Event Ephemeris: private-data VSEPH-P0 prototype packer / verifier.

This is NOT the VSEPH v1 specification or the finished Rust library. It is a
fully reversible, deterministic proof-of-concept for the scientific fields in
three pgAdmin4 CSV exports. Does not transmit data, connect to PostgreSQL,
or publish release assets.

Source licensing for this tool: CPAL-1.0, with VirgoStars attribution.
Copyright (c) 2026 Cesare-ml (VirgoStars project). Powered by VirgoStars.
https://www.virgostars.it/ — see repo LICENSE, Exhibit A and B.
"""
from __future__ import annotations
import argparse
import collections
import csv
import datetime as dt
import hashlib
import json
import re
import struct
import sys
import zlib
from pathlib import Path

MAGIC = b'VSEPH-P0'  # 8 bytes, explicitly NOT a VSEPH v1 release
MAX_HEADER_SIZE = 5_000_000
MAX_COMPRESSED_BLOCK = 5_000_000
MAX_BLOCK_UNCOMPRESSED = 25_000_000
MAX_RECORDS_PER_BLOCK = 512
EPOCH_ORDINAL = dt.date(1970, 1, 1).toordinal()
US_DAY = 86400 * 1_000_000
REGEX_TS = re.compile(r'^(\d{4})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)(?:\.(\d{1,6}))?(\+00)?$')
REGEX_EXTRAS = re.compile(r'\d{4}-\d\d-\d\d \d\d:\d\d:\d\d(?:\.\d{1,6})?\+00')
FAMILIES = {'sinodi':'synod','moto_pianeti':'motion','ingresso_segni':'ingress'}
def discover_csv(folder:Path,table:str) -> Path:
    # The filename date is not part of the data format; accept any dated pgAdmin export.
    choices=sorted(folder.glob('*_'+table+'.csv'))
    if len(choices)!=1:
        raise ValueError(f'Expected exactly one CSV named *_{table}.csv in {folder}; found {len(choices)}')
    return choices[0]


def to_us(value: str, with_offset: bool) -> int:
    match = REGEX_TS.fullmatch(value)
    if not match or (match.group(8) is not None) != with_offset:
        raise ValueError(f'Expected PostgreSQL date with UTC={with_offset}, got: {value!r}')
    year, month, day, hour, minute, second = map(int, match.groups()[:6])
    micro = int((match.group(7) or '').ljust(6,'0'))
    d = dt.date(year, month, day)  # explicit validation including historical leap years
    if hour >=24 or minute>=60 or second>=60: raise ValueError('Invalid civil time '+value)
    us = ((d.toordinal()-EPOCH_ORDINAL)*US_DAY
          + ((hour*60+minute)*60+second)*1_000_000 + micro)
    if not (-2**63 <= us < 2**63): raise ValueError('Outside i64 timestamp range')
    return us


def us_to_iso(us:int, offset=False) -> str:
    days, left = divmod(us,US_DAY)
    d=dt.date.fromordinal(EPOCH_ORDINAL + days)
    hh,left=divmod(left,3600*1_000_000)
    mm,left=divmod(left,60*1_000_000)
    ss,micro=divmod(left,1_000_000)
    suffix = f'.{micro:06d}' if micro else ''
    return f'{d.year:04d}-{d.month:02d}-{d.day:02d} {hh:02d}:{mm:02d}:{ss:02d}{suffix}'+('+00' if offset else '')


def read_inputs(folder:Path):
    by_family={}
    audit={'row_counts':{},'by_body':{},'source_columns_excluded':{},'issues':[], 'source_csv_sha256':{}, 'timestamp_semantics':{
        'synod':'TIMESTAMPTZ, explicit +00 in input',
        'motion':'TIMESTAMP WITHOUT TIME ZONE; assumed UTC by app convention, needs owner confirmation',
        'ingress':'TIMESTAMP WITHOUT TIME ZONE; assumed UTC by app convention, needs owner confirmation'}}
    for tag,family in FAMILIES.items():
        filename = discover_csv(folder,tag)
        with filename.open('rb') as raw_source:
            audit['source_csv_sha256'][tag] = hashlib.file_digest(raw_source,'sha256').hexdigest()
        rows=[];excluded=collections.Counter();seen=set();bodies=collections.defaultdict(list)
        with filename.open(encoding='utf-8-sig',newline='') as stream:
            for n,row in enumerate(csv.reader(stream),1):
                try:
                    if tag=='sinodi':
                        if len(row) not in (4,6):raise ValueError(f'Expected 4/6 columns, got {len(row)}')
                        event_id=int(row[0]);body=row[1];instant=to_us(row[2],True)
                        if len(row)==4:
                            if row[3]!='':raise ValueError('Nonempty 4-column synod extras: '+repr(row[3][:100]))
                            extras=None
                        else:
                            # pgAdmin CSV exported PostgreSQL array is internally quoted in a
                            # nonstandard way; scientific content has exactly 3 timestamp tokens.
                            extra_text=','.join(row[3:]);tokens=REGEX_EXTRAS.findall(extra_text)
                            if len(tokens)!=3:raise ValueError('Expected three phase timestamps; found '+str(len(tokens)))
                            extras=tuple(to_us(x,True) for x in tokens)
                            # Reject unrecognized characters instead of silently dropping fields.
                            remainder=REGEX_EXTRAS.sub('',extra_text)
                            if any(ch not in '\"\',{} ' for ch in remainder):raise ValueError('Unexpected characters in PostgreSQL array: '+repr(remainder[:100]))
                            if not (instant < extras[0] < extras[1] < extras[2]):
                                raise ValueError('Invalid phase ordering')
                        rec=(event_id,instant,extras)
                    elif tag=='moto_pianeti':
                        if len(row)!=5:raise ValueError('Expected 5 columns')
                        event_id=int(row[0]);motion=row[1];sign=int(row[2]);instant=to_us(row[3],False);body=row[4]
                        if motion not in ('r','d'):raise ValueError('Unexpected motion code')
                        if not 0<=sign<=11:raise ValueError('Sign outside 0..11')
                        rec=(event_id,instant,sign,motion)
                    else:
                        if len(row)!=6:raise ValueError('Expected 6 columns')
                        event_id=int(row[0]);sign=int(row[1]);instant=to_us(row[2],False);body=row[3]
                        if not 0<=sign<=11:raise ValueError('Sign outside 0..11')
                        excluded['interpretazione' if row[4] else 'interpretazione_empty']+=1
                        excluded['messaggio_id' if row[5] else 'messaggio_id_empty']+=1
                        rec=(event_id,instant,sign)
                    if not body:raise ValueError('Missing body')
                    k=(body,instant)
                    if k in seen:raise ValueError('Duplicate body+timestamp')
                    seen.add(k)
                    bodies[body].append(rec)
                except Exception as err:
                    raise ValueError(f'{tag} CSV row #{n}: {err}') from err
        body_report={}
        for body,items in bodies.items():
            items.sort(key=lambda r:(r[1],r[0]))
            body_report[body]={'count':len(items),'min_utc':us_to_iso(items[0][1],True),
                               'max_utc':us_to_iso(items[-1][1],True)}
        audit['row_counts'][tag]=sum(x['count'] for x in body_report.values())
        audit['by_body'][tag]=dict(sorted(body_report.items()))
        audit['source_columns_excluded'][tag]=dict(sorted(excluded.items()))
        by_family[family]=bodies
    moon=by_family['ingress'].get('Moon',[])
    future_moon=[r for r in moon if dt.date.fromordinal(EPOCH_ORDINAL+r[1]//US_DAY).year>=2100]
    if future_moon:
        years=[dt.date.fromordinal(EPOCH_ORDINAL+r[1]//US_DAY).year for r in future_moon]
        audit['issues'].append({
            'severity':'review_needed',
            'kind':'disconnected_lunar_ingress_coverage',
            'description':'Moon ingresses after year 2099 are only in zodiac sign 0; there is a multi-century gap. Do not infer continuous signAt coverage.',
            'future_rows':len(future_moon),
            'year_start':min(years), 'year_end':max(years),
            'signs':sorted({r[2] for r in future_moon})
        })
    return by_family,audit


def uvar(n:int)->bytes:
    if n<0:raise ValueError('negative uvar')
    out=bytearray()
    while n>=128:
        out.append((n&127)|128);n>>=7
    out.append(n)
    return bytes(out)


def take_uvar(buf:memoryview,pos:int)->tuple[int,int]:
    acc=0
    for step in range(10):
        if pos>=len(buf):raise ValueError('truncated varint')
        byte=buf[pos];pos+=1
        if step==9 and byte>1:raise ValueError('varint overflow')
        acc|=(byte&127)<<(step*7)
        if byte<128:return acc,pos
    raise ValueError('varint exceeds 10 bytes')


def encode_block(family,records):
    if not records or len(records)>MAX_RECORDS_PER_BLOCK:raise ValueError('Invalid block size')
    buf=bytearray()
    buf.extend(struct.pack('<q',records[0][1]))
    prev=records[0][1]
    for i,rec in enumerate(records):
        record_id,when=rec[:2]
        if record_id<0:raise ValueError('negative database ID')
        delta=when-prev
        if delta<0:raise ValueError('unsorted block')
        buf.extend(uvar(record_id));buf.extend(uvar(delta));prev=when
        if family=='synod':
            extras=rec[2]
            if extras is None:buf.extend(uvar(0))
            else:
                buf.extend(uvar(len(extras)+1));last=when
                for when_phase in extras:
                    if when_phase<=last:raise ValueError('non-increasing phases')
                    buf.extend(uvar(when_phase-last));last=when_phase
        elif family=='motion':
            buf.append(rec[2]);buf.append(1 if rec[3]=='r' else 0)
        elif family=='ingress':buf.append(rec[2])
        else:raise ValueError('unknown family')
    return bytes(buf)


def decode_block(family,plain,count):
    buf=memoryview(plain)
    if len(buf)<8 or count<1 or count>MAX_RECORDS_PER_BLOCK:raise ValueError('invalid encoded block')
    base=struct.unpack_from('<q',buf,0)[0];pos=8;prev=base;records=[]
    for i in range(count):
        ident,pos=take_uvar(buf,pos);delta,pos=take_uvar(buf,pos)
        when=prev+delta
        if when>=2**63 or when<-(2**63):raise ValueError('decoded time overflow')
        prev=when
        if family=='synod':
            phase_count_code,pos=take_uvar(buf,pos)
            if phase_count_code>1024:raise ValueError('phase-count excess')
            if phase_count_code==0:extras=None
            else:
                tmp=[];phase_when=when
                for _ in range(phase_count_code-1):
                    delta_phase,pos=take_uvar(buf,pos)
                    if delta_phase<=0:raise ValueError('non-increasing phase')
                    phase_when+=delta_phase
                    if phase_when>=2**63:raise ValueError('phase timestamp overflow')
                    tmp.append(phase_when)
                extras=tuple(tmp)
            rec=(ident,when,extras)
        elif family=='motion':
            if pos+2>len(buf):raise ValueError('truncated motion')
            sign,direction=buf[pos],buf[pos+1];pos+=2
            if sign>11 or direction>1:raise ValueError('invalid sign/motion')
            rec=(ident,when,sign,'r' if direction else 'd')
        elif family=='ingress':
            if pos>=len(buf):raise ValueError('truncated ingress')
            sign=buf[pos];pos+=1
            if sign>11:raise ValueError('invalid sign')
            rec=(ident,when,sign)
        else:raise ValueError('unknown family')
        records.append(rec)
    if pos!=len(buf):raise ValueError('trailing/unconsumed block bytes')
    return records


def digest_records(by_family):
    """Canonical independent comparison of all scientific fields including source row IDs."""
    results={}
    for family,bodies in sorted(by_family.items()):
        dig=hashlib.sha256()
        for body,records in sorted(bodies.items()):
            for rec in records:
                canonical=[family,body,rec[0],rec[1]]
                if family=='synod':canonical.append(None if rec[2] is None else list(rec[2]))
                else:canonical.extend(rec[2:])
                dig.update(json.dumps(canonical,separators=(',',':'),ensure_ascii=False).encode('utf-8')+b'\n')
        results[family]=dig.hexdigest()
    return results


def pack_dataset(by_family,audit,outfile:Path):
    payload_parts=[];toc=[];offset=0;tot=collections.Counter()
    for family,bodies in sorted(by_family.items()):
        for body,rows in sorted(bodies.items()):
            for i in range(0,len(rows),MAX_RECORDS_PER_BLOCK):
                chunk=rows[i:i+MAX_RECORDS_PER_BLOCK]
                raw=encode_block(family,chunk)
                encoded=zlib.compress(raw,9)
                if len(raw)>MAX_BLOCK_UNCOMPRESSED or len(encoded)>MAX_COMPRESSED_BLOCK:
                    raise ValueError('Block size exceeds safety bounds')
                toc.append({'family':family,'body':body,'min_us':str(chunk[0][1]),
                    'max_us':str(chunk[-1][1]),'count':len(chunk),'start':offset,'length':len(encoded),
                    'raw_length':len(raw),'raw_sha256':hashlib.sha256(raw).hexdigest()})
                payload_parts.append(encoded)
                offset+=len(encoded);tot[family]+=len(chunk)
    header={'format':'VSEPH-P0-EXPERIMENTAL','format_version':[0,0,1],
        'warning':'NOT VSEPH v1; provisional layout, do not use for public releases',
        'unit':'signed i64 Unix/POSIX UTC microseconds',
        'motion_ingress_naive_timestamp_assumption':'UTC (must be confirmed by dataset owner)',
        'codec':'zlib','block_records_max':MAX_RECORDS_PER_BLOCK,
        'record_model':'base i64 LE + per-record unsigned varint(id,delta_us); synod optional phases (null=0, nonnull len+1, phase deltas); motion sign byte+direction byte; ingress sign byte',
        'sections':dict(sorted(tot.items())),'table_of_contents':toc}
    header_bytes=json.dumps(header,ensure_ascii=False,separators=(',',':'),sort_keys=True).encode('utf-8')
    if len(header_bytes)>MAX_HEADER_SIZE:raise ValueError('Oversized header')
    with outfile.open('wb') as f:
        f.write(MAGIC);f.write(struct.pack('<I',len(header_bytes)));f.write(header_bytes)
        for payload in payload_parts:f.write(payload)
    with outfile.open('rb') as stream:
        packed_sha256 = hashlib.file_digest(stream,'sha256').hexdigest()
    manifest={'dataset_status':'private_user_upload_only','format_version':'0.0.1-prototype',
        'dataset_license':'not_selected; do_not_publish',
        'source':'pgAdmin4 CSV exports supplied by the dataset owner on 2026-11-10',
        'file':outfile.name,'file_size_bytes':outfile.stat().st_size,
        'file_sha256':packed_sha256,
        'header_size_bytes':len(header_bytes),'compressed_payload_bytes':offset,'block_count':len(toc),
        'roundtrip_digests_sha256':digest_records(by_family),
        'report':audit}
    return manifest


def safe_decompress(data:bytes,declared_length:int):
    if declared_length<8 or declared_length>MAX_BLOCK_UNCOMPRESSED:raise ValueError('unsafe declared uncompressed size')
    z=zlib.decompressobj()
    try:
        plain=z.decompress(data,declared_length+1)
    except zlib.error as err:
        raise ValueError('invalid compressed block') from err
    if len(plain)!=declared_length or not z.eof or z.unused_data or z.unconsumed_tail:
        raise ValueError('invalid compressed block length/stream')
    if z.flush():raise ValueError('unexpected decompressor tail')
    return plain


def unpack_dataset(path:Path):
    content=path.read_bytes()
    if len(content)<12 or content[:8]!=MAGIC:raise ValueError('invalid VSEPH-P0 magic')
    header_size=struct.unpack_from('<I',content,8)[0]
    if header_size>MAX_HEADER_SIZE or 12+header_size>len(content):raise ValueError('invalid header length')
    header=json.loads(content[12:12+header_size])
    if header.get('format')!='VSEPH-P0-EXPERIMENTAL' or header.get('format_version')!=[0,0,1] or header.get('codec')!='zlib':
        raise ValueError('unsupported VSEPH-P0 header')
    base=12+header_size
    groups=collections.defaultdict(lambda: collections.defaultdict(list))
    expected_offset=0
    for entry in header['table_of_contents']:
        fam=entry['family'];body=entry['body'];count=entry['count']
        if fam not in ('synod','motion','ingress') or not isinstance(body,str) or not body:
            raise ValueError('invalid toc family/body')
        offset=entry['start'];length=entry['length']
        if not all(isinstance(x,int) for x in (offset,length,count)) or offset!=expected_offset or length<0 or length>MAX_COMPRESSED_BLOCK or base+offset+length>len(content):
            raise ValueError('unsafe TOC extent')
        expected_offset+=length
        compressed=content[base+offset:base+offset+length]
        plain=safe_decompress(compressed,entry['raw_length'])
        if hashlib.sha256(plain).hexdigest()!=entry['raw_sha256']:raise ValueError('block SHA-256 mismatch')
        rows=decode_block(fam,plain,count)
        if str(rows[0][1])!=entry['min_us'] or str(rows[-1][1])!=entry['max_us']:
            raise ValueError('TOC range mismatch')
        groups[fam][body].extend(rows)
    if base+expected_offset!=len(content):raise ValueError('trailing dataset bytes')
    if {f:sum(len(x) for x in bodies.values()) for f,bodies in groups.items()}!=header['sections']:
        raise ValueError('section counts mismatch')
    return dict(groups),header


def verify_against_sources(csv_dir:Path,bin_path:Path):
    original,audit=read_inputs(csv_dir)
    decoded,header=unpack_dataset(bin_path)
    orig_digest=digest_records(original);back_digest=digest_records(decoded)
    if orig_digest!=back_digest:
        raise ValueError('Roundtrip record mismatch: '+repr((orig_digest,back_digest)))
    # Explicit per-body tuples equality to guard against ordering/digest logic mistakes.
    for fam,src_bodies in original.items():
        if set(src_bodies)!=set(decoded.get(fam,{})):raise ValueError('Body names differ: '+fam)
        for body,records in src_bodies.items():
            if records != decoded[fam][body]:raise ValueError(f'Row inequality {fam} {body}')
    return {'verified':True,'total_rows':sum(audit['row_counts'].values()),
        'per_table_rows':audit['row_counts'],'same_row_tuples':True,
        'digests_sha256':orig_digest,'blocks':len(header['table_of_contents'])}


def main(argv=None):
    ap=argparse.ArgumentParser(description=__doc__)
    sub=ap.add_subparsers(dest='command',required=True)
    pack=sub.add_parser('pack',help='Pack the 3 pgAdmin CSV files into private VSEPH-P0')
    pack.add_argument('folder',type=Path)
    pack.add_argument('--out',type=Path,required=True)
    pack.add_argument('--manifest',type=Path,required=True)
    verify=sub.add_parser('verify',help='Compare every scientific field against the source CSV')
    verify.add_argument('folder',type=Path);verify.add_argument('binary',type=Path)
    info=sub.add_parser('info',help='Inspect counts and coverage without revealing excluded application columns')
    info.add_argument('binary',type=Path)
    opts=ap.parse_args(argv)
    if opts.command=='pack':
        data,audit=read_inputs(opts.folder)
        opts.out.parent.mkdir(parents=True,exist_ok=True)
        manifest=pack_dataset(data,audit,opts.out)
        opts.manifest.write_text(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
        print(json.dumps({'packed':opts.out.name,'size_bytes':manifest['file_size_bytes'],
              'rows':audit['row_counts'],'blocks':manifest['block_count']},ensure_ascii=False))
    elif opts.command=='verify':
        print(json.dumps(verify_against_sources(opts.folder,opts.binary),indent=2))
    else:
        dataset,header=unpack_dataset(opts.binary)
        print(json.dumps({'format':header['format'],'counts':header['sections'],
                         'bodies':{f:sorted(body) for f,body in dataset.items()},
                         'blocks':len(header['table_of_contents'])},indent=2))

if __name__=='__main__':
    main()
