/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
//! EXPERIMENTAL VSEPH-P0 decoder. NOT the future VSEPH v1 wire format.
//!
//! Reads the 2026-11-10 prototype: 8-byte magic, little-endian JSON-header
//! length, bounded JSON table of contents and independently zlib-compressed
//! blocks with SHA-256 checksums. Does not require a database, ephemeris
//! calculator, network connection or other VirgoStars repositories.
//!
//! All results are only RECORDED events. No claim of astronomical coverage.

pub mod coverage;
pub use coverage::{CoverageAwareP0, CoveredState};

use flate2::bufread::ZlibDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use virgostars_events_core::{
    BodyId, Event, EventFamily, IngressEvent, MotionDirection, MotionEvent, SynodEvent,
    SynodPhase, TimestampMicros, ZodiacSign, MAX_CHRONOLOGICAL_PAGE_SIZE,
};

const MAGIC: &[u8; 8] = b"VSEPH-P0";
const MAX_HEADER: usize = 5_000_000;
const MAX_BLOCK: usize = 5_000_000;
const MAX_RAW: usize = 25_000_000;
const MAX_RECORDS: usize = 512;

#[derive(Debug)]
pub enum P0Error {
    Io(std::io::Error),
    Invalid(String),
    Unsupported(String),
}

impl fmt::Display for P0Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Invalid(message) => write!(f, "invalid VSEPH-P0: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported VSEPH-P0: {message}"),
        }
    }
}

impl Error for P0Error {}

impl From<std::io::Error> for P0Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn invalid(message: impl Into<String>) -> P0Error {
    P0Error::Invalid(message.into())
}

fn string<'a>(item: &'a Value, key: &str) -> Result<&'a str, P0Error> {
    item.get(key).and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("missing or invalid {key}")))
}

fn unsigned(item: &Value, key: &str) -> Result<u64, P0Error> {
    item.get(key).and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("missing or invalid {key}")))
}

fn timestamp(item: &Value, key: &str) -> Result<TimestampMicros, P0Error> {
    string(item, key)?.parse::<i64>()
        .map(TimestampMicros::new)
        .map_err(|_| invalid(format!("invalid i64 timestamp in {key}")))
}

fn family_from_name(value: &str) -> Result<EventFamily, P0Error> {
    match value {
        "synod" => Ok(EventFamily::Synod),
        "motion" => Ok(EventFamily::Motion),
        "ingress" => Ok(EventFamily::Ingress),
        _ => Err(invalid("unknown event family")),
    }
}

#[derive(Clone, Debug)]
struct Block {
    start: u64,
    len: usize,
    raw_len: usize,
    count: usize,
    first: TimestampMicros,
    last: TimestampMicros,
    sha256: String,
}

/// A record identified by its original PostgreSQL source row ID.
/// It is a historical recorded event, not a certified continuous state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P0Record {
    pub source_id: u64,
    pub event: Event,
}

impl P0Record {
    pub fn at(&self) -> TimestampMicros {
        self.event.at()
    }
}

/// Full-block integrity result for an *experimental* P0 file.
///
/// It verifies the encoded events in every indexed compressed block, but does
/// not establish astronomical event completeness or verify an external
/// manifest's whole-file SHA-256. For archival validation, check that digest
/// independently and use an immutable snapshot while auditing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P0AuditSummary {
    pub blocks: usize,
    pub records: u64,
    pub by_family: BTreeMap<EventFamily, u64>,
}

/// Cursor for an immutable experimental P0 archive page.
///
/// The original source row ID makes the selected event unambiguous. This is
/// not a persistent cursor: preserve the source file and query interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P0ChronologicalCursor {
    start: TimestampMicros,
    end: TimestampMicros,
    at: TimestampMicros,
    family: EventFamily,
    body: BodyId,
    source_id: u64,
}

impl P0ChronologicalCursor {
    pub const fn at(&self) -> TimestampMicros {
        self.at
    }

    pub const fn interval_start(&self) -> TimestampMicros {
        self.start
    }

    pub const fn interval_end(&self) -> TimestampMicros {
        self.end
    }

    pub const fn family(&self) -> EventFamily {
        self.family
    }

    pub fn body(&self) -> &BodyId {
        &self.body
    }

    pub const fn source_id(&self) -> u64 {
        self.source_id
    }
}

