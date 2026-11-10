# Development: minimal Rust workspace

**Status:** bootstrap code is in draft PR #5. The workspace is not a released decoder.

## Prerequisites

Install a stable Rust toolchain with `cargo`, `rustfmt` and `clippy` (declared MSRV in workspace: Rust 1.75). No database, network access or sibling VirgoStars repositories are needed for the crates. The core and format crates share a local path dependency. The experimental P0 reader needs crates.io dependencies for its first build.

## Commands from repository root

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

These commands are expected review gates, not evidence they have run. On the original authoring environment, Cargo and rustc were **unavailable**, so outcomes must be verified by someone with an installed Rust toolchain before marking P0.2 done.

## Crates

- `virgostars-events-format`: exposes `VSEPH_MAGIC` and `probe_magic()` only. This is **not a complete binary parser**, version validator, checksum checker or reader. The binary layout is still draft and belongs to issue #3.
- `virgostars-events-core`: `TimestampMicros`, `BodyId`, `ZodiacSign`, `MotionDirection`, typed `SynodPhase`, `SynodEvent`, `MotionEvent`, `IngressEvent`, `Event` and `ModelError`. The crate now also exposes a **preliminary immutable in-memory** `EventIndex`, `EventFamily`, `SynodHandle`, `IndexError` and `QueryError`. It provides indexed `at`, `next`, `previous`, `between`, and `synod_at`/handle navigation over **already loaded, recorded events**. No file-backed VSEPH decoder or completeness certification exists.

The core and format crates have no third-party runtime dependencies; the separate experimental events-p0 crate uses serde_json, flate2 (pure Rust backend) and sha2, plus base64 for its independent test fixture. Core and public integration tests use fabricated event timestamps. More complex golden fixture tests remain tracked in issue #1, independently of the Rust workspace.

## Design notes

- `i64` signed POSIX microseconds ensure exact pre-1970 timestamps and no JavaScript floating-point assumptions.
- `SynodEvent.phases` distinguishes `None` (missing/unknown) from `Some([])` (known empty list). Phases must strictly follow the anchor and each other.
- Names like `true Node` and `osc. Apogee` are valid body IDs, with original case preserved.
- No claim is made about completeness or temporal coverage until an authorized dataset manifest exists.
- The VSEPH magic probe intentionally accepts a 5-byte prefix but does **not** validate the rest of a binary file.

## Follow-up

Next: issue #3 must freeze byte-level VSEPH v1 metadata and golden bytes with corruption tests, then roadmap P0.4 implements a bounded file-backed binary reader. P0.5's preliminary **in-memory** event navigation is present, but the full P0.5 acceptance criteria still require fixture parity, coverage safeguards, error semantics and verification. Maintain the boundaries in [AGENTS.md](../AGENTS.md).

Do not enable GitHub Actions, merge this draft PR, publish packages or access the production database without authorization.


## Immutable in-memory timeline (under review)

The new module at crates/events-core/src/indexed.rs provides the following Rust APIs for **recorded events**:

~~~rust
use virgostars_events_core::{BodyId, EventIndex, EventFamily, TimestampMicros};

let moon = BodyId::try_from("Moon")?;
let index = EventIndex::from_events(records)?; // records: Vec<Event> supplied by caller
let t = TimestampMicros::new(1_800_000_000_000_000);
let recorded = index.at(EventFamily::Ingress, &moon, t);
let next_recorded = index.next(EventFamily::Ingress, &moon, t);
let earlier_recorded = index.previous(EventFamily::Ingress, &moon, t);
let interval = index.between(
    EventFamily::Ingress, &moon,
    TimestampMicros::new(0), t,
)?;
let synod = index.synod_at(&moon, t);
let following_synod = synod.and_then(|handle| handle.next());
~~~

This snippet is illustrative and not a working standalone main function: `records` must come from a decoder or test setup, and this crate does not yet load an actual VSEPH-P0 archive.

- Time points are signed **i64 POSIX microseconds**; `at` is inclusive, `next` and `previous` are strict, and `between` is [start, end).
- Different bodies/families may have events at the same instant; duplicate family/body/time records are rejected, not silently overwritten.
- An empty/missing series never manufactures a prior or next event.
- Synod navigation stays within one body. End of the last recorded synod remains unknown.
- These are **recorded-event lookups only**: an old Moon ingress from 2099 does not establish a scientifically verified sign in 3000. The Python P0 prototype's SHA-bound `coverage_policy.py` is separate; an equivalent reviewed coverage gate must be implemented in Rust before any `sign_at`/`direction_at` result can claim completeness.

