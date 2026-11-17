# Rebuilding missing event intervals — source inventory and migration contract

**Additional requested event algorithms (2026-11-17):** [exact planetary conjunctions, sextiles, squares, trines, and local maxima of longitudinal speed](ASTRONOMICAL_EVENT_ALGORITHMS.md). `speed_extrema::speed_extremum_candidates` and the optional `speed_extrema::adaptive_speed_extrema_scan` are **experimental Rust-only developer functions** for local maxima of direct speed and minima of signed retrograde speed. The adaptive scan checks quarter points and selectively refines suspicious regions, but has **no CLI/P0/reader integration, completeness proof or astronomical validation**. Generic aspects are developed separately in draft PR #36, not yet on this `main` baseline. The legacy `calcolo-aspetti-per-db.js` is a candidate reference only. `station_candidates` already brackets a stop/reversal and the following direction as one event, while `ingress_candidates` already brackets sign ingress/re-entry. See [ROADMAP G3A–G3D](ROADMAP.md) for explicit work items; do not mistake sampled speed peaks for verified astronomical extrema.

**Where are the exact missing dates?** [Coverage and gaps status](COVERAGE_STATUS.md) lists publicly documented source anomalies and the still-unknown ranges. After generating read-only `--mode plan` staging JSON, run `python3 tools/report_missing_intervals.py --format markdown STAGING.json` (or pass multiple same-P0/family/body/threshold files) to print **UTC inclusive start / exclusive end dates, source anchors, flagged ranges, and unscanned gaps between submitted windows**. A no-flags report is **not** a completeness certificate. Contributors should follow [contributing additional time ranges](CONTRIBUTING_EVENT_RANGES.md).

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

### Optional half-step cadence rescan

For `--mode calculate`, add `--cadence-check half` to rescan **every same chunk** with half the requested sample stride, against the same position provider. Before starting the provider, the CLI budgets the **combined** original and half-step sample count under `--max-steps` (hard cap 1,000,000), and requires `step_us / 2 >= precision_us`. The calculation aborts **without staging JSON** if the finer scan finds more/fewer events, changes any candidate phase/direction/sign label or moves an event midpoint by more than the declared bracket precision. Otherwise it emits an explicitly unverified `cadence_check` object with the checked count and actual half-step stride. Example (add to the calculation command above):

```bash
  --cadence-check half
```

This catches *some* missed pairs of closely spaced events. Synthetic regression: a daily scan with **two** hidden motion stations reports zero candidates; the half-day scan finds both and refuses to stage an apparently empty interval. **It does not prove absence of even faster multiple crossings or scientific correctness**. Repeating with another halved step, testing phases near a chunk edge and comparing genuine source records are still necessary. The separate `vseph-review-candidates` executable checks `cadence_check` metadata consistency but **does not recompute the half-step scan**; it reports `cadence_check_independently_recomputed: false`. Do not use this self-reported heuristic as certified coverage.

### Optional adaptive station-risk scan (Rust-only development check)

For `--mode calculate --family motions`, `--adaptive-station-depth N` with `N` from 1 to 8 probes the speed at the midpoint of **each coarse segment** and recursively subdivides segments when either half has a speed-sign change or when the midpoint speed magnitude is less than half the smaller endpoint magnitude. It then applies the existing zero-crossing bracketer to the resulting leaf segments. This can detect a **pair of stations concealed by same-sign coarse endpoints**, while spending fewer midpoint probes on apparently low-risk sections than indiscriminately halving every segment to maximum depth. Example appended to a motion calculation:

```bash
  --adaptive-station-depth 6
```

The CLI requires `step_us / 2^N >= precision_us`; the `--max-steps` preflight accounts for coarse grid steps and optional full half-step rescans, while **the remaining budget caps additional adaptive midpoint probes** across chunks. Exhaustion aborts without output. The budget is *not* a direct cap on every provider evaluation inside the already bounded root-refinement brackets. When a rescan differs in candidate count, event labels or bracket-midpoint precision, staging fails closed with no JSON output. Otherwise `adaptive_station_check` records `mode: risk_midpoint`, depth and the midpoint/refinement/leaf counts, `matched_candidates` and `coverage_certified: false`. The separate reviewer validates this object's structure and consistency, but reports `adaptive_station_check_independently_recomputed: false`.

