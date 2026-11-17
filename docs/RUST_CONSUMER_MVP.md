# Rust-first offline consumer MVP — VirgoStars Desktop / Mobile

**Status (2026-11-17): Rust 1.90 Linux workspace compilation, locked unit tests and Clippy passed; not released or app-integrated.** The maintainer now prioritizes a Rust-only library for both native applications ahead of Python wheels/WASM. This supersedes the historical Python-first priority without deleting the previous work.

## What the first application can call

`crates/events-rust` adds the `virgostars-event-ephemeris` Rust crate. It wraps the existing `events-p0` Rust decoder and provides one simple, read-only owner:

```rust
use virgostars_event_ephemeris::{EventFamily, VirgoStarsEventEphemeris};

fn show_events(path: &std::path::Path, trusted_sha256: &str) -> Result<(), Box<dyn std::error::Error>> {
    let events = VirgoStarsEventEphemeris::open_verified(path, trusted_sha256)?;
    let now_utc_us: i64 = 1_800_000_000_000_000; // Example only, not today's sky
    let latest_cycles = events.synods_at(now_utc_us)?;
    let next_station = events.next(EventFamily::Motion, "Mars", now_utc_us)?;
    let moon_ingresses = events.between(
        EventFamily::Ingress, "Moon", now_utc_us, now_utc_us + 86_400_000_000
    )?;
    println!("{} cycle bodies, {next_station:?}, {} ingresses", latest_cycles.len(), moon_ingresses.len());
    Ok(())
}
```

Other methods: `at`, `previous`, `raw_archive` (advanced coverage policy and chronological pagination), `audit`, `verified_sha256`, and `open_verified_in(path, digest, cache_root)` for applications with a designated private cache directory. All instants are *signed UTC/POSIX microseconds*, and interval ends are exclusive. `at` includes equality; `next`/`previous` are strict.

`open_verified` rejects an invalid/mismatched **separately trusted** whole-file SHA-256, creates a unique private temporary copy of the input and performs `P0Archive::verify_all()` on **that copy** across every compressed block before handing out a reader. Subsequent queries only read the authenticated snapshot. The original path can be overwritten, renamed or removed without changing the result. The snapshot directory and file are deleted when the reader is dropped, including on rejected digest/full-block audit. It costs a one-time file copy and matching temporary disk space; do not open a new reader for every query. The temporary directory must remain private; this is not a protection against an adversary already able to mutate the app's private cache. It does not invent source IDs, interpolate phases, certify continuous motion/sign state or establish astronomical correctness. P0 remains an **experimental** byte format, not VSEPH v1. For verified ongoing sign/motion state, use only a reviewed SHA-bound coverage policy; the present owner snapshot has no approved complete windows.

## How the apps should consume it

For both `Cesare-ml/virgostars-desktop` and `Cesare-ml/virgostars-mobile-app` (both currently Tauri 2 with their own `src-tauri/Cargo.toml`, Rust 1.90), use a direct Git dependency on an **approved commit** after review and native target validation, rather than copying the reader or building two astronomy engines:

```toml
[dependencies]
virgostars-event-ephemeris = { git = "https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris", rev = "<reviewed_commit_sha>" }
```

Do **not** use the draft head as if it were a released or target-validated dependency. A Rust dependency does not require PyO3, Python, Node.js, HTTP, PostgreSQL, Swiss or a license server at runtime; `events-p0` currently uses pure-Rust zlib, serde and SHA-256. Its `publish = false` flag blocks a premature crates.io publication, **not** local/Git builds.

- Desktop: provision an approved archive in an application-managed data directory, save its independently trusted digest in app configuration, create a shared read-only reader once and invoke it from Tauri commands. The reader automatically creates a private authenticated snapshot. No raw database credentials in the app.
- Android: bundle or provision an **authorized** archive as a local file in app-private storage (Android APK assets are not ordinary `std::fs::File` paths); use `VirgoStarsEventEphemeris::open_verified_in(archive_path, digest, app_private_cache_dir)` with a writable app-private cache directory. The OS default temporary path may not be writable in an Android sandbox, so do not assume `open_verified` is portable there. The mobile prototype currently has an optional `sweph-sys` feature for *chart calculation*; the event reader must not enable it. Native Tauri/Android build configuration is owned by the mobile repository and must be validated on Android targets separately.
- Both: expose `coverage unreviewed` and `unknown` honestly, preserve original timestamps/body spelling and user-facing `Powered by VirgoStars` credit in accordance with CPAL-1.0.

## Acceptance before calling it usable

1. Review the Cargo-generated `Cargo.lock` (not hand-edited), run per-crate `cargo fmt -p virgostars-event-ephemeris -p virgostars-events-generator -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, and `cargo build --locked --workspace --release`. The whole-workspace `cargo fmt --all -- --check` still flags **pre-existing** unformatted Rust files outside these crates; resolve this as a separate reviewed formatting task.
2. Run `cargo test -p virgostars-event-ephemeris` on the committed branch. Synthetic Rust tests cover exact SHA, full-block audit, boundary semantics, missing/invalid body and interval errors, immutable snapshots after source overwrite/removal, private-cache cleanup, corrupted trust inputs and multithreaded read-only access.
3. Verify an authorized immutable real P0 snapshot and its independent normalized scientific CSV parity. The snapshot in draft PR #6 is not automatically an approved/relicensed release asset.
4. Compile a minimal consumer from **each app** against a pinned commit, including at least one Windows and one Android Rust target. Measure cold open, memory and query latency; avoid release claims before real builds.

**Verified locally:** Rust 1.90, synthetic Rust unit tests and workspace Clippy were run on Linux, including the private-copy lifecycle. **Not verified:** Windows/Android target compilation, Tauri consumption, real owner-approved dataset scientific parity or resistance to a malicious writer inside the private cache. Regeneration lives in a separate development crate and is never invoked by either application at runtime. Python/WASM work may continue later but is not a prerequisite for the Rust-first MVP.
