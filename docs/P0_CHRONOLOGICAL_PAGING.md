# Chronological event pages — experimental P0

**Status:** draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), **NOT RUN with Rust/Cargo** after this increment. The compressed scientific dataset is staged separately in [PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). This is NOT the final VSEPH v1 format.

## APIs and semantics

Two Rust entry points share the same contract:

- `virgostars_events_core::EventIndex::chronological_between(start, end, limit, after)`: merges immutable in-memory recorded events and returns a page of borrowed `&Event` values.
- `virgostars_events_p0::P0Archive::chronological_between(start, end, limit, after)`: merges *on-demand decoded* experimental compressed P0 records, returning owned `P0Record` values and their original positive PostgreSQL source IDs.

Order is ascending **signed UTC/POSIX microseconds**, then `EventFamily` (Synod, Motion, Ingress), then lexicographically ordered exact `BodyId`. Both methods use a **start-inclusive / end-exclusive** range `[start, end)`, including pre-1970 instants. No extrapolation, timezone conversion or astronomical state certification is performed.

Page limits are `1..=MAX_CHRONOLOGICAL_PAGE_SIZE` (10,000); zero and larger pages are rejected. The optional `next_cursor` appears only when more recorded events remain in this range. With P0, active work consists of at most **one decoded block of at most 512 records per family/body series** plus the current output page, not the complete 380,844-event archive.

## Cursor contract

- Always use the cursor returned by the previous page of **the same index or archive** and the **exact same start/end interval**. Cursors retain their original interval bounds and reject their reuse if either bound changes, even when the cursor timestamp is still in range.
- The file-backed P0 cursor retains the original source ID; each continuation verifies that its recorded event still exists at the expected instant.
- Cursors are not serialized or cryptographically version-bound. Keep the dataset file immutable between pages. Verify its full SHA-256 against the separately trusted manifest using `P0Archive::verify_archive_sha256()` when authenticity matters. A matching hash is not evidence that missing historical events were calculated.
- `next_cursor = None` means no further *recorded* events inside the requested interval.

## Minimal file-backed usage

```rust
use virgostars_events_core::TimestampMicros;
use virgostars_events_p0::P0Archive;

fn example(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let archive = P0Archive::open(path)?;
    let start = TimestampMicros::new(0);
    let end = TimestampMicros::new(1_000_000_000_000);
    let mut cursor = None;
    loop {
        let page = archive.chronological_between(start, end, 200, cursor.as_ref())?;
        for record in &page.events {
            println!("{}: {} (source {})",
                record.at().as_i64(), record.event.body(), record.source_id);
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(())
}
```

## Test contract and validation status

Run locally on Windows after safely updating the draft branch and preserving uncommitted local changes:

```powershell
cargo fmt --all
py -3 tools/validate_rust.py
```

Targeted new suites:
```powershell
cargo test -p virgostars-events-core --test chronological_public_api
cargo test -p virgostars-events-p0 --test chronological_p0
```

- **6 in-memory integration tests**: tie ordering, stable cursor paging, historical and half-open boundaries, invalid limits, invalid/changed-interval cursor, sparse-gapped data.
- **7 P0 file-backed integration tests**: sorted exact IDs for the independent synthetic golden file, page sizes 1/2/3/5/7/8, in-memory parity, boundaries/gaps, invalid limits/cursors, correct final-page detection, and two separate compressed blocks with an interleaved motion event.
- **Results:** the maintainer reported all four Cargo gates passed on the *previous* Rust revision. The 13 new tests and file-backed chronological code are **NOT YET COMPILED OR EXECUTED** by the agent; static review cannot substitute for Cargo. P0.7 stays in progress until a successful rerun on this revision.

Next: validate the combined draft PR checkout against the real 380,844-event archive; benchmark query memory/I/O; add filters and persistent cursors only after VSEPH v1 API stabilization. Dataset license, release authorization and review of missing lunar intervals are separate.

**Powered by VirgoStars — https://www.virgostars.it/** (source code CPAL-1.0; dataset reuse license separately pending).