**Scientific limitation:** a flat or misleading same-sign speed curve may hide multiple roots without triggering this station-risk heuristic. It is **not interval arithmetic, not proof of complete coverage, not an independent ephemeris**, and not a substitute for a validated smaller uniform step or comparison with source events. This station-specific option applies only to **motion stations**; ingresses and Moon phases have distinct options below, while other synods, aspects and eclipses still lack adaptive checks. It does not modify the P0 dataset or assign new source IDs. Synthetic tests cover missed double stations, probe budget exhaustion, and reviewer metadata rejection; real Swiss calculations and Windows runs have not been validated.

### Optional adaptive tropical-ingress scan

For `--mode calculate --family ingresses`, add `--adaptive-ingress-depth N` (1–8) to check each coarse segment's **midpoint longitude and instantaneous speed**. The checker recursively splits a segment if the midpoint changes tropical sign relative to either endpoint, the longitude speed changes sign across a half-step, or the midpoint moves much closer to a 30° zodiac boundary. The existing `ingress_candidates` algorithm then brackets every detected forward ingress and retrograde re-entry on those finer leaves. The reference example is a synthetic planet starting and ending at 29° Aries within one day, reaching 31° Taurus between them: the coarse scan reports no event; the adaptive scan finds both crossings and **refuses** to stage the misleading empty result.

```bash
  --adaptive-ingress-depth 6
```

The ingress option is separate from the station option: each is valid only for its respective event family. It shares the existing `--max-steps` residual midpoint-probe budget and the `step_us / 2^N >= precision_us` constraint. A mismatch with the initial candidates aborts **without JSON stdout**. A matching result is annotated with `adaptive_ingress_check` (`mode: zodiac_boundary_midpoint`, depth, probes, refined/leaf segments and `coverage_certified: false`); the separate reviewer checks only the self-reported schema and responds `adaptive_ingress_check_independently_recomputed: false`. The Windows operator script accepts `-AdaptiveIngressDepth` and gives its results separate `_ingressAdaptiveN` filenames; it remains **untested on Windows**.

**Caution:** the two-point-plus-midpoint risk heuristic can still miss an even faster pair of crossings, a subtle short excursion, or inconsistent physical samples. It does not prove complete zodiac coverage, validate a provider's calculations, or authorize importing staged events. Real ephemeris parity, scientific acceptance tolerances and owner-source comparisons are still required; other planetary synods, aspects and eclipses have **no adaptive scanner** in this iteration.

### Optional adaptive lunar phase-angle scan

For `--mode calculate --family synods --body Moon`, add `--adaptive-lunar-depth N` (1–8). The checker probes the **Moon–Sun relative angle and relative angular speed** at every coarse segment midpoint and uses three independent warnings:

- Either half-step advances at least 30° in the 0°–360° phase angle, violating the generator's existing safe cadence.
- Two measured half-step advances together exceed the coarse modulo-360 advance, revealing a possible hidden revolution.
- A speed-based estimate for the full step exceeds the same 30° safety threshold, including an alias where even both half-step angles repeat after a complete revolution.

If any warning triggers, candidate generation **fails closed without JSON stdout**; it does not infer or silently manufacture missing phases. Otherwise the algorithm subdivides intervals with a suspect speed peak or approach to a 0°/90°/180°/270° boundary, bracket-checks the leaves and compares their phase identities and event midpoint precision to the original scan. Example appended to a Moon calculation:

```bash
  --adaptive-lunar-depth 6
```

This produces `adaptive_lunar_check` (`mode: phase_angle_midpoint`, depth, midpoint/refinement/leaf counts, `matched_candidates`, `coverage_certified:false`) only for actually scanned gaps. The reviewer validates its structure but does **not** independently repeat the lunar rescan (`adaptive_lunar_check_independently_recomputed:false`). The scan shares the residual midpoint budget, `step_us / 2^N >= precision_us` constraint, and Windows PowerShell option `-AdaptiveLunarDepth` with separate `_lunarAdaptiveN` staging filenames. Real Swiss/DE441 execution and Windows integration remain **untested**.

