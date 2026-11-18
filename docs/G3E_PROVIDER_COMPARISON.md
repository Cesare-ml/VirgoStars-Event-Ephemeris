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

## Requirements before real astronomical parity

1. Implement two **separately substantiated, license-reviewed** `LatitudeProvider`s. State the ecliptic reference plane (mean/true, epoch/date), geocentric/apparent corrections, time scale, ephemeris/kernel exact version, algorithm and dependencies for **each**. Sharing one upstream kernel through two wrappers is not independent validation.
2. Agree an explicit versioned observation convention and acceptable per-body numerical/time tolerances *before* comparing results. Time tolerances are UTC microseconds; signed-speed tolerances are degrees/day.
3. Use bounded UTC intervals and adequate budgets; record every failure, including provider errors, ambiguous plateaus or densely clustered candidate events. Investigate both unmatched and matched-but-discrepant events. Run additional independently selected sample phases/cadences to challenge aliasing.
4. Compare exact UTC timestamps and signed speeds against **independently authored per-body goldens**, not just the other provider's generated candidates. Review broad coverage only from independent evidence; each family/body UTC window remains `UNKNOWN` until explicitly approved.
5. Add a programmatic CLI/reviewer integration only after the provider's source, setup, numerical precision and testing process have been reviewed. The offline reader must continue to read approved precomputed records, without an astronomical calculation dependency.

## Local synthetic gate

```bash
cargo test --locked -p virgostars-events-generator latitude_provider_compare
cargo fmt -p virgostars-events-generator -- --check
cargo test --locked --workspace --quiet
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release --quiet
```

Fifteen deterministic tests exercise both **event-candidate comparison (8)** and **direct sample residuals (7)** using analytical synthetic providers. The latter cover systematic latitude and speed offsets even with no candidate events, independent tolerance verdicts, exact exclusive UTC end, invalid tolerances/samples, provider errors, non-finite residuals and exhausted budgets. **These are not real planetary ephemeris goldens**. The 33 scanner tests plus these 15 comparison tests total **48 synthetic G3E tests**. See [G3E roadmap](ROADMAP.md), [algorithm inventory](ASTRONOMICAL_EVENT_ALGORITHMS.md), [coverage status](COVERAGE_STATUS.md), and [issue #38](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/38).