**Validation still required:** Cargo fmt, Clippy, unit/integration tests and release build are **NOT RUN** in this environment because Cargo/rustc are unavailable. The new tests have been committed but not compiled here.

## Experimental compressed VSEPH-P0 Rust reader (draft PR #5)

The newly added **virgostars-events-p0** crate is an isolated, provisional decoder for the existing compressed 2026-11-10 VSEPH-P0 archive. It does **not** define or freeze the future VSEPH v1 format.

It validates the 8-byte VSEPH-P0 magic, bounded JSON header, version, zlib codec, indexed block extents, per-family record counts, and SHA-256 of each decompressed block. It decodes signed i64 POSIX microsecond timestamps, original source row IDs, motion stations, ingresses, and optional synod phases. Query methods: at(), next(), previous(), between(), synod_at() with handle navigation. Every query returns **recorded evidence only**; no time-range completeness is certified. Synod phases have ordinal names (extra_1, extra_2, etc.), not a verified physical phase label.

The independently Python-generated golden fixture is stored as Base64 source at crates/events-p0/tests/fixtures/synthetic.p0.base64. It represents **eight deliberately synthetic events across four blocks**, with decoded file SHA-256 **4b7dbe3171633402b76565df79a31066f58913b52b2839c735ea3e68cb413499**. Rust tests check exact event parity, boundary semantics, NULL/empty phase distinction and corruption rejection.

Run the standard Rust gates from the repository root:

~~~sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
~~~

These Rust commands have **NOT RUN** in the authoring environment (no cargo/rustc). Python reference decoding and existing six Python prototype tests have been executed successfully. After assembling dataset PR #6 in a checkout that includes the Rust PR, optionally verify the real 380,844-record archive:

~~~sh
python tools/assemble_dataset.py
VSEPH_P0_PATH=datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 cargo test -p virgostars-events-p0 --test golden_p0 -- --ignored
~~~

The ignored test checks 761 blocks and performs a recorded-event lookup, NOT an astronomical coverage certification. Global file SHA-256 is separately validated by the published dataset manifest and assembler; the reader checks block hashes lazily when queried.

## SHA-256-bound coverage policy in Rust (draft)

Shared `events-core` now defines validated `ReviewedWindow`, `CoveragePolicy` and `CoverageAssessment` with fail-closed state semantics. The P0 adapter parses the same `virgostars-reviewed-coverage/v0` policy format used by the Python reference in draft PR #6, verifies the full archive SHA-256, and rejects policy overlap, invalid ranges and changed binary data. `CoverageAwareP0.sign_at` and `direction_at` distinguish `UnknownCoverage`, `UnknownInitialState` and an explicitly `Reviewed` recorded event. The latter relies on externally reviewed completeness evidence and is not derived from file index density. The real-data policy currently contains **zero approved intervals**, so no real-data states are certified.

New tests: five shared-core coverage cases in `crates/events-core/src/coverage.rs` and seven P0 cross-language-behavior cases in `crates/events-p0/tests/coverage_p0.rs`. **These Rust tests have not been compiled or executed here**; Cargo is unavailable. The reference Python decoder tests were actually executed in the local source directory with `python -m unittest -v test_vseph_prototype` and passed **6/6**. The real VSEPH-P0 file was inspected again with SHA-256 `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e` and **380,844 source records / 761 blocks**.

Full behavior, usage and hash caveats: [REVIEWED_COVERAGE_RUST.md](REVIEWED_COVERAGE_RUST.md). P0 currently re-hashes the complete archive for each coverage-aware query to detect changes; benchmark and replace with immutable verified file handles before performance-sensitive use. No production release or merge until Cargo checks and cross-language parity execute successfully.

## Independent whole-archive P0 integrity audit (Python, actually executed)

The added **`tools/audit_p0_bytes.py`** independently checks every compressed P0 block and every encoded record. Its **`tools/test_audit_p0_bytes.py`** regression suite uses the independently generated 8-event fixture and rejects corrupt headers, incorrect SHA-256, truncated/appended payloads and compressed-block corruption. Full instructions for Windows and macOS/Linux: [P0_BYTE_AUDIT.md](P0_BYTE_AUDIT.md).