**Scientific boundary:** using a provider's instantaneous speeds to detect aliasing is not rigorous integration or proof that a fast excursion cannot hide between three samples. This is **Moon quarters only**; original `sinodi` rows for other planets are not reimplemented. Historical phase source semantics, high-precision real calculations, source parity and independent coverage approval remain pending.

### Independent structural review before keeping staging JSON

The separate `vseph-review-candidates` Rust executable takes an **existing** output file and reopens the original immutable P0 through the independently supplied trusted SHA-256. It re-audits all archive blocks, checks the staged dataset identity, **recalculates every sparse interval and source ID from recorded P0 rows**, and validates all proposed brackets against their claimed interval, family, body, phase/sign/direction fields and declared search precision. It rejects unexpected JSON keys (including forged `source_id` fields), invalid provider settings and duplicate/out-of-order brackets. The producer's SHA-256 field must never be used as the reviewer's trust anchor.

```bash
# First produce a JSON file explicitly, then use an independently trusted digest.
cargo run --locked -p virgostars-events-generator --bin vseph-review-candidates -- \
  --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_64_HEX_DIGEST \
  --staging /path/to/previously-generated-staging.json
```

Success emits a small report with `staging_structure_verified: true`, `source_archive_authenticated: true`, `brackets_touching_recorded_anchors` and **still** `coverage_certified: false`, `astronomical_accuracy_verified: false`, `safe_to_import: false`. Such a check catches altered reports and structural contradictions but **cannot independently confirm the astronomy, the original source's legacy grouping, correct scan cadence or genuine absence of events**. It neither assigns source IDs nor builds a revised archive. The producer now includes `calculation_settings` (step, precision, chunk and budget) to make bracket-width checks possible; older staging files without these settings require regeneration to pass the strict schema. Structurally reviewed staging is **not** publishable event data. A separate scientist/source-parity review and owner approval remain mandatory.

### Optional second-provider numerical crossing check

For a calculated staging file containing candidates, `vseph-review-candidates` additionally accepts `--reference-provider-program EXEC`, a **separately installed** JSONL-v2 longitude provider (the same interface as the generator). It evaluates the endpoints of every candidate against this provider, independently of the original calculation process:

- **Motions:** checks an actual longitude-speed sign change in the stated direct/retrograde direction.
- **Ingresses:** checks a single adjacent tropical-sign boundary crossing, sign after the crossing, and direct/retrograde sense, including the 0°/360° wrap.
- **Moon phases:** checks an advancing Moon–Sun relative longitude crossing of the declared 0°/90°/180°/270° target, with an under-30° local angle progression.

```bash
cargo run --locked -p virgostars-events-generator --bin vseph-review-candidates -- \
  --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_64_HEX_DIGEST \
  --staging /path/to/previously-generated-staging.json \
  --reference-provider-program /path/to/separate-reference-provider
```

The reference must announce a **different** provider identity/settings from the source-stage metadata; an exact match aborts. The calculation is limited to **10,000 brackets per invocation**. Any inconsistent crossing aborts with no JSON stdout. A successful report adds `numerical_crossings_checked`, `reference_crossings_consistent: true` and the declared `reference_provider` identity, while retaining `reference_independence_scientifically_established: false`, `astronomical_accuracy_verified: false` and `safe_to_import: false`. **Crucial:** distinct self-declared identities do not prove actual independence, reference-provider authenticity, sub-second accuracy, missed events between samples, or historical source parity. A real independent implementation, signed/pinned software and golden source comparisons remain prerequisites for scientific release. In particular, the legacy Swiss adapter rounds positions to whole UTC seconds; a one-second candidate bracket may fail against a differently rounded reference even when both calculations are reasonable, requiring a documented tolerance/precision policy before promotion.

#### Optional SHA-256 pin of the reference executable

To bind a reference run to an **operator-trusted local executable file**, append `--reference-provider-sha256 TRUSTED_64_LOWERCASE_HEX_DIGEST` alongside `--reference-provider-program`. The reviewer streams the selected regular non-symlink file (at most 512 MiB) through SHA-256 **before starting it**, refuses wrong or malformed digests without JSON stdout, and includes `reference_provider_digest_authenticated:true` and `reference_provider_executable_sha256` in a successful report. Without this option the boolean is false and the digest field is null. The digest must be obtained and reviewed independently, never copied from the provider's self-description or a staging file.

