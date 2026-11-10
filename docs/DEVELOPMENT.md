# Development: minimal Rust workspace

**Status:** bootstrap code is in draft PR #5. The workspace is not a released decoder.

## Prerequisites

Install a stable Rust toolchain with `cargo`, `rustfmt` and `clippy` (declared MSRV in workspace: Rust 1.75). No database, network access or sibling VirgoStars repositories are needed for the crates. The current workspace has only a local path dependency between the two crates.

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
- `virgostars-events-core`: `TimestampMicros`, `BodyId`, `ZodiacSign`, `MotionDirection`, typed `SynodPhase`, `SynodEvent`, `MotionEvent`, `IngressEvent`, `Event` and `ModelError`. Types enforce strict body/sign/phase validation. No indexes, binary decode, exported data or `synod.next()` implementation yet.

The code does not use third-party crates. Core unit tests and a public integration test exercise example events with fabricated timestamps (not real astronomical events). More complex golden fixture tests remain tracked in issue #1, independently of the Rust workspace.

## Design notes

- `i64` signed POSIX microseconds ensure exact pre-1970 timestamps and no JavaScript floating-point assumptions.
- `SynodEvent.phases` distinguishes `None` (missing/unknown) from `Some([])` (known empty list). Phases must strictly follow the anchor and each other.
- Names like `true Node` and `osc. Apogee` are valid body IDs, with original case preserved.
- No claim is made about completeness or temporal coverage until an authorized dataset manifest exists.
- The VSEPH magic probe intentionally accepts a 5-byte prefix but does **not** validate the rest of a binary file.

## Follow-up

Next: issue #3 must freeze byte-level VSEPH v1 metadata and golden bytes with corruption tests, then issue #4 on the roadmap implements a bounded binary reader. Issues #5–#7 add indexed query methods and navigation. Maintain the boundaries in [AGENTS.md](../AGENTS.md).

Do not enable GitHub Actions, merge this draft PR, publish packages or access the production database without authorization.
