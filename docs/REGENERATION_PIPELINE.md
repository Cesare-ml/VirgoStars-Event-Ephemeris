# Rebuilding missing event intervals — source inventory and migration contract

**Status (2026-11-17):** design + first provider-neutral Rust candidate detectors; **not** a completed astronomical regeneration, backfill or independently verified coverage report. No production database or original dataset was modified.

The authoritative legacy implementation is in the **private** [`Cesare-ml/virgo-stars`](https://github.com/Cesare-ml/virgo-stars) repository, under `backend/routines/` (not a separate `Cesare-ml/virgostars` repository). These scripts and their helpers must be treated as reference behavior to audit and port, not blindly copied into this public repository, and should never bring credentials or private exports with them.

## Existing generators and exact dependencies

| Legacy script in `backend/routines/` | Referenced calculation entry points | Required future Rust generator | Current stage |
| --- | --- | --- | --- |
| `calcolo-moto-pianeti-per-db.js` | `services/astrology/func.js::getPianetiCaseByNomiElCelesti`, longitude speed from Swiss | Direct/retrograde zero-crossing brackets and source sign at station | **Candidate bracketing implemented** in `events-generator` |
| `calcolo-ingresso-segni-per-db.js` | `services/astrology/ampolla-func.js::getAmpollaCongiunzione` → `func.js` | Tropical sign-boundary crossings, including retrograde re-entry | **Candidate bracketing implemented** in `events-generator` |
| `calcolo-effemeridi-sinodiche-per-db.js` | `services/astrology/sinodi-func.js::getSinodi`, `getFasiSinodichePianetaInferiore/Superiore`, `getFasiLunari`, and other helpers → `func.js` | Body-specific cycles, optional ordered thresholds and source `NULL` vs `[]` | **Not ported**; current script also refers to external `router`/`moment` context |
| `calcolo-aspetti-per-db.js` | `services/astrology/aspetti-func.js`, `func.js`, `#ricerca-dicotomica-func.js`, `ampolla-func.js` | Aspect entry/exact/exit windows and checked orb rules | **Not ported**; not yet a P0 family |
| `calcolo-eclissi-lunari-per-db.js` | Direct Swiss `swe_lun_eclipse_when` with `SEFLG_JPLEPH` | Global lunar eclipse contact times and type | **Not ported**; not yet a P0 family |
| `calcolo-eclissi-solari-per-db.js` | Direct Swiss `swe_sol_eclipse_when_glob` with `SEFLG_JPLEPH` | Global solar eclipse contact times and type | **Not ported**; not yet a P0 family |

**Source review:** files inspected from current `master` on 2026-11-17. SHA-1 blobs, respectively: `2695b4b`, `8094d25`, `b1e760d`, `28f523c`, `6e35631`, `fefd366`. Shared helpers include `func.js` (`2039621`), `sinodi-func.js` (`31900c8`) and `ampolla-func.js` (`cf7f496`). Confirm latest code and scientific conventions when porting; the abbreviated SHA labels are for traceability, not cryptographic release manifests.

The motion script currently selects `true Node`/`osc. Apogee`, scans forward daily from year 0100 to 3000, and bisects longitude-speed sign changes to approximately one second. The ingress script currently targets `osc. Apogee`, scans **backward** from 2040 to 1900, and writes sign/century JSON. Those hard-coded ranges and body filters are **not** a definition of scientific completeness or an appropriate generalized backfill policy. The synod script uses different Moon/inferior/superior/node conventions. Solar/lunar scripts query eclipse-specific native Swiss functions rather than a general longitude crossing.

## Boundary between the reader and generator

The consumer's `virgostars-event-ephemeris` crate only reads verified historical records. It must not calculate missing events. The separate `virgostars-events-generator` **development** crate defines:

- `LongitudeProvider::sample(body, utc_microseconds)` returning a finite geocentric tropical longitude and longitude speed, with documented ephemeris flags, time scale and reference frame.
- `station_candidates(provider, body, SearchWindow)` and `ingress_candidates(...)` producing **brackets**, not authoritative timestamps or source IDs. Positive/negative speed changes, zodiac wraparound and retrograde re-entry are handled; overly large angle jumps and invalid samples fail closed. Scan intervals have a hard upper bound and configurable resolution.
- Synthetic deterministic Rust tests for zero crossings, forward and backward ingress, invalid sampling and ambiguous steps, **executed with Cargo on Linux/Rust 1.90**. A scan may miss a pair of crossings within one step; production backfill requires adaptive sampling and independent scientific comparison.

## Runnable Rust staging command (motion/ingress only)

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
{"protocol":"virgostars.longitude.v1","provider":"approved-ephemeris","version":"pinned-version","frame":"geocentric-tropical-ecliptic-degrees","input_time_scale":"utc-posix-microseconds","flags":"documented-ephemeris-flags","kernel_digest":null}
```

```json
{"body":"Mars","utc_microseconds":1735689600000000}
```

```json
{"body":"Mars","utc_microseconds":1735689600000000,"longitude_degrees":123.456,"longitude_speed_degrees_per_day":0.45}
```

The adapter must convert the input UTC/POSIX instant to its documented astronomical time scales, handle leap seconds/calendar and use the declared geocentric tropical coordinate system. The supplied identity is **self-reported provenance, not scientific authentication**. Wrong frame/time scale, stale body/timestamp, malformed or non-finite samples, ambiguous steps and scan-budget overflow fail closed. The command emits **one JSON object only after the scan finishes**, with `kind: unverified_candidate_brackets`, `coverage_certified: false`, provider identity, interval and brackets. On failure it exits nonzero without partial stdout. No archive, database or source IDs are written. A separate independently licensed Swiss adapter can implement the same protocol now; a VirgoStars-Ephemeris adapter can replace it later. **Neither adapter is bundled or scientifically validated here**, so this is a functional staging interface, not automatic real-data completion.

For now, Swiss can remain an **optional, separate development-time position provider** through an explicitly configured adapter in the existing legacy environment, **subject to a licensing review**. Do not statically link Swiss into the CPAL reader or copy its code/data into this repository. Once [`VirgoStars-Ephemeris`](https://github.com/Cesare-ml/VirgoStars-Ephemeris) offers validated tropical positions, speeds and applicable eclipse methods, implement the same provider interface in Rust and compare results on pinned UTC epochs and source records before selecting it as default. Never silently switch algorithms or merge different provider outputs without provenance.

## Safe gap-repair workflow (not automatic)

1. **Audit** approved source snapshots by family/body/year and identify candidate holes (especially Moon synods 1918–1920 and 2017–2019, Moon sign ingress 2099–4836 and the later Aries-only segment). `missing records` is an investigation flag, not a formula for the exact number of events.
2. **Plan** bounded UTC windows, body lists, provider version/flags/kernel digest, observer/frame/zodiac, scan step and uncertainty tolerance. Reject unsupported bodies or precision before computing.
3. **Generate candidates** into a separately reviewed, immutable staging artifact. For stations/ingresses the initial generic bracketing code is now present; synods, aspects and eclipses need family-specific implementations and golden tests.
4. **Compare independently** against original rows at overlap boundaries and a second approved ephemeris/oracle; assess timestamps, direction, sign, exact phases/contacts, duplicate keys and whether continuity is truly justified. Never synthesize source IDs as though they were original DB IDs.
5. **Owner review and release gate:** separate record provenance (`original` vs `generated`), approved coverage windows, generator revision and data license. Only after explicit approval should a new versioned VSEPH manifest/archive be constructed and shipped. The immutable 2026-11-10 snapshot must remain unchanged.

The current experimental P0 file represents **three** families only: synods (70,554 reported), motions (266,516), ingresses (43,774). Aspects and eclipses need independent schemas, validated format extensions and explicit API design; the existing P0 decoder **must reject** unrecognized families rather than treating them as present. No live PostgreSQL writes, silent interpolation, synthetic real-data claims, GitHub Actions, publication or automatic repair.