```bash
# Get the reviewed digest separately: sha256sum /path/to/separate-reference-provider
cargo run --locked -p virgostars-events-generator --bin vseph-review-candidates -- \
  --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_ARCHIVE_DIGEST \
  --staging /path/to/calculated-staging.json \
  --reference-provider-program /path/to/separate-reference-provider \
  --reference-provider-sha256 YOUR_SEPARATELY_TRUSTED_EXECUTABLE_SHA256
```

**Scope of the pin is deliberately narrow:** it authenticates only the bytes at that executable path during the check, not any `.cmd`/Node/Python wrapper's dependencies, imported source, dynamically loaded libraries, ephemeris kernel, environment, accuracy, or file replacement between verification and launch. Keep those components/version/flags pinned and independently audited for a meaningful scientific comparison. A matching SHA is **not a signature of the developer, evidence of independent computations, or permission to import events**.

### Optional second-provider full-span rescan

The endpoint-only check above cannot discover additional events outside the staged brackets. To *also* search **all independently authenticated P0 sparse spans**, supply a positive, strictly **finer** scan interval using `--reference-rescan-step-us`. The rescan is valid only in `calculate` mode, requires `--reference-provider-program` and at least one sparse span, and allows an originally **empty candidate list**. The selected step must be between the producer's bracket precision and its original step (exclusive upper bound). Example with a **synthetic** 12-hour rescan:

```bash
cargo run --locked -p virgostars-events-generator --bin vseph-review-candidates -- \
  --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_64_HEX_DIGEST \
  --staging /path/to/calculated-staging.json \
  --reference-provider-program /path/to/separate-reference-provider \
  --reference-rescan-step-us 43200000000
```

Before the reference process is launched, the reviewer preflights a maximum **100,000 aggregate grid steps across all source spans**, in addition to the existing 10,000-candidate limit; samples required for numerical bisection are not included in the grid-step cap. It searches the same three implemented families (motion stations, tropical sign ingress/re-entry, Moon quarters), then matches all reference discoveries against staged candidates by original span index, event labels and bracket-midpoint separation **no greater than the producer's declared precision**. A missing event, extra event, time/label disagreement, numerical ambiguity, zero/too-coarse rescan step or exceeded budget exits nonzero with no JSON stdout. The result reports `reference_rescan_performed`, `reference_rescan_step_us`, `reference_rescan_grid_steps`, `reference_rescan_candidates_matched` and `reference_rescan_correspondence_consistent`; absent rescan metrics are JSON null, except the opt-in boolean fields which are false. Existing output fields and unverified status are preserved.

Synthetic tests prove that two Mars stations or two zodiac crossings can hide inside a coarse daily interval with no initial candidates, yet are exposed by a three-hour reference scan. **This is a finite-grid cross-check, not independent proof of physical continuity, genuine provider independence, scientific precision or completeness.** Even two providers with different self-reported identities can share a common error; multiple short crossings can remain hidden between *both* grids. A real independently reviewed reference, pinned kernel/version/time-scale and owner-approved scientific parity are required. The Windows batch script leaves the rescan off by default but exposes it explicitly with `-ReferenceRescanStepHours` (see below).

### Optional SHA-256-bound reference timestamp comparison

For developer-only real Swiss/DE441 golden-event checks **without bundling real ephemeris data**, `vseph-review-candidates` also accepts a separately prepared JSON event list and its **independently trusted SHA-256** (never taken from the staging report). An optional timestamp tolerance defaults to **0** and cannot exceed the candidate bracket precision:

```bash
cargo run --locked -p virgostars-events-generator --bin vseph-review-candidates -- \
  --archive /path/to/authorized.p0 --sha256 YOUR_TRUSTED_ARCHIVE_DIGEST \
  --staging /path/to/previously-generated-staging.json \
  --reference-events /path/to/independent-event-list.json \
  --reference-events-sha256 YOUR_SEPARATELY_TRUSTED_REFERENCE_DIGEST \
  --reference-tolerance-us 1000000
```