```sh
python -m unittest discover -s tools -p 'test_audit_p0_bytes.py' -v
python tools/audit_p0_bytes.py datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

**Actual execution on 2026-11-10:** **PASS 7/7** Python tests with the real-data optional test enabled; **PASS 761/761 zlib blocks and 380,844/380,844 encoded records**, with all per-block SHA-256 and complete-archive hash matching. The exact committed Python files were matched to locally executed copies via Git blob IDs. The dataset and tests remain in separate PR branches; a fresh checkout is not yet validated. Rust/Cargo tests are **NOT RUN** in this environment; do not infer Rust compilation from Python results.

## Full-archive validation from Rust (new, not yet compiled)

The experimental P0 crate now exposes `P0Archive::verify_all()`. It traverses every block in the index, reuses the bounded zlib decoder, verifies each decompressed SHA-256, checks source IDs, timestamps, signs, directions, synod phases, per-block first/last times, and returns a `P0AuditSummary` with exact records, blocks and per-family counts. One block's records are held in memory at a time.

For an offline CLI after reconstructing the snapshot in a combined checkout containing PRs #5 and #6:

```sh
cargo run -p virgostars-events-p0 --bin p0-verify -- datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0
```

The command prints JSON with `blocks`, `records`, `sections` and `coverage_certified: false`, and exits nonzero on corruption or invalid input. It **does not** certify continuous astronomical coverage. The new optional `--sha256` flag compares the **full archive file** to the expected digest supplied from a trusted manifest; without that flag the command computes and prints the digest but does not authenticate a snapshot.

Two new golden tests cover all-block validation and corruption in a **nonqueried** series; two more exercise the CLI's successful JSON result and nonzero exit on corruption. The optional ignored integration test now requests a full Rust audit of all **761 indexed blocks / 380,844 records**, not merely a header inspection. These Rust tests and CLI are **COMMITTED BUT NOT EXECUTED**: there is no Cargo/rustc in the current environment. The independent Python regression suite, including a matching nonqueried-block corruption case, was executed on the real archive: **8/8 PASS**. See [byte audit instructions](P0_BYTE_AUDIT.md).

## Trusted whole-file hash in Rust (new, requires Cargo)

The P0 reader now exposes streaming `P0Archive::archive_sha256()` and `verify_archive_sha256(expected)`. The SHA verification implementation is shared by the experimental review-policy adapter and the offline CLI. The total record count from the JSON header is accumulated with overflow checks. `p0-verify` reports `sha256` and `sha256_checked_against_expected`; it exits nonzero on an incorrect/malformed expected digest.

```sh
cargo run -p virgostars-events-p0 --bin p0-verify -- datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

The expected digest above belongs **only** to the immutable 2026-11-10 dataset in draft PR #6; fetch the correct versioned manifest when verifying newer snapshots. Three new Rust golden regression tests cover full-file digest, CLI correct digest and CLI corrupt/invalid expected digest; **NOT RUN** here because Cargo is unavailable. The Python reference audit was rerun in this session: **8/8 PASS** and **761/761 blocks, 380,844/380,844 encoded events**, correct full-archive SHA-256. Rust build, Clippy and runtime parity are not yet verified.

## Automated local gate orchestration (no GitHub Actions)

The branch includes `tools/validate_rust.py` (Python 3.10+ stdlib) and `tools/test_validate_rust.py`. It works on Windows PowerShell with `py -3 tools\\validate_rust.py` and on macOS/Linux with `python3 tools/validate_rust.py`. It stops immediately on failed Rust format, Clippy, tests or release build. For a reconstructed P0 archive it also runs **two independent full-file audits** and compares JSON `sha256`, `blocks`, `records` and `sections`, requiring the Rust CLI to declare `coverage_certified:false`.

The validation wrapper was executed with a **simulated Cargo invocation** and passed **9/9 Python regression tests**; it correctly refuses to claim success when Cargo is not installed. The actual Cargo toolchain was unavailable here, so the four Rust gates remain **NOT RUN**. Exact commands, dataset instructions and Windows steps are in [LOCAL_RUST_CHECKS.md](LOCAL_RUST_CHECKS.md).


## CSV-to-compressed-archive field parity (executed)

