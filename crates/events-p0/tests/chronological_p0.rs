/*
 * The contents of this file are subject to the Common Public Attribution
 * License Version 1.0 (CPAL-1.0). See LICENSE and NOTICE.
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All rights reserved.
 * Powered by VirgoStars — https://www.virgostars.it/
 */
//! File-backed chronological queries against an independent Python golden P0
//! fixture; no astronomical completeness is inferred.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use virgostars_events_core::{
    BodyId, EventFamily, EventIndex, TimestampMicros, MAX_CHRONOLOGICAL_PAGE_SIZE,
};
use virgostars_events_p0::P0Archive;

const GOLDEN_SHA256: &str = "4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499";
static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

struct FixtureFile(PathBuf);

impl FixtureFile {
    fn new() -> Self {
        let bytes = STANDARD
            .decode(include_str!("fixtures/synthetic.p0.base64").trim())
            .expect("valid synthetic fixture");
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), GOLDEN_SHA256);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "virgostars-chrono-{}-{nanos}-{id}.p0",
            std::process::id()
        ));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        output.write_all(&bytes).unwrap();
        Self(path)
    }
}

impl Drop for FixtureFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn time(us: i64) -> TimestampMicros {
    TimestampMicros::new(us)
}

fn body(name: &str) -> BodyId {
    BodyId::try_from(name).unwrap()
}

fn selected(page: &[virgostars_events_p0::P0Record]) -> Vec<(i64, EventFamily, String, u64)> {
    page.iter()
        .map(|record| (
            record.at().as_i64(),
            record.event.family(),
            record.event.body().as_str().to_owned(),
            record.source_id,
        ))
        .collect()
}

#[test]
fn golden_p0_merges_all_three_families_and_breaks_same_time_ties() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let page = archive.chronological_between(time(-876_545), time(5_000_001), 100, None).unwrap();
    assert!(page.next_cursor.is_none());
    assert_eq!(
        selected(&page.events),
        vec![
            (-876_544, EventFamily::Synod, "Moon".to_owned(), 10),
            (-876_544, EventFamily::Ingress, "Moon".to_owned(), 41),
            (0, EventFamily::Motion, "Mars".to_owned(), 31),
            (1_500_000, EventFamily::Synod, "Moon".to_owned(), 12),
            (2_000_000, EventFamily::Synod, "Mercury".to_owned(), 21),
            (2_200_000, EventFamily::Motion, "Mars".to_owned(), 32),
            (3_000_000, EventFamily::Ingress, "Moon".to_owned(), 42),
            (5_000_000, EventFamily::Ingress, "Moon".to_owned(), 43),
        ]
    );
}

#[test]
fn paginated_file_queries_reproduce_the_unpaginated_result_exactly() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let direct = archive.chronological_between(time(-876_545), time(5_000_001), 100, None).unwrap();
    let expected = selected(&direct.events);
    for page_size in [1, 2, 3, 5, 7, 8, MAX_CHRONOLOGICAL_PAGE_SIZE] {
        let mut cursor = None;
        let mut records = Vec::new();
        for _ in 0..9 {
            let page = archive
                .chronological_between(
                    time(-876_545),
                    time(5_000_001),
                    page_size,
                    cursor.as_ref(),
                )
                .unwrap();
            assert!(!page.events.is_empty());
            assert!(page.events.len() <= page_size);
            records.extend(selected(&page.events));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert!(cursor.is_none(), "pagination must terminate");
        assert_eq!(records, expected, "page_size={page_size}");
    }
}

#[test]
fn compressed_archive_and_in_memory_core_have_identical_chronology() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let mut stored = Vec::new();
    for (family, name) in [
        (EventFamily::Ingress, "Moon"),
        (EventFamily::Motion, "Mars"),
        (EventFamily::Synod, "Moon"),
        (EventFamily::Synod, "Mercury"),
    ] {
        stored.extend(
            archive
                .between(family, &body(name), time(-876_545), time(5_000_001))
                .unwrap()
                .into_iter()
                .map(|record| record.event),
        );
    }
    let index = EventIndex::from_events(stored).unwrap();
    for (start, end) in [
        (-876_545, 5_000_001),
        (-876_544, 0),
        (0, 2_200_001),
        (2_000_000, 3_000_001),
        (3_000_000, 3_000_000),
        (5_000_000, 5_000_001),
    ] {
        let file_page = archive.chronological_between(time(start), time(end), 100, None).unwrap();
        let memory_page = index.chronological_between(time(start), time(end), 100, None).unwrap();
        let file_keys: Vec<_> = file_page.events.iter()
            .map(|record| (record.at(), record.event.family(), record.event.body().clone()))
            .collect();
        let memory_keys: Vec<_> = memory_page.events.iter()
            .map(|event| (event.at(), event.family(), event.body().clone()))
            .collect();
        assert_eq!(file_keys, memory_keys, "range {start}..{end}");
    }
}

#[test]
fn exact_boundaries_and_missing_periods_never_invent_new_events() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let negative = archive.chronological_between(time(-876_544), time(0), 10, None).unwrap();
    assert_eq!(negative.events.len(), 2);
    assert!(negative.events.iter().all(|event| event.at() == time(-876_544)));
    let gap = archive.chronological_between(time(1), time(1_000), 10, None).unwrap();
    assert!(gap.events.is_empty());
    assert!(gap.next_cursor.is_none());
    let empty = archive.chronological_between(time(0), time(0), 10, None).unwrap();
    assert!(empty.events.is_empty());
    let last_excluded = archive.chronological_between(time(3_000_000), time(5_000_000), 10, None).unwrap();
    assert_eq!(last_excluded.events.len(), 1);
    assert_eq!(last_excluded.events[0].source_id, 42);
}

#[test]
fn invalid_query_limits_and_cursor_ranges_fail_closed() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    assert!(archive.chronological_between(time(10), time(0), 10, None).is_err());
    assert!(archive.chronological_between(time(0), time(10), 0, None).is_err());
    assert!(archive
        .chronological_between(time(0), time(10), MAX_CHRONOLOGICAL_PAGE_SIZE + 1, None)
        .is_err());
    let page = archive.chronological_between(time(-876_545), time(5_000_001), 1, None).unwrap();
    let cursor = page.next_cursor.as_ref().unwrap();
    assert_eq!(cursor.source_id(), 10);
    assert_eq!(cursor.family(), EventFamily::Synod);
    assert_eq!(cursor.at(), time(-876_544));
    assert_eq!(cursor.body().as_str(), "Moon");
    assert!(archive
        .chronological_between(time(0), time(5_000_001), 1, Some(cursor))
        .is_err());
}

#[test]
fn fully_consumed_page_does_not_expose_a_spurious_next_cursor() {
    let fixture = FixtureFile::new();
    let archive = P0Archive::open(&fixture.0).unwrap();
    let page = archive.chronological_between(time(-876_545), time(5_000_001), 8, None).unwrap();
    assert_eq!(page.events.len(), 8);
    assert!(page.next_cursor.is_none());
    let after_last = archive.chronological_between(time(5_000_001), time(5_000_010), 2, None).unwrap();
    assert!(after_last.events.is_empty());
    assert!(after_last.next_cursor.is_none());
}
