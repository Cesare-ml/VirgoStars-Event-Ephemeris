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

The optional `ProcessLatitudeProvider::spawn_with_args(program, literal_args)` starts an interpreter or native program with **literal argv values**, without `cmd.exe`, a POSIX shell or string substitution. The audit CLI permits repeatable `--primary-arg VALUE` and `--reference-arg VALUE` (up to 16 per provider); this is **process invocation only**, not a change in astronomical semantics. The external programs are trusted code and remain under the operator's control.

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

Six subprocess CLI tests plus three argument-parser tests exercise the exit rules, complete-versus-atomic reports, genuine dual-process analytical candidate scanning, out-of-tolerance samples, literal interpreter argv and malformed inputs. They use only a synthetic Rust fixture; actual independent source/kernel identity and UTC golden events remain untested.

## Experimental VirgoStars-Ephemeris → latitude provider bridge

`tools/vseph_virgostars_latitude_provider.cjs` is a runnable **development-only producer** for `virgostars.latitude.v1`. Unlike the analytical fixtures, it calls the actual `Cesare-ml/VirgoStars-Ephemeris` Node `Engine.calcUt(jdUt, ipl, 257)` (`SEFLG_JPLEPH | SEFLG_SPEED`) and returns **field `xx[1]` latitude** and **field `xx[4]` signed latitude velocity**, with no replacement astronomy in JavaScript. The Node binding delegates to its Rust core. It requires the **operator's separate, compiled** `VirgoStars-Ephemeris/bindings/node/index.js` and **operator-supplied classic DE441 `.eph` file**. Neither codebase downloads a kernel.

The producer explicitly declares the **geocentric apparent true ecliptic and equinox of date**, IAU 2006/2000B orientation, light-time, solar deflection, annual aberration, per-**TDB**-day velocity and **UTC approximated as UT1** in the native `calcUt` path. It allows only Sun, Moon, Mercury, Venus and the *DE441 planet-system barycenters* Mars through Pluto. The labels `Mars`, `Jupiter`, etc. here are consumer aliases to **barycenters**, not guarantees of physical planet-center ephemerides. The input civil-time policy supports `[1900-01-01T00:00:00Z, 2027-06-28T00:00:00Z)` only: historical pre-1972 ΔT polynomial or modern 1972+ IERS leap-second table. Out-of-range inputs fail closed; a provided or predicted future ΔT table is deliberately **not** loaded. Rates are per TDB day, **not** per UTC day. The Node binding's single `f64` Julian date can lose sub-millisecond resolution, so exact-string UTC echo is a *transport integrity guarantee*, not proof of sub-millisecond astronomical timing.

### Preparation on the owner's computer

1. Compile `VirgoStars-Ephemeris/bindings/node` with its documented `npm run build` command; the native `.node` addon must load successfully. This bridge **does not** compile or copy that repository.
2. Select and inspect the exact source Git commit from the local `VirgoStars-Ephemeris` checkout and obtain an **independently trusted** SHA-256 for the classic DE441 file (for example, from an independently reviewed, preserved manifest). The SHA-256 is checked by streaming the file **before emitting the identity**; no multi-GB memory allocation. Recomputing the same file's hash moments before launch is useful for inspection but **does not independently establish that the kernel is authentic**. The source commit is an **operator-supplied claim**; it is not cryptographically measured against the compiled `.node` addon.
3. Confirm the kernel/license provenance separately. The bridge's CPAL-1.0 declaration applies to code, **not automatically to the DE441 data**.

```powershell
# Example paths: adjust to your local checkouts; do not paste secrets.
$ephemeris = 'D:\GitHub\VirgoStars-Ephemeris'
$kernel = 'D:\GitHub\virgo-stars\database\effemeridi\astrodienst\de441.eph'
$sourceCommit = (git -C $ephemeris rev-parse HEAD)
$module = Join-Path $ephemeris 'bindings\node\index.js'

# Inspect the measured hash and compare with a separately reviewed source manifest.
Get-FileHash -Algorithm SHA256 $kernel
$trustedKernelHash = '<64-character lowercase SHA256 verified independently>'

# If you have no independently reviewed manifest, you may instead run a
# TRANSPORT-ONLY exploratory smoke with the local hash shown above:
# $trustedKernelHash = (Get-FileHash -Algorithm SHA256 $kernel).Hash.ToLowerInvariant()
# This establishes only consistency of the same local file, not trusted provenance.

# Owner smoke: checks DE441, fingerprints binding, probes Mars (1960),
# Moon (2000) and Jupiter (2026); outputs an UNCERTIFIED JSON report.
node .\tools\vseph_de441_owner_smoke.cjs --binding $module --kernel $kernel --kernel-sha256 $trustedKernelHash --source-commit $sourceCommit

# Optional create-only report (existing report is never overwritten):
node .\tools\vseph_de441_owner_smoke.cjs --binding $module --kernel $kernel --kernel-sha256 $trustedKernelHash --source-commit $sourceCommit --report "$env:TEMP\vseph-g3e-smoke-20261118.json"

# Lower-level interactive producer when individual sample requests are needed:
node .\tools\vseph_virgostars_latitude_provider.cjs $module $kernel $trustedKernelHash $sourceCommit
```

