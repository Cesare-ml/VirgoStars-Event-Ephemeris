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