Reference JSON schema `vseph-reference-events-v1`, illustrated with **purely synthetic numbers** (not real Mars event times):

```json
{
  "schema": "vseph-reference-events-v1",
  "source": "independently-reviewed-ephemeris-implementation",
  "version": "pinned-implementation-and-flags-version",
  "family": "ingresses",
  "body": "Mars",
  "requested_window": { "start_us": 0, "end_us": 172800000000 },
  "sparse_spans": [{ "start_us": 0, "end_us": 172800000000 }],
  "events": [{ "gap_index": 0, "at_us": 43200000000, "sign_after_index": 1, "retrograde": false }]
}
```

`requested_window` and *all* `sparse_spans` must match the authenticated staging. For motion events, replace ingress sign/retrograde fields with `direction_after: "direct"|"retrograde"`; for Moon quarters use `phase: "new_moon"|"first_quarter"|"full_moon"|"last_quarter"`. Events must be strictly sorted and unique by `(gap_index, at_us)`, remain within their sparse span and use signed UTC/POSIX microseconds. The reviewer compares **every** reference event one-to-one with staged candidates by order, gap index and family-specific labels, and requires the reference instant inside `[bracket_start_us - tolerance, bracket_end_us + tolerance]`. Missing/extra events, invalid labels, wrong source window, forged keys, incorrect file SHA or an excessive tolerance **fail closed with empty stdout**. Limits: 16 MiB JSON file and 10,000 events per review.

Successful report fields: `reference_event_digest_authenticated:true`, `reference_event_timestamps_matched`, `reference_events_correspondence_consistent:true`, **but** `reference_events_scientifically_independent:false`, `astronomical_accuracy_verified:false`, `coverage_certified:false`, `safe_to_import:false`. The trusted hash authenticates bytes, **not** the astronomy, alleged reference independence, or completeness of claimed reference spans. A reference file copied from the stage can trivially pass and is **not an independent oracle**. The reviewer does not generate trusted reference data, and the Windows batch script does not auto-run this optional scientific comparison. Supply a separately authored file for each stage and run this check explicitly; never commit owner data without distribution approval.