Example stdin line after the lower-level producer handshake: `{"body":"Mars","utc_microseconds":"1795000000000000"}`; the response must echo the string and supply **finite** latitude/speed. This is not an independent golden test. On Linux/macOS the same script accepts positional arguments in a `node` command. The higher-level owner smoke exits `0` only after three expected UTC/body echoes and finite coordinate checks; it exits `2` **without JSON** on invalid pin, native error or invalid responses. If `--report` is supplied, it uses create-only (`wx`) semantics and refuses an already-existing path. Its `result` contains `native_binary_source_commit_attested:false`, `independent_provider_compared:false`, `astronomical_accuracy_verified:false`, `coverage_certified:false`, `safe_to_import:false` even on success. This owner-machine run is **pending**: the real DE441 and native addon are absent from this Linux checkout.

### Scope of the native binding fingerprint

The bridge now sets `source_digest` to SHA-256 over the following concatenation of raw ASCII/UTF-8 bytes: `virgostars-node-binding-v1`, one NUL byte, the **lowercase SHA-256 hex** of the operator-selected `index.js`, one NUL byte, and the **lowercase SHA-256 hex** of `native/virgostars_ephemeris.node` beside that loader. Both files are stream-hashed **before the JSON identity**; a missing addon fails before handshake. The owner smoke separately recomputes the same local fingerprint and compares it against the handshake. This identifies file bytes observed during the check; it **does not prove** that the addon was compiled from the self-declared Git commit, that native dependencies or compiler flags match, or that the source math and ephemeris are scientifically independent. The kernel has a separate measured, required SHA trust pin.

For a **real cross-provider audit**, point `vseph-compare-latitudes` to Node with literal arguments, and to a separately implemented and scientifically reviewed reference program:

```text
--primary-program node
--primary-arg /absolute/path/to/tools/vseph_virgostars_latitude_provider.cjs
--primary-arg /absolute/path/to/VirgoStars-Ephemeris/bindings/node/index.js
--primary-arg /absolute/path/to/de441.eph
--primary-arg <independently-trusted-lowercase-kernel-SHA256>
--primary-arg <reviewed-40-character-ephemeris-Git-commit>
--reference-program /absolute/path/to/INDEPENDENT-REVIEWED-REFERENCE
```

The rest of the audit CLI's required window/precision/tolerance options still apply. The reference must declare **identical** frame, ecliptic plane, correction and time conventions (not merely similar human descriptions). Comparing two copies of this adapter, or two programs sharing DE441, is **not** an independent reference. A correct handshake, even with a real kernel, is **not a claim of scientific accuracy or coverage**. Its `kernel_digest` is measured and pinned, `version` is operator-declared, and `source_digest` measures local binding bytes but is **not** a source-commit proof. The report remains `safe_to_import:false`.

Run the reproducible **synthetic bridge and owner-smoke tests** with `node --test tools/tests/vseph_virgostars_latitude_provider.test.cjs tools/tests/vseph_de441_owner_smoke.test.cjs`. They supply a tiny **fake** `.eph` file, fake addon bytes and a fake Node `Engine` only to check process plumbing, kernel/binding fingerprints, three calendar epochs, exact UTC echoes, civil-time policy, malformed native replies and create-only reports. **Ten tests** pass on Linux; these are **not** a real DE441 integration, Windows runtime check or source-independent astronomical comparison. A real-kernel owner smoke and independently authored goldens remain pending.

## G3E SHA-pinned coordinate-golden comparison (experimental)

`tools/vseph_review_latitude_goldens.cjs` compares actual Node-bridge **geocentric ecliptic latitude** and **signed latitude speed** at *explicit owner-supplied UTC instants* against a bounded JSON reference file. Unlike the owner smoke's three fixed dates, this accepts **1–256 distinct body/UTC samples** selected by a reference author. It requires separate lowercase SHA-256 pins for **both** the existing DE441 and the reference file and reports the measured binding fingerprint alongside the source identity. It does not invent event times or generate goldens. No new source data is included in this repository.

### Reference file contract

The following is a **synthetic format illustration**, not real planetary values or an independent reference:

