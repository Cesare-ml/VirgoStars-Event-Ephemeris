# Rebuilding missing event intervals — source inventory and migration contract

**Status (2026-11-17):** executable Rust sparse-span planner and candidate calculation for motions, tropical sign ingresses and **Moon-only** lunar synod phases, plus a thin optional development adapter to the existing private backend's Swiss position function. Linux synthetic tests pass; **no real astronomical backfill or scientifically verified coverage** is claimed. No production database or original dataset was modified.

The authoritative legacy implementation is in the **private** [`Cesare-ml/virgo-stars`](https://github.com/Cesare-ml/virgo-stars) repository, under `backend/routines/` (not a separate `Cesare-ml/virgostars` repository). These scripts and their helpers must be treated as reference behavior to audit and port, not blindly copied into this public repository, and should never bring credentials or private exports with them.

## Existing generators and exact dependencies

| Legacy script in `backend/routines/` | Referenced calculation entry points | Required future Rust generator | Current stage |
| --- | --- | --- | --- |
| `calcolo-moto-pianeti-per-db.js` | `services/astrology/func.js::getPianetiCaseByNomiElCelesti`, longitude speed from Swiss | Direct/retrograde zero-crossing brackets and source sign at station | **Candidate bracketing implemented** in `events-generator` |
| `calcolo-ingresso-segni-per-db.js` | `services/astrology/ampolla-func.js::getAmpollaCongiunzione` → `func.js` | Tropical sign-boundary crossings, including retrograde re-entry | **Candidate bracketing implemented** in `events-generator` |
| `calcolo-effemeridi-sinodiche-per-db.js` | `services/astrology/sinodi-func.js::getSinodi`, `getFasiSinodichePianetaInferiore/Superiore`, `getFasiLunari`, and other helpers → `func.js` | Body-specific cycles, optional ordered thresholds and source `NULL` vs `[]` | **Moon phase brackets implemented as candidates only**; original legacy sinodic conventions, grouped source rows and all other bodies **not ported**. Script also refers to external `router`/`moment` context |
| `calcolo-aspetti-per-db.js` | `services/astrology/aspetti-func.js`, `func.js`, `#ricerca-dicotomica-func.js`, `ampolla-func.js` | Aspect entry/exact/exit windows and checked orb rules | **Not ported**; not yet a P0 family |
| `calcolo-eclissi-lunari-per-db.js` | Direct Swiss `swe_lun_eclipse_when` with `SEFLG_JPLEPH` | Global lunar eclipse contact times and type | **Not ported**; not yet a P0 family |
| `calcolo-eclissi-solari-per-db.js` | Direct Swiss `swe_sol_eclipse_when_glob` with `SEFLG_JPLEPH` | Global solar eclipse contact times and type | **Not ported**; not yet a P0 family |

**Source review:** files inspected from current `master` on 2026-11-17. SHA-1 blobs, respectively: `2695b4b`, `8094d25`, `b1e760d`, `28f523c`, `6e35631`, `fefd366`. Shared helpers include `func.js` (`2039621`), `sinodi-func.js` (`31900c8`) and `ampolla-func.js` (`cf7f496`). Confirm latest code and scientific conventions when porting; the abbreviated SHA labels are for traceability, not cryptographic release manifests.

The motion script currently selects `true Node`/`osc. Apogee`, scans forward daily from year 0100 to 3000, and bisects longitude-speed sign changes to approximately one second. The ingress script currently targets `osc. Apogee`, scans **backward** from 2040 to 1900, and writes sign/century JSON. Those hard-coded ranges and body filters are **not** a definition of scientific completeness or an appropriate generalized backfill policy. The synod script uses different Moon/inferior/superior/node conventions. Solar/lunar scripts query eclipse-specific native Swiss functions rather than a general longitude crossing.

## Boundary between the reader and generator

The consumer's `virgostars-event-ephemeris` crate only reads verified historical records. It must not calculate missing events. The separate `virgostars-events-generator` **development** crate defines:

- `LongitudeProvider::sample(body, utc_microseconds)` returning a finite geocentric tropical longitude and longitude speed, with documented ephemeris flags, time scale and reference frame.
- `station_candidates(provider, body, SearchWindow)` and `ingress_candidates(...)` producing **brackets**, not authoritative timestamps or source IDs. Positive/negative speed changes, zodiac wraparound and retrograde re-entry are handled; overly large angle jumps and invalid samples fail closed. Scan intervals have a hard upper bound and configurable resolution.
- `lunar_phase_candidates(provider, SearchWindow)` searching the Moon–Sun geocentric tropical longitude offsets 0°/90°/180°/270° for **new-moon, first-quarter, full-moon and last-quarter candidate brackets**. It rejects ambiguous relative-longitude jumps and is not equivalent to the independently validated legacy `getFasiLunari` source rows.
- `detect_sparse_spans(verified_archive, family, body, requested_window, chosen_threshold)` scanning original IDs and timestamps without modifying them. It distinguishes an anomalously long interval between two recorded anchors from **uncertain leading/trailing/no-record** boundaries; the threshold is caller-supplied and is not a certificate that any expected events are actually absent.
- Synthetic deterministic Rust tests for zero crossings, forward and backward ingress, invalid sampling and ambiguous steps, **executed with Cargo on Linux/Rust 1.90**. A scan may miss a pair of crossings within one step; production backfill requires adaptive sampling and independent scientific comparison.

## Standalone Rust commands

The main missing-interval command is `vseph-fill-gaps`. It accepts a local owner-authorized P0 archive and a **separately trusted** whole-file SHA-256, runs a full integrity audit, identifies sparse spans and optionally calculates candidates only inside those spans. Nothing is written to the archive, source ID sequences, PostgreSQL or production endpoints. The output is a **single JSON staging object** (or no stdout if any provider/generation step fails).

```bash
# Read-only gap report; pick a family-specific anomaly threshold explicitly.
cargo run --locked -p virgostars-events-generator --bin vseph-fill-gaps -- \
  --mode plan --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_64_HEX_DIGEST \
  --family synods --body Moon \
  --start-us -1609459200000000 --end-us -1577836800000000 \
  --threshold-us 3888000000000

# Recalculate candidate brackets after planning and separate Swiss setup.
cargo run --locked -p virgostars-events-generator --bin vseph-fill-gaps -- \
  --mode calculate --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_64_HEX_DIGEST \
  --family synods --body Moon \
  --start-us 1513555200000000 --end-us 1546732800000000 \
  --threshold-us 3888000000000 --step-us 21600000000 --precision-us 1000000 \
  --provider-program /path/to/legacy-swiss-position.sh
```

These epoch numbers are **illustrative** and must be checked against the desired approved source window before running. The tool's explicit `--max-steps` (default 100,000, maximum 1,000,000) fails closed before provider launch if the job is too large; `--chunk-us` (default 365 days) subdivides long spans safely. For `--family synods`, **only `--body Moon` is calculable**. All three families can be *planned*, but synods for other planets, nodes, aspects and eclipses still require dedicated science/code and source parity review. Original source row IDs remain present only as anchors in the sparse-span report, never attached to newly calculated candidates. Empty results never establish scientific completeness.

### Windows batch script for known gaps and resumable intervals

`tools/compute-missing-intervals.ps1` (PowerShell 5.1+/7) runs this Rust command in **small UTC slices**, saves UTF-8 JSON only after a successful full calculation and refuses to overwrite a previous artifact. By default only 10 new yearly slices are attempted per invocation; rerun to continue, or adjust `-MaxJobs`. `-Mode plan` needs no Swiss installation; `-Mode calculate` needs an authorized private backend checkout with its existing Node dependencies and Swiss license configured. For instance:

```powershell
# Analyze the two known Moon source discontinuities (1918–1920, 2017–2019).
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -KnownMoonGaps -Mode plan

# Generate Moon-phase brackets through the existing backend's Swiss calculation.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -KnownMoonGaps -Mode calculate -BackendRoot "D:\GitHub\virgo-stars"

# Long ingress gap: bounded, resumable slices; example 8-day threshold is
# only a user-selected investigation rule, not scientifically certified.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -Family ingresses -Body Moon -FromUtc "2099-01-01T00:00:00Z" `
  -ToUtc "4836-01-01T00:00:00Z" -ThresholdDays 8 -SliceYears 1 -MaxJobs 10 -Mode plan
```

The script reads `backend/services/astrology/func.js` through `-BackendRoot` only for calculation, and assigns the private Git revision to `VSEPH_LEGACY_COMMIT`; it does not copy or publish that code. Users who need a different provider can set `-ProviderProgram` and the compatible provider's environment. The script generates no astronomical records until `-Mode calculate` is selected. Existing JSON artifacts are skipped, **not** re-verified or overwritten; handle them as untrusted staging and review them before reuse. For large multi-century gaps, work in small year slices rather than launching one giant Swiss scan.

## Low-level candidate staging command

`cargo run --locked -p virgostars-events-generator --bin vseph-generate-candidates -- --help` documents a **Rust-only executable**. It accepts `--mode stations|ingresses`, `--body`, `--start-us`, `--end-us`, `--step-us`, `--precision-us` and `--provider-program PATH` (the latter an executable adapter, not a shell command). For example, after independently configuring your approved ephemeris adapter:

```bash
cargo run --locked -p virgostars-events-generator --bin vseph-generate-candidates -- \
  --mode stations --body Mars \
  --start-us 1735689600000000 --end-us 1767225600000000 \
  --step-us 86400000000 --precision-us 1000000 \
  --provider-program /path/to/approved-provider-adapter
```

The adapter is a **long-lived, separately deployed JSON-lines process**. Its first stdout line must be its identity (sample below; adapt flags/version to the actual provider), then each request yields exactly one newline-terminated sample. Only stdout JSON is protocol traffic; diagnostics belong on stderr:

```json
{"protocol":"virgostars.longitude.v2","provider":"approved-ephemeris","version":"pinned-version","frame":"geocentric-tropical-ecliptic-degrees","input_time_scale":"utc-posix-microseconds","flags":"documented-ephemeris-flags","kernel_digest":null,"source_digest":null}
```

```json
{"body":"Mars","utc_microseconds":"1735689600000000"}
```

```json
{"body":"Mars","utc_microseconds":"1735689600000000","longitude_degrees":123.456,"longitude_speed_degrees_per_day":0.45}
```

The **v2 protocol uses exact decimal strings** for microsecond instants because source years such as 0100 or 4836 can exceed JavaScript's integer-safe numeric range. The adapter must convert UTC/POSIX to its documented astronomical time scale and coordinate frame. The supplied identity is **self-reported provenance, not scientific authentication**. Wrong frame/time scale, stale body/timestamp, malformed or non-finite samples, ambiguous steps and scan-budget overflow fail closed. The command emits **one JSON object only after the scan finishes**, with `kind: unverified_candidate_brackets`, `coverage_certified: false`, provider identity, interval and brackets. On failure it exits nonzero without partial stdout. No archive, database or source IDs are written.

The thin bridge `tools/providers/legacy-swiss-position.cjs` plus its `.cmd`/`.sh` launchers calls the **existing, separately checked-out** `getPianetiCaseByNomiElCelesti` method with `calcCase=false`; its only astronomical source is that private owner-controlled backend. `VSEPH_LEGACY_FUNC_MODULE` must point to the private absolute `func.js` path and `VSEPH_LEGACY_COMMIT` must be a pinned 40-character commit SHA. The bridge records its own source-file SHA-256 and the optional ephemeris kernel digest from the environment. The historic function accepts UTC seconds, so its bridge rounds input down to one second and **must not be used to claim sub-second accuracy**. Node tests with a synthetic mock backend verify exact high-magnitude signed timestamp echo and invalid-body rejection; they do **not** prove a working Swiss installation or scientific parity. Review Swiss license and installed ephemeris data/flags independently. A VirgoStars-Ephemeris adapter can replace it later with the same protocol.

For now, Swiss can remain an **optional, separate development-time position provider** through an explicitly configured adapter in the existing legacy environment, **subject to a licensing review**. Do not statically link Swiss into the CPAL reader or copy its code/data into this repository. Once [`VirgoStars-Ephemeris`](https://github.com/Cesare-ml/VirgoStars-Ephemeris) offers validated tropical positions, speeds and applicable eclipse methods, implement the same provider interface in Rust and compare results on pinned UTC epochs and source records before selecting it as default. Never silently switch algorithms or merge different provider outputs without provenance.

## Safe gap-repair workflow (not automatic)

1. **Audit** approved source snapshots by family/body/year and identify candidate holes (especially Moon synods 1918–1920 and 2017–2019, Moon sign ingress 2099–4836 and the later Aries-only segment). `missing records` is an investigation flag, not a formula for the exact number of events.
2. **Plan** bounded UTC windows, body lists, provider version/flags/kernel digest, observer/frame/zodiac, scan step and uncertainty tolerance. Reject unsupported bodies or precision before computing.
3. **Generate candidates** into a separately reviewed, immutable staging artifact. Station/ingress and Moon-only longitude phase brackets can now be calculated; other planetary synods, source phase-grouping, aspects and eclipses need family-specific implementations and independent scientific golden tests.
4. **Compare independently** against original rows at overlap boundaries and a second approved ephemeris/oracle; assess timestamps, direction, sign, exact phases/contacts, duplicate keys and whether continuity is truly justified. Never synthesize source IDs as though they were original DB IDs.
5. **Owner review and release gate:** separate record provenance (`original` vs `generated`), approved coverage windows, generator revision and data license. Only after explicit approval should a new versioned VSEPH manifest/archive be constructed and shipped. The immutable 2026-11-10 snapshot must remain unchanged.

The current experimental P0 file represents **three** families only: synods (70,554 reported), motions (266,516), ingresses (43,774). Aspects and eclipses need independent schemas, validated format extensions and explicit API design; the existing P0 decoder **must reject** unrecognized families rather than treating them as present. No live PostgreSQL writes, silent interpolation, synthetic real-data claims, GitHub Actions, publication or automatic repair.
