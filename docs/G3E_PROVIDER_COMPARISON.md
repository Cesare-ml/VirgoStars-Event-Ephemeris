# G3E — Comparison of latitude-event candidates from two providers

**Status: experimental Rust generator code, no astronomical certification (2026-11-18).**

`latitude_provider_compare::compare_latitude_event_providers` accepts two separately supplied implementations of `LatitudeProvider`, the same `BodyId`, a half-open UTC `SearchWindow`, an adaptive depth and separate probe budgets. The function never opens or modifies the original source data, P0 archive, database or offline consumer.

## What is actually compared

The comparison runs the adaptive candidate scanners against both providers for:

| Physical event | Same-kind matching category | Numeric value comparison |
| --- | --- | --- |
| `β=0`, negative to positive | `PlaneAscending` | UTC bracket midpoint only |
| `β=0`, positive to negative | `PlaneDescending` | UTC bracket midpoint only |
| `dβ/dt=0`, afterward northbound | `StationNorthward` | UTC bracket midpoint only |
| `dβ/dt=0`, afterward southbound | `StationSouthward` | UTC bracket midpoint only |
| Positive local maximum of `dβ/dt` | `NorthwardSpeedMaximum` | UTC midpoint and signed °/day |
| Negative local minimum of `dβ/dt` | `SouthwardSpeedMagnitudeMaximum` | UTC midpoint and signed °/day |

For each category, only one-to-one candidates whose bracket midpoints differ by at most `tolerance.time_us` can match. A nearby pair of same-kind candidates within **twice** the time tolerance makes the pairing ambiguous and returns an error rather than an arbitrary pair. Matched speed peaks report reference-minus-primary signed-speed differences and an explicit tolerance verdict; roots have no assigned peak-speed value or artificial verdict. Unmatched primary/reference events are retained, in chronological order. Separate provider budgets prevent the first provider from consuming the other's allowance; within each provider, all three scans share its budget. The report also counts adaptive *interior probes* per event family and provider.

**An empty result, two agreeing empty results or even all observed events matching is not a UTC coverage certificate.** A high-frequency oscillation can hide multiple roots between adaptive quarter probes. The code deliberately supplies no `complete`, `covered` or `verified` flag.

## Direct same-instant sample comparison

`latitude_provider_compare::compare_latitude_provider_samples` independently checks the raw `LatitudeSample` outputs from both providers on **the same UTC grid**, even when no station, node or peak is observed. It takes:

- A half-open UTC `SearchWindow`, with samples at `start_us`, `start_us + step_us`, etc. strictly **before** `end_us`.
- `LatitudeSampleTolerance { latitude_degrees, signed_speed_degrees_per_day }`, both finite and nonnegative; units remain degrees and degrees/day.
- Independent `&mut usize` budgets for each provider, with bounded scan length. Invalid samples, latitude outside ±90°, non-finite residuals and provider failures return explicit errors **without a partial parity report**.

The report gives the number of tested instants, **signed reference-minus-primary latitude and speed residuals** for out-of-tolerance samples, the maximum absolute residual of each coordinate and the UTC instant where each maximum was first observed. Latitude and speed tolerances are evaluated independently. The report deliberately stores disagreements rather than every in-tolerance sample.

This direct check catches a coordinate-frame or velocity mismatch that event-only comparison might miss (for example, two providers returning **zero event candidates** throughout the same interval). It still cannot prove that unsampled instants agree, identify which provider is correct or certify completeness. Direct sample comparison should precede event-candidate matching in an actual ephemeris adapter audit.

## Development-only latitude process adapter (`virgostars.latitude.v1`)

`provider_process::ProcessLatitudeProvider::spawn(path)` now implements `LatitudeProvider` through an explicitly selected **external executable**. It does not load a kernel or a private backend itself and is never used by the offline reader. It shares the existing bounded JSON-lines transport with the longitude adapter, but has a **distinct versioned identity and response format**; do not point it at a `virgostars.longitude.v2` process.

The executable first writes one UTF-8 JSON line with exactly these identity fields (example values are illustrative, **not** verified astronomical facts):

```json
{"protocol":"virgostars.latitude.v1","provider":"YOUR_REVIEWED_PROVIDER","version":"PINNED_VERSION","frame":"geocentric-ecliptic-latitude-degrees","ecliptic_plane":"mean-of-date","position_corrections":"geometric","dynamical_time_scale":"TT","input_time_scale":"utc-posix-microseconds","speed_definition":"signed-d-beta-dt-degrees-per-day","flags":"PINNED_FLAGS","source_license":"REVIEWED_LICENSE","kernel_digest":null,"source_digest":null}
```

Values for `ecliptic_plane`, `position_corrections`, `dynamical_time_scale`, `flags`, `source_license` and optional digests are supplied and **must be audited independently**; strings merely being nonempty does not validate them. In particular, the speed must be the actual signed time derivative of latitude, not longitude speed or an undocumented finite-difference approximation. Two self-reported identities, even if different, do not prove that the providers use different kernels or algorithms.

For each requested sample, the executable reads one line:

```json
{"body":"Mars","utc_microseconds":"-1"}
```

It returns one JSON line with **exactly** the same body and decimal-string UTC/POSIX microseconds:

```json
{"body":"Mars","utc_microseconds":"-1","latitude_degrees":1.25,"latitude_speed_degrees_per_day":-0.125}
```

The example numbers above are **synthetic**. The adapter rejects mismatched body/time echoes, malformed or excessive output (8,192-byte line cap), invalid identities, nonfinite values and latitudes outside ±90°. String timestamps preserve even the full signed `i64` range. Provider errors propagate to the caller rather than returning plausible defaults. The executable's mathematical time conversion, binaries, library dependencies, kernel and dataset licensing are **not** validated by the transport.