/// A bounded chronological page of decoded *recorded* P0 events.
///
/// Ownership differs from the in-memory `ChronologicalPage<'_>`: file-backed
/// records are decoded on demand and must be owned by the returned page.
#[derive(Clone, Debug)]
pub struct P0ChronologicalPage {
    pub events: Vec<P0Record>,
    pub next_cursor: Option<P0ChronologicalCursor>,
}

/// A file-backed, immutable, indexed experimental archive.
///
/// Open validates all header/index ranges and counts without allocating
/// decompressed dataset blocks. Each query reads, decompresses and verifies
/// only the relevant block(s), bounded by explicit safety caps.
pub struct P0Archive {
    path: PathBuf,
    payload_start: u64,
    blocks: BTreeMap<(EventFamily, BodyId), Vec<Block>>,
    record_count: u64,
}

impl P0Archive {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, P0Error> {
        let path = path.as_ref().to_owned();
        let mut file = File::open(&path)?;
        let file_len = file.metadata()?.len();
        let mut prefix = [0u8; 12];
        file.read_exact(&mut prefix)?;
        if &prefix[..8] != MAGIC {
            return Err(invalid("incorrect experimental magic"));
        }
        let header_len = u32::from_le_bytes(prefix[8..12].try_into()
            .map_err(|_| invalid("invalid header prefix"))?) as usize;
        if header_len == 0 || header_len > MAX_HEADER {
            return Err(invalid("oversized or empty JSON header"));
        }
        let payload_start = 12_u64.checked_add(header_len as u64)
            .ok_or_else(|| invalid("header offset overflow"))?;
        if payload_start > file_len {
            return Err(invalid("truncated header"));
        }
        let mut header = vec![0u8; header_len];
        file.read_exact(&mut header)?;
        let doc: Value = serde_json::from_slice(&header)
            .map_err(|error| invalid(format!("malformed JSON header: {error}")))?;
        if string(&doc, "format")? != "VSEPH-P0-EXPERIMENTAL"
            || doc.get("format_version") != Some(&serde_json::json!([0, 0, 1]))
            || string(&doc, "codec")? != "zlib"
        {
            return Err(P0Error::Unsupported("format/version/codec mismatch".into()));
        }
        if unsigned(&doc, "block_records_max")? != MAX_RECORDS as u64 {
            return Err(P0Error::Unsupported("block record cap mismatch".into()));
        }
        let entries = doc.get("table_of_contents").and_then(Value::as_array)
            .ok_or_else(|| invalid("missing table_of_contents"))?;
        let mut blocks: BTreeMap<(EventFamily, BodyId), Vec<Block>> = BTreeMap::new();
        let mut expected_offset: u64 = 0;
        let mut totals: BTreeMap<EventFamily, u64> = BTreeMap::new();
        let mut prior_key: Option<(String, String)> = None;
        for entry in entries {
            let family = family_from_name(string(entry, "family")?)?;
            let body = BodyId::try_from(string(entry, "body")?)
                .map_err(|_| invalid("invalid body ID"))?;
            let order_key = (string(entry, "family")?.to_owned(), string(entry, "body")?.to_owned());
            let key = (family, body);
            if prior_key.as_ref().is_some_and(|prev| &order_key < prev) {
                return Err(invalid("table_of_contents key order is not canonical"));
            }
            let first = timestamp(entry, "min_us")?;
            let last = timestamp(entry, "max_us")?;
            if last < first {
                return Err(invalid("block timestamp range reversed"));
            }
            let start = unsigned(entry, "start")?;
            let len = usize::try_from(unsigned(entry, "length")?)
                .map_err(|_| invalid("block length overflow"))?;
            let raw_len = usize::try_from(unsigned(entry, "raw_length")?)
                .map_err(|_| invalid("raw block length overflow"))?;
            let count = usize::try_from(unsigned(entry, "count")?)
                .map_err(|_| invalid("block count overflow"))?;
            if start != expected_offset || len == 0 || len > MAX_BLOCK
                || !(8..=MAX_RAW).contains(&raw_len)
                || count == 0 || count > MAX_RECORDS
            {
                return Err(invalid("unsafe block offset, length or count"));
            }
            expected_offset = expected_offset.checked_add(len as u64)
                .ok_or_else(|| invalid("compressed offset overflow"))?;
            let sha256 = string(entry, "raw_sha256")?.to_owned();
            if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid("bad block SHA-256 digest"));
            }
            if let Some(previous) = blocks.get(&key).and_then(|series| series.last()) {
                if previous.last >= first {
                    return Err(invalid("overlapping or unsorted time blocks"));
                }
            }
            blocks.entry(key.clone()).or_default().push(Block {
                start, len, raw_len, count, first, last, sha256,
            });
            let total = totals.entry(family).or_default();
            *total = total.checked_add(count as u64)
                .ok_or_else(|| invalid("section count overflow"))?;
            prior_key = Some(order_key);
        }
        if payload_start.checked_add(expected_offset) != Some(file_len) {
            return Err(invalid("header index does not cover the entire file"));
        }
        let sections = doc.get("sections").and_then(Value::as_object)
            .ok_or_else(|| invalid("missing sections map"))?;
        if sections.len() != totals.len() {
            return Err(invalid("section count mismatch"));
        }
        for (name, value) in sections {
            let family = family_from_name(name)?;
            if totals.get(&family).copied() != value.as_u64() {
                return Err(invalid("section record count mismatch"));
            }
        }
        let record_count = totals.values().try_fold(0u64, |acc, count| {
            acc.checked_add(*count)
                .ok_or_else(|| invalid("archive record count overflow"))
        })?;
        Ok(Self { path, payload_start, blocks, record_count })
    }

    pub fn record_count(&self) -> u64 {
        self.record_count
    }

    pub fn block_count(&self) -> usize {
        self.blocks.values().map(Vec::len).sum()
    }

    /// Stream SHA-256 of the **entire** experimental archive file.
    ///
    /// This digest is independent of the per-block SHA-256 values checked
    /// during decoding. Callers should compare it with a separately trusted
    /// versioned manifest before treating a snapshot as authentic.
    pub fn archive_sha256(&self) -> Result<String, P0Error> {
        let mut file = File::open(&self.path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Require an exact lower-case SHA-256 from a trusted manifest.
    ///
    /// This checks bytes of the full archive and does not certify scientific
    /// completeness of astronomical event dates.
    pub fn verify_archive_sha256(&self, expected: &str) -> Result<String, P0Error> {
        if expected.len() != 64
            || !expected.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("expected archive SHA-256 must be 64 lower-case hex characters"));
        }
        let digest = self.archive_sha256()?;
        if digest != expected {
            return Err(invalid("whole-file archive SHA-256 mismatch"));
        }
        Ok(digest)
    }

    /// Decode and validate **every** indexed compressed block, not only
    /// blocks touched by a normal query. Counts are calculated from decoded
    /// records and checked against the already validated header totals.
    ///
    /// Processes one block at a time; peak record allocation is capped at
    /// MAX_RECORDS. The caller must separately compare the whole-file digest
    /// to the dataset manifest and ensure the source file is not mutated while
    /// auditing. Does not infer completeness across missing dates.
    pub fn verify_all(&self) -> Result<P0AuditSummary, P0Error> {
        let mut summary = P0AuditSummary {
            blocks: 0,
            records: 0,
            by_family: BTreeMap::new(),
        };
        // PostgreSQL IDs are unique within each source table/family, even
        // when events belong to different bodies or compressed blocks.
        let mut seen_source_ids: BTreeMap<EventFamily, BTreeSet<u64>> = BTreeMap::new();
        for ((family, body), blocks) in &self.blocks {
            for block in blocks {
                let decoded = self.load_block(*family, body, block)?;
                let seen = seen_source_ids.entry(*family).or_default();
                for record in &decoded {
                    if !seen.insert(record.source_id) {
                        return Err(invalid(format!(
                            "duplicate original source ID {} across {:?} blocks/bodies",
                            record.source_id, family
                        )));
                    }
                }
                let count = u64::try_from(decoded.len())
                    .map_err(|_| invalid("decoded block record count overflows u64"))?;
                summary.records = summary.records.checked_add(count)
                    .ok_or_else(|| invalid("archive record count overflow"))?;
                summary.blocks = summary.blocks.checked_add(1)
                    .ok_or_else(|| invalid("archive block count overflow"))?;
                let family_total = summary.by_family.entry(*family).or_default();
                *family_total = family_total.checked_add(count)
                    .ok_or_else(|| invalid("family record count overflow"))?;
            }
        }
        if summary.records != self.record_count || summary.blocks != self.block_count() {
            return Err(invalid("decoded totals disagree with archive index"));
        }
        Ok(summary)
    }

    fn load_block(&self, family: EventFamily, body: &BodyId, block: &Block) -> Result<Vec<P0Record>, P0Error> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(
            self.payload_start.checked_add(block.start)
                .ok_or_else(|| invalid("block seek overflow"))?
        ))?;
        let mut compressed = vec![0u8; block.len];
        file.read_exact(&mut compressed)?;
        let mut decoder = ZlibDecoder::new(Cursor::new(compressed.as_slice()));
        let mut raw = Vec::new();
        // Read to EOF while refusing to allocate beyond the declared limit.
        (&mut decoder).take(block.raw_len as u64 + 1).read_to_end(&mut raw)?;
        let consumed = decoder.into_inner().position();
        if raw.len() != block.raw_len || consumed != compressed.len() as u64 {
            return Err(invalid("invalid compressed stream length or trailing bytes"));
        }
        let sha = format!("{:x}", Sha256::digest(&raw));
        if sha != block.sha256 {
            return Err(invalid("uncompressed block SHA-256 mismatch"));
        }
        let records = decode_records(family, body, &raw, block.count)?;
        if records.first().map(P0Record::at) != Some(block.first)
            || records.last().map(P0Record::at) != Some(block.last)
        {
            return Err(invalid("decompressed timestamps disagree with index"));
        }
        Ok(records)
    }

    /// Latest recorded event at or before the requested timestamp.
    pub fn at(&self, family: EventFamily, body: &BodyId, t: TimestampMicros) -> Result<Option<P0Record>, P0Error> {
        let Some(series) = self.blocks.get(&(family, body.clone())) else { return Ok(None); };
        let end = series.partition_point(|block| block.first <= t);
        let Some(i) = end.checked_sub(1) else { return Ok(None); };
        let records = self.load_block(family, body, &series[i])?;
        Ok(records.into_iter().rev().find(|record| record.at() <= t))
    }

    /// First recorded event strictly later than the requested timestamp.
    pub fn next(&self, family: EventFamily, body: &BodyId, t: TimestampMicros) -> Result<Option<P0Record>, P0Error> {
        let Some(series) = self.blocks.get(&(family, body.clone())) else { return Ok(None); };
        let first = series.partition_point(|block| block.last <= t);
        for block in &series[first..] {
            if let Some(record) = self.load_block(family, body, block)?
                .into_iter().find(|record| record.at() > t)
            {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    /// Last recorded event strictly earlier than the requested timestamp.
    pub fn previous(&self, family: EventFamily, body: &BodyId, t: TimestampMicros) -> Result<Option<P0Record>, P0Error> {
        let Some(series) = self.blocks.get(&(family, body.clone())) else { return Ok(None); };
        let end = series.partition_point(|block| block.first < t);
        for block in series[..end].iter().rev() {
            if let Some(record) = self.load_block(family, body, block)?
                .into_iter().rev().find(|record| record.at() < t)
            {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    /// Recorded events within half-open [start,end), not evidence of completeness.
    pub fn between(
        &self, family: EventFamily, body: &BodyId,
        start: TimestampMicros, end: TimestampMicros,
    ) -> Result<Vec<P0Record>, P0Error> {
        if end < start {
            return Err(invalid("query interval reversed"));
        }
        if end == start {
            return Ok(Vec::new());
        }
        let Some(series) = self.blocks.get(&(family, body.clone())) else { return Ok(Vec::new()); };
        let first = series.partition_point(|block| block.last < start);
        let mut result = Vec::new();
        for block in &series[first..] {
            if block.first >= end { break; }
            result.extend(self.load_block(family, body, block)?
                .into_iter().filter(|record| start <= record.at() && record.at() < end));
        }
        Ok(result)
    }

    /// One bounded chronological page of recorded P0 events in [start,end).
    ///
    /// Stable tie ordering is timestamp, EventFamily, BodyId, matching the
    /// in-memory Rust EventIndex. Only the current compressed block for each
    /// active family/body series is decoded; the complete archive is never
    /// materialized. The cursor is specific to this immutable P0 snapshot.
    pub fn chronological_between(
        &self,
        start: TimestampMicros,
        end: TimestampMicros,
        limit: usize,
        after: Option<&P0ChronologicalCursor>,
    ) -> Result<P0ChronologicalPage, P0Error> {
        if end < start {
            return Err(invalid("chronological query interval reversed"));
        }
        if limit == 0 || limit > MAX_CHRONOLOGICAL_PAGE_SIZE {
            return Err(invalid("chronological page size out of range"));
        }
        if let Some(cursor) = after {
            if cursor.start != start || cursor.end != end
                || cursor.at < start || cursor.at >= end
            {
                return Err(invalid("chronological cursor outside interval"));
            }
            let record = self.at(cursor.family, &cursor.body, cursor.at)?;
            if !record.is_some_and(|record| {
                record.at() == cursor.at && record.source_id == cursor.source_id
            }) {
                return Err(invalid("chronological cursor missing from archive"));
            }
        }
        if start == end {
            return Ok(P0ChronologicalPage {
                events: Vec::new(),
                next_cursor: None,
            });
        }

        // A single decoded block per active series and an ordered next-event
        // head. A block contains at most MAX_RECORDS events (512 in P0).
        let mut heads: BTreeMap<
            (TimestampMicros, EventFamily, BodyId),
            (Vec<P0Record>, usize, usize),
        > = BTreeMap::new();
        for ((family, body), series) in &self.blocks {
            let threshold = after.map_or(start, |cursor| start.max(cursor.at));
            let mut block_index = series.partition_point(|block| block.last < threshold);
            while let Some(block) = series.get(block_index) {
                if block.first >= end {
                    break;
                }
                let records = self.load_block(*family, body, block)?;
                let position = records.partition_point(|record| {
                    record.at() < start || after.is_some_and(|cursor| {
                        (record.at(), *family, body)
                            <= (cursor.at, cursor.family, &cursor.body)
                    })
                });
                let next_at = records.get(position).map(P0Record::at);
                if let Some(next_at) = next_at {
                    if next_at < end {
                        heads.insert(
                            (next_at, *family, body.clone()),
                            (records, position, block_index),
                        );
                    }
                    break;
                }
                block_index += 1;
            }
        }

        let mut events = Vec::with_capacity(limit);
        while events.len() < limit {
            let Some(((_, family, body), (records, position, block_index))) = heads.pop_first() else {
                break;
            };
            events.push(records[position].clone());
            let next_position = position + 1;
            if let Some(next_at) = records.get(next_position).map(P0Record::at) {
                if next_at < end {
                    heads.insert(
                        (next_at, family, body),
                        (records, next_position, block_index),
                    );
                }
            } else {
                let series = self.blocks.get(&(family, body.clone()))
                    .ok_or_else(|| invalid("chronological series disappeared"))?;
                if let Some(next_block) = series.get(block_index + 1) {
                    if next_block.first < end {
                        let next_records = self.load_block(family, &body, next_block)?;
                        let next_at = next_records.first()
                            .ok_or_else(|| invalid("empty indexed P0 block"))?
                            .at();
                        heads.insert(
                            (next_at, family, body),
                            (next_records, 0, block_index + 1),
                        );
                    }
                }
            }
        }

        let next_cursor = if heads.is_empty() {
            None
        } else {
            events.last().map(|last| P0ChronologicalCursor {
                start,
                end,
                at: last.at(),
                family: last.event.family(),
                body: last.event.body().clone(),
                source_id: last.source_id,
            })
        };
        Ok(P0ChronologicalPage { events, next_cursor })
    }

    pub fn synod_at(&self, body: &BodyId, t: TimestampMicros) -> Result<Option<P0SynodHandle<'_>>, P0Error> {
        let record = self.at(EventFamily::Synod, body, t)?;
        Ok(record.map(|record| P0SynodHandle { archive: self, body: body.clone(), record }))
    }
}

/// An indexed synod record with strictly ordered navigation.
pub struct P0SynodHandle<'a> {
    archive: &'a P0Archive,
    body: BodyId,
    pub record: P0Record,
}

impl<'a> P0SynodHandle<'a> {
    pub fn next(&self) -> Result<Option<Self>, P0Error> {
        Ok(self.archive.next(EventFamily::Synod, &self.body, self.record.at())?
            .map(|record| Self { archive: self.archive, body: self.body.clone(), record }))
    }

    pub fn previous(&self) -> Result<Option<Self>, P0Error> {
        Ok(self.archive.previous(EventFamily::Synod, &self.body, self.record.at())?
            .map(|record| Self { archive: self.archive, body: self.body.clone(), record }))
    }
}

fn take_varint(bytes: &[u8], pos: &mut usize) -> Result<u64, P0Error> {
    let mut value = 0u64;
    for step in 0..10 {
        let b = *bytes.get(*pos).ok_or_else(|| invalid("truncated unsigned varint"))?;
        *pos += 1;
        if step == 9 && b > 1 {
            return Err(invalid("unsigned varint overflow"));
        }
        value |= ((b & 0x7f) as u64) << (step * 7);
        if b & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(invalid("unsigned varint exceeds 10 bytes"))
}

fn take_i64_delta(bytes: &[u8], pos: &mut usize, base: i64) -> Result<i64, P0Error> {
    let delta = take_varint(bytes, pos)?;
    let delta = i64::try_from(delta).map_err(|_| invalid("timestamp delta exceeds i64"))?;
    base.checked_add(delta).ok_or_else(|| invalid("timestamp addition overflow"))
}

fn decode_records(
    family: EventFamily, body: &BodyId, bytes: &[u8], count: usize,
) -> Result<Vec<P0Record>, P0Error> {
    let base = bytes.get(..8).ok_or_else(|| invalid("truncated block base time"))?;
    let mut previous = i64::from_le_bytes(base.try_into()
        .map_err(|_| invalid("invalid block base time"))?);
    let mut pos = 8;
    let mut records = Vec::with_capacity(count);
    let mut source_ids = BTreeSet::new();
    for i in 0..count {
        let source_id = take_varint(bytes, &mut pos)?;
        if source_id == 0 || !source_ids.insert(source_id) {
            return Err(invalid("zero or duplicate original source ID inside block"));
        }
        let current = take_i64_delta(bytes, &mut pos, previous)?;
        if (i == 0 && current != previous) || (i > 0 && current <= previous) {
            return Err(invalid("non-increasing or incorrect block-base timestamp"));
        }
        previous = current;
        let at = TimestampMicros::new(current);
        let event = match family {
            EventFamily::Synod => {
                let code = take_varint(bytes, &mut pos)?;
                if code > 1024 { return Err(invalid("synod phase count cap exceeded")); }
                let mut phases = Vec::new();
                let mut last_phase = current;
                for n in 0..code.saturating_sub(1) {
                    let when = take_i64_delta(bytes, &mut pos, last_phase)?;
                    if when <= last_phase { return Err(invalid("non-increasing phase time")); }
                    phases.push(SynodPhase::new(format!("extra_{}", n + 1), TimestampMicros::new(when))
                        .map_err(|error| invalid(error.to_string()))?);
                    last_phase = when;
                }
                let phases = if code == 0 { None } else { Some(phases) };
                Event::Synod(SynodEvent::new(body.clone(), at, phases)
                    .map_err(|error| invalid(error.to_string()))?)
            }
            EventFamily::Ingress => {
                let sign = *bytes.get(pos).ok_or_else(|| invalid("truncated ingress sign"))?;
                pos += 1;
                let sign = ZodiacSign::try_from(sign).map_err(|error| invalid(error.to_string()))?;
                Event::Ingress(IngressEvent::new(body.clone(), at, sign))
            }
            EventFamily::Motion => {
                let raw = bytes.get(pos..pos + 2).ok_or_else(|| invalid("truncated motion fields"))?;
                pos += 2;
                let sign = ZodiacSign::try_from(raw[0])
                    .map_err(|error| invalid(error.to_string()))?;
                let direction = match raw[1] {
                    0 => MotionDirection::Direct,
                    1 => MotionDirection::Retrograde,
                    _ => return Err(invalid("invalid motion direction byte")),
                };
                Event::Motion(MotionEvent::new(body.clone(), at, direction, sign))
            }
        };
        records.push(P0Record { source_id, event });
    }
    if pos != bytes.len() {
        return Err(invalid("unexpected trailing block bytes"));
    }
    Ok(records)
}