The stdlib-only `tools/verify_csv_p0.py` independently compares normalized **public** CSV rows with the experimental compressed P0 archive **by family, body and original source row ID**. It verifies every scientific field including signed historical UTC microseconds, phases (NULL versus empty), direction and sign; requires exactly the documented CSV headers; and uses a disposable SQLite database so duplicates, missing rows and mismatches are rejected. It first invokes `tools/audit_p0_bytes.py` to check P0 framing and hashes. A trusted `--sha256` checks the complete immutable binary against its snapshot digest.

```sh
python -m unittest discover -s tools -p test_verify_csv_p0.py -v
python tools/verify_csv_p0.py datasets/2026-11-10 datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

**Verified on 2026-11-10:** 11/11 synthetic Python tests PASS; **380,844** normalized CSV rows compared field by field against the actual P0 archive with **no discrepancies**. Tested source and test file bytes match the current GitHub branch blobs. The local integration run used a compatible `audit_file` helper rather than the exact repo copy; a fresh checkout containing both PR #5 and PR #6 remains a necessary additional review step. Rust Cargo checks are still **NOT RUN**. More context: [CSV_P0_FIELD_PARITY.md](CSV_P0_FIELD_PARITY.md).

## Identity checks across astronomical event series

The experimental P0 reader now rejects **zero original PostgreSQL row IDs**, repeated IDs within a decoded block, and repeated IDs across compressed blocks and celestial bodies *within the same family*. Family-specific identity sequences are independent: the same numeric ID in ingress vs motion or synod remains legitimate. The full-archive uniqueness scan is implemented in `P0Archive::verify_all()`, not in every targeted query, to avoid reading unrelated blocks.

Run `python -m unittest discover -s tools -p test_audit_p0_bytes.py -v` and follow [P0_BYTE_AUDIT.md](P0_BYTE_AUDIT.md) for the 13-test independent Python oracle. **13/13 Python tests actually passed**, including the full real snapshot, with exact committed source/fixture Git SHA hashes checked. Rust has four additional identity tests defined in `crates/events-p0/tests/golden_p0.rs` but **they have not been run** because Cargo is unavailable. A new CSV or P0 snapshot must preserve legitimate original ID uniqueness for every source table or fail review.

## Unified chronological event pages in Rust (P0.7, draft)

The in-memory `virgostars-events-core::EventIndex` exposes the newly implemented `chronological_between(start, end, limit, after)` to combine **recorded** synod anchors, motion stations and zodiac ingresses into a single stable stream without loading all matching events.

- **Order:** timestamp (signed POSIX UTC microseconds) ascending, then `EventFamily` (`Synod`, `Motion`, `Ingress`), then exact `BodyId` lexicographically. The current in-memory index rejects duplicate instants for the same family/body, so no record ordinal tie-break is needed here.
- **Range:** half-open `[start, end)`; an event at `start` is included, one at `end` excluded. Pre-1970 timestamps retain full integer precision. Missing periods never produce invented events or certified state.
- **Page:** `limit` in `1..=MAX_CHRONOLOGICAL_PAGE_SIZE` (10,000). A returned `ChronologicalPage.events` contains borrowed `&Event` references and `next_cursor` is `None` on the final page.
- **Cursor:** pass a reference to the previous page's `ChronologicalCursor`; events tied at a single instant are never repeated or skipped. A cursor is valid only for the same immutable index and interval; cursors referring to missing events or outside the range return `ChronologicalQueryError::InvalidCursor`. This is **not** a persistent wire-format cursor or a data version token.
- **Bounds:** rejects reversed intervals and zero or excessive limits. One active head per indexed body/family and at most one page are resident, rather than collecting the entire requested interval.

Example (after creating an `EventIndex` from synthetic/precomputed events):
```rust
use virgostars_events_core::{EventIndex, TimestampMicros};
let index = EventIndex::default();
let start = TimestampMicros::new(-1_000_000);
let end = TimestampMicros::new(1_000_000);
let first = index.chronological_between(start, end, 200, None)?;
let second = index.chronological_between(start, end, 200, first.next_cursor.as_ref())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
The example is illustrative; the source is not yet a release-stable API.

**Test status:** `crates/events-core/tests/chronological_public_api.rs` contains six new Rust integration tests for tie ordering, cursor pagination, signed and half-open boundaries, unsafe inputs, invalid cursors and fail-closed gaps. They are **committed but NOT YET RUN** after this code addition. The maintainer reports that the four local Cargo gates passed for the *prior* code revision, which is not evidence about this change. Run the gates again and report their complete output. P0.7 remains in progress because the P0 file-backed reader lacks the unified chronological page interface.