Four cross-platform subprocess integration tests compile an independent, standalone Rust mock executable, exercise exact time echoes and negative velocity, refuse incompatible identities and six malformed/faulty reply modes, then run a real two-process sample comparison. They are **transport tests only**, not astronomical goldens. The next scientific step is to implement and separately validate one actual producer for this protocol (and then an independent reference), before adding a command-line scientific review workflow.

## Bounded two-source CLI for experimental audits

The new **development-only** `vseph-compare-latitudes` binary accepts two explicitly chosen `virgostars.latitude.v1` executables and can compare `samples`, observed directional `events` or `both`. No database, P0/CSV/archive reader or importer is involved. This is a transport/test harness, **not** a scientifically approved reviewer or source independence certificate.

```bash
cargo run --locked -p virgostars-events-generator --bin vseph-compare-latitudes -- \
  --mode both --body Mars \
  --start-us 0 --end-us 172800000000 \
  --step-us 86400000000 --precision-us 1000000 \
  --primary-program /path/to/trusted-primary \
  --reference-program /path/to/trusted-reference \
  --latitude-tolerance-deg 0.01 \
  --speed-tolerance-deg-per-day 0.01 \
  --event-time-tolerance-us 600000000 \
  --peak-speed-tolerance-deg-per-day 0.1 \
  --adaptive-depth 1 --max-probes 3000
```

All numeric values are **illustrative test parameters**, not approved planetary accuracy tolerances. Every option is explicit; both providers must self-declare the same ecliptic plane, frame, correction model, dynamical/input time scales and signed-speed definition. The CLI rejects mismatches **before sampling**. It uses bounded UTC grids (max 100,000 points), validated finite thresholds, adaptive depth 1–8, and separately allocated per-provider budgets for the sample and event passes. No automatic data download occurs. Only run executable paths that the operator explicitly trusts; a subprocess is arbitrary local code, and this protocol currently has no built-in wall-clock timeout.

The emitted JSON includes source identities, UTC window/tolerances, sampled coordinate and signed-speed residuals, event candidate matches/unmatched events, per-pass probe counts and explicit flags: `provider_independence_verified:false`, `astronomical_accuracy_verified:false`, `coverage_certified:false`, `safe_to_import:false` **in every case**. Exact UTC integer times and offsets are encoded as **decimal strings** to preserve microsecond precision across JSON/JavaScript consumers.

| Exit | Meaning | JSON stdout |
| --- | --- | --- |
| `0` | No observed discrepancies at sampled instants and configured tolerances; **not scientific agreement** | Complete report |
| `1` | At least one sampled discrepancy or unmatched/discordant event candidate | Complete report |
| `2` | Invalid request, incompatible declared conventions, provider failure, ambiguous scan or exhausted budget | **Empty**; diagnostic on stderr |

Five subprocess CLI tests plus two argument-parser tests exercise the exit rules, complete-versus-atomic reports, genuine dual-process analytical candidate scanning, out-of-tolerance samples and malformed inputs. They use only a synthetic Rust fixture; actual independent source/kernel identity and UTC golden events remain untested.

## Requirements before real astronomical parity

1. Implement two **separately substantiated, license-reviewed** `LatitudeProvider`s. State the ecliptic reference plane (mean/true, epoch/date), geocentric/apparent corrections, time scale, ephemeris/kernel exact version, algorithm and dependencies for **each**. Sharing one upstream kernel through two wrappers is not independent validation.
2. Agree an explicit versioned observation convention and acceptable per-body numerical/time tolerances *before* comparing results. Time tolerances are UTC microseconds; signed-speed tolerances are degrees/day.
3. Use bounded UTC intervals and adequate budgets; record every failure, including provider errors, ambiguous plateaus or densely clustered candidate events. Investigate both unmatched and matched-but-discrepant events. Run additional independently selected sample phases/cadences to challenge aliasing.
4. Compare exact UTC timestamps and signed speeds against **independently authored per-body goldens**, not just the other provider's generated candidates. Review broad coverage only from independent evidence; each family/body UTC window remains `UNKNOWN` until explicitly approved.
5. The experimental CLI exists, but **promotion to a scientific reviewer** requires that the provider's source, setup, numerical precision and testing process first be reviewed. The offline reader must continue to read approved precomputed records, without an astronomical calculation dependency.

## Local synthetic gate

```bash
cargo test --locked -p virgostars-events-generator latitude_provider_compare
cargo test --locked -p virgostars-events-generator --test latitude_process
cargo test --locked -p virgostars-events-generator --test latitude_cli
cargo fmt -p virgostars-events-generator -- --check
cargo test --locked --workspace --quiet
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release --quiet
```

Fifteen deterministic tests exercise both **event-candidate comparison (8)** and **direct sample residuals (7)** using analytical synthetic providers. The latter cover systematic latitude and speed offsets even with no candidate events, independent tolerance verdicts, exact exclusive UTC end, invalid tolerances/samples, provider errors, non-finite residuals and exhausted budgets. The 33 scanner tests plus these 15 comparison tests total **48 synthetic G3E algorithm tests**, now supplemented by **4 external-process protocol tests**. **None are real planetary ephemeris goldens**. See [G3E roadmap](ROADMAP.md), [algorithm inventory](ASTRONOMICAL_EVENT_ALGORITHMS.md), [coverage status](COVERAGE_STATUS.md), and [issue #38](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/38).