```json
{
  "schema": "vseph.g3e.latitude-goldens.v1",
  "reference": {
    "provider": "SYNTHETIC-REFERENCE-NOT-ASTRONOMICAL",
    "version": "fixture-v1",
    "origin": "test-only-manual-fixture",
    "method": "synthetic-constant-function",
    "source_license": "synthetic-test-data",
    "frame": "geocentric-ecliptic-latitude-degrees",
    "ecliptic_plane": "true-ecliptic-and-equinox-of-date (IAU-2006/2000B)",
    "position_corrections": "one-way-light-time;solar-deflection;annual-aberration;date-frame-rotation",
    "dynamical_time_scale": "TDB (approximate from civil UTC; signed rate per TDB day)",
    "input_time_scale": "utc-posix-microseconds",
    "speed_definition": "signed-d-beta-dt-degrees-per-day"
  },
  "samples": [
    {
      "body": "Mars",
      "utc_microseconds": "946684800000000",
      "latitude_degrees": 1.25,
      "latitude_speed_degrees_per_day": -0.125
    }
  ]
}
```

Only *precisely matched conventions* may be compared: the six frame/ecliptic/correction/time/speed fields must **exactly match** the bridge identity, after independent semantic review of their meaning. Numeric latitude values must be finite within ±90°, signed speeds finite, and UTC microseconds canonical decimal strings within the bridge's `[1900-01-01,2027-06-28)` policy. Duplicate body+UTC samples, empty/malformed reference metadata, excessive file size (256 KiB) and more than 256 entries are refused. A reference file can falsely *declare* an independent method, just as a bridge can falsely declare its source revision: **metadata equality and two agreeing numerically different executables do not substantiate scientific independence**.

### Owner invocation (PowerShell, after building the native addon)

From the **event-ephemeris checkout containing this script**:

```powershell
$ephemeris = 'D:\GitHub\VirgoStars-Ephemeris'
$kernel = 'D:\GitHub\virgo-stars\database\effemeridi\astrodienst\de441.eph'
$binding = Join-Path $ephemeris 'bindings\node\index.js'
$commit = (git -C $ephemeris rev-parse HEAD)
$goldens = 'D:\astronomy-reference\reviewed-latitude-goldens.json'

# Obtain BOTH of these hashes from separately reviewed/preserved manifests.
$trustedKernelSha = '<independently-pinned-64-lowercase-hex-sha256-of-DE441>'
$trustedGoldenSha = '<independently-pinned-64-lowercase-hex-sha256-of-reference-json>'

node .\tools\vseph_review_latitude_goldens.cjs --binding $binding --kernel $kernel --kernel-sha256 $trustedKernelSha --source-commit $commit --goldens $goldens --goldens-sha256 $trustedGoldenSha --latitude-tolerance-deg 0.001 --speed-tolerance-deg-per-day 0.0001
```

The example tolerances are **arbitrary CLI placeholders**, **not** scientifically justified error bounds. An actual comparison requires independent author/source/license/provenance review and tolerances justified per body and epoch *before interpreting the results*. An optional `--report <new-path>` saves the same JSON **without overwriting** an existing file.

| Exit | Meaning | JSON stdout |
| --- | --- | --- |
| `0` | All *supplied samples* within **operator-defined** tolerances; does not establish physical accuracy | Complete report |
| `1` | At least one latitude or signed-speed residual beyond tolerance | Complete report, every discrepancy and worst observed residual |
| `2` | Bad SHA pin/reference/format, mismatched frame/time convention, native process error or existing report path | **Empty**; diagnostic on stderr |

The JSON deliberately keeps `reference_independence_scientifically_established:false`, `astronomical_accuracy_verified:false`, `coverage_certified:false`, and `safe_to_import:false` **regardless of exit status**. A matching oracle set establishes only an observed numerical check at its supplied instants; no continuous-time completeness, independent physical correctness, node/station/peak timestamp parity, P0 acceptance or database import follows. The executable's **six tests use synthetic fake Engine and fake `.eph` only**; an owner-run real DE441 and truly independently authored reference vectors remain **NOT RUN**.

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
node --test tools/tests/vseph_virgostars_latitude_provider.test.cjs
node --test tools/tests/vseph_de441_owner_smoke.test.cjs
node --test tools/tests/vseph_review_latitude_goldens.test.cjs
cargo fmt -p virgostars-events-generator -- --check
cargo test --locked --workspace --quiet
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release --quiet
```

Fifteen deterministic tests exercise both **event-candidate comparison (8)** and **direct sample residuals (7)** using analytical synthetic providers. The latter cover systematic latitude and speed offsets even with no candidate events, independent tolerance verdicts, exact exclusive UTC end, invalid tolerances/samples, provider errors, non-finite residuals and exhausted budgets. The 33 scanner tests plus these 15 comparison tests total **48 synthetic G3E algorithm tests**, now supplemented by **4 external-process protocol tests**. **None are real planetary ephemeris goldens**. See [G3E roadmap](ROADMAP.md), [algorithm inventory](ASTRONOMICAL_EVENT_ALGORITHMS.md), [coverage status](COVERAGE_STATUS.md), and [issue #38](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/38).
