# Reviewed coverage in Rust (experimental)

**Status:** draft PR [#5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5), not merged. These types are preliminary, not a stable API or an astronomy engine. The reference policy and Python adapter remain in [dataset PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6).

## Scientific distinction

The last **recorded** ingress/motion event is not necessarily the last **actual astronomical** event. In the supplied snapshot, Moon sign ingresses have a very long discontinuity after 2099 and restart with Aries-only records in 4836. Reading the 2099 event for a request in the year 3000 is correct as **recorded evidence**, but returning its sign as a scientifically confirmed state is wrong.

The shared Rust core now has `CoveragePolicy`, `ReviewedWindow`, `CoverageAssessment`, `CoverageError`. It differentiates:

- `UnknownCoverage`: no human-reviewed, completeness-certified interval contains the requested time.
- `UnknownInitialState`: a reviewed interval exists but the latest recorded event is older than its start, so the initial state is not established.
- `Reviewed`: an event within the reviewed interval establishes a state at the requested instant. **This trusts the supplied human review evidence**, not the dataset index itself.

A `ReviewedWindow` has one family (ingress or motion), body, signed i64 microsecond start and end, reviewer and evidence. Intervals are **[start, end)**, disjoint by family/body; missing reviewer or evidence and overlaps fail. Synod phases/cycle navigation are not eligible for direct sign/motion state coverage.

## Bind the coverage policy to a specific archive

The experimental `virgostars-events-p0` crate now exposes:

```rust
use virgostars_events_core::{BodyId, TimestampMicros};
use virgostars_events_p0::{CoveredState, P0Archive};

let archive = P0Archive::open("datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0")?;
let safe = archive.with_coverage_policy("prototype/coverage-policy-20261110.json")?;
let moon = BodyId::try_from("Moon")?;
let t = TimestampMicros::new(32_503_680_000_000_000); // illustrative instant
match safe.sign_at(&moon, t)? {
    CoveredState::UnknownCoverage { .. } => { /* do not claim a sign */ }
    CoveredState::UnknownInitialState { .. } => { /* do not claim a sign */ }
    CoveredState::Reviewed { record, reviewer, evidence, .. } => {
        // The named reviewer and evidence, not the file index, claim completeness.
        // Inspect the record and provenance before showing it to users.
        let _ = (record, reviewer, evidence);
    }
}
```

The snippet belongs inside a Rust function returning a compatible error type. The current **2026-11-10 policy contains zero reviewed windows**, so all current real-data state lookups are `UnknownCoverage` even if a recorded event is present.

`P0Archive::with_coverage_policy` requires `schema="virgostars-reviewed-coverage/v0"` and `verification="human-reviewed-only"`. It parses policy times as signed decimal-string POSIX microseconds, rejects overlapping/malformed windows and checks a **lowercase SHA-256 of the entire archive** against `archive_sha256` in the policy. To avoid trusting a changed file, P0 currently re-hashes the archive before each coverage-aware query. This is safe for experimental correctness but **not optimized for high-frequency queries**. A future verified, immutable file handle and hash caching can improve performance.

The `P0Archive` raw event APIs (`at`, `next`, `previous`, `between`) remain available **without** coverage review, but their output MUST be labeled as recorded events rather than trusted continuous states. Do not automatically certify any time interval based solely on event density or apparent regularity.

## Validate before merge

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

The new shared-core tests and the P0 coverage tests use fabricated events, including pre-1970 microseconds, half-open boundaries, unknown initial state, rejection of unreviewed/overlapping windows and rejection of a mutated archive/hash mismatch. Tests are **written but not yet run with Cargo** in this environment. The independent Python P0 reference tests were re-executed here and passed 6/6.

The full 2026-11-10 policy and actual P0 binary are published on a **different draft PR #6**. Validate on a combined local checkout before claiming Rust/Python parity or scientific coverage.

## Next engineering steps

1. Execute Cargo verification and fix compilation/clippy/formatting if necessary.
2. Test the exact 2026-11-10 zero-window policy against the reassembled real archive in a combined checkout.
3. Port the same semantics to the final VSEPH v1 reader, backed by a frozen wire specification and golden fixtures.
4. Have independent reviewers certify any specific family/body/time range and record the evidence in a separate versioned coverage policy. Never edit the immutable 2026-11-10 dataset to fabricate missing events.

Attribution: **Powered by VirgoStars** — https://www.virgostars.it/. Source CPAL-1.0; dataset reuse license is tracked separately in issue #9.