For a timestamp comparison, the report now also includes **descriptive UTC microsecond measurements**: `reference_timestamp_tolerance_us` (the operator-selected limit), `reference_max_abs_midpoint_offset_us` (maximum absolute difference between a reference event's instant and the integer midpoint of its staged bracket), `reference_max_outside_bracket_us` (maximum distance beyond a bracket endpoint, *at most* the accepted tolerance), and `reference_events_requiring_tolerance` (count of reference instants outside strict brackets yet still accepted by tolerance). These fields are **JSON null when no reference event file was checked**, and are zero where applicable when all checked differences are zero. Midpoint offsets describe the *location of references relative to the staging bracket*, not the true numerical error of an event root, the reference provider's physical precision or a statistical uncertainty interval. Do not use this report alone as proof of scientific accuracy.

### Windows batch script for known gaps and resumable intervals

`tools/compute-missing-intervals.ps1` (PowerShell 5.1+/7) runs these **two** Rust commands in **small UTC slices**. It first generates the report to a temporary file, then calls the independent structural reviewer; only if both succeed is the UTF-8 JSON moved to its final staging location. Optional `-VerifyHalfStep` in `-Mode calculate` enables the producer's finer-cadence scan; `-AdaptiveStationDepth 1..8` enables the **motion-only** speed-risk check; `-AdaptiveIngressDepth 1..8` independently enables the **ingress-only** boundary check; `-AdaptiveLunarDepth 1..8` independently enables the **Moon-only** phase-alias check; optional `-ReferenceProviderProgram` additionally requires numerical crossing agreement before saving new outputs. **`-ReferenceProviderSha256 DIGEST`** optionally pins the selected reference executable bytes and validates the reviewer's digest report; the filename receives `_sha` plus the first 12 digest characters. **`-ReferenceRescanStepHours N`** enables the newly added full-span second-provider scan only with `-Mode calculate -ReferenceProviderProgram`, with a positive integer `N` strictly smaller than `-StepHours`. The independent reviewer also enforces its original precision and **100,000 total reference grid steps**; an empty-sparse-span window cannot use this option. The script checks all rescan report counts and flags before saving. **Separate `_halfstep` / `_adaptiveN` / `_ingressAdaptiveN` / `_lunarAdaptiveN` / `_reference_shaXXXXXXXXXXXX_rescanNh` filenames** distinguish these check modes from ordinary staging, without overwriting existing artifacts. Previously present artifacts are never overwritten **and are skipped without re-review**. By default only 10 new yearly slices are attempted per invocation; rerun to continue, or adjust `-MaxJobs`. `-Mode plan` needs no Swiss installation; `-Mode calculate` needs either a compatible independent `-ProviderProgram` executable, **or** an authorized private backend checkout with its existing Node dependencies and Swiss license configured for the default legacy adapter. For instance:

```powershell
# Analyze the two known Moon source discontinuities (1918–1920, 2017–2019).
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -KnownMoonGaps -Mode plan

# Generate Moon-phase brackets through the existing backend's Swiss calculation.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -KnownMoonGaps -Mode calculate -BackendRoot "D:\GitHub\virgo-stars" `
  -VerifyHalfStep

# Long ingress gap: bounded, resumable slices; example 8-day threshold is
# only a user-selected investigation rule, not scientifically certified.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -Family ingresses -Body Moon -FromUtc "2099-01-01T00:00:00Z" `
  -ToUtc "4836-01-01T00:00:00Z" -ThresholdDays 8 -SliceYears 1 -MaxJobs 10 -Mode plan

# Motion-station calculation: adaptive probe check in addition to half-step.
# This is still unverified staging; choose an appropriate source and interval.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -Family motions -Body Mars -FromUtc "2020-01-01T00:00:00Z" `
  -ToUtc "2021-01-01T00:00:00Z" -ThresholdDays 45 -Mode calculate `
  -BackendRoot "D:\GitHub\virgo-stars" -VerifyHalfStep -AdaptiveStationDepth 6

# Tropical ingress calculation with its own adaptive re-entry safety check.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -Family ingresses -Body Mars -FromUtc "2020-01-01T00:00:00Z" `
  -ToUtc "2021-01-01T00:00:00Z" -ThresholdDays 45 -Mode calculate `
  -BackendRoot "D:\GitHub\virgo-stars" -VerifyHalfStep -AdaptiveIngressDepth 6

# Moon-only phase-alias check during a bounded known-gap calculation.
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -KnownMoonGaps -Mode calculate -BackendRoot "D:\GitHub\virgo-stars" `
  -VerifyHalfStep -AdaptiveLunarDepth 6

# Independently rescan every source gap at 3-hour cadence after an initial
# 6-hour calculation; distinct providers/kernels must be reviewed separately.
# In Windows PowerShell, obtain a vetted executable digest beforehand:
# (Get-FileHash 'D:\ephemerides\reference-provider.exe' -Algorithm SHA256).Hash.ToLowerInvariant()
.\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
  -TrustedSha256 "<trusted 64-char lowercase digest>" -OutputDir "D:\vseph-staging" `
  -Family ingresses -Body Mars -FromUtc "2020-01-01T00:00:00Z" `
  -ToUtc "2021-01-01T00:00:00Z" -Mode calculate -ThresholdDays 45 `
  -ProviderProgram "D:\ephemerides\generation-provider.exe" -StepHours 6 `
  -ReferenceProviderProgram "D:\ephemerides\reference-provider.exe" `
  -ReferenceProviderSha256 "<independently trusted lowercase executable digest>" `
  -ReferenceRescanStepHours 3
```

The script reads `backend/services/astrology/func.js` through `-BackendRoot` only for calculation, and assigns the private Git revision to `VSEPH_LEGACY_COMMIT`; it does not copy or publish that code. Users who need a different provider can set `-ProviderProgram` and the compatible provider's environment. The script generates no astronomical records until `-Mode calculate` is selected. Existing JSON artifacts are skipped, **not** re-verified or overwritten; handle them as untrusted staging and review them before reuse. For large multi-century gaps, work in small year slices rather than launching one giant Swiss scan. The Windows PowerShell integration has **not yet been executed** on a Windows host.

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
