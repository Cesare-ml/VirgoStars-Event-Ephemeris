# Public API contracts — draft v0

**Design only; no released package.** Runtime must be independent of database and astronomical engines. All implementations must yield the same events and boundaries from one VSEPH dataset.

## Canonical Rust model

Suggested (not yet frozen) types:
- `EventEphemeris::open(path | bytes) -> Result<Reader>` and `Reader::metadata()` / `validate()`.
- `BodyId` and `EventFamily` have stable numeric identifiers. Names such as 'Moon', 'Mercury', 'true Node' and 'osc. Apogee' are aliases, not inferred identifiers.
- `SynodHandle` with `body()`, `start()`, `phases()`, `end_exclusive()`, `next()`, `previous()`, `phase_at(t)`. The end comes from the **next start** if available; otherwise unknown.
- `MotionEvent`: body, instant, direction after event, sign at station.
- `IngressEvent`: body, instant, entered sign.
- Errors: `UnsupportedVersion`, `CorruptDataset`, `IntegrityFailure`, `UnknownBody`, `MissingCoverage`, `OutOfRange`, `InvalidInstant` and `InvalidTimeZone`; distinguish missing event from proven absence within covered range.

## Operations required for backend parity

| Family | Methods | Semantics |
| --- | --- | --- |
| Synods | `at(body,t)`, `atAll(t,bodies?)`, `next(body,t)`, `previous(body,t)`, `between(start,end,bodies?)` | Anchor last <= t; next/previous strictly beyond/behind t; [start,end) range |
| Synod handles | `next()` / `previous()`, `phaseAt(t)`, `events()` | Traverse same body and classify exact phase boundaries; no embedded DB query |
| Lunar | `lunarEvents(year,month,timeZone='UTC')`, `cyclesOverlapping(start,end)` | Return all phases falling within window (not only three cycles) |
| Motions | `onDate(date,tz)`, `between(start,end,bodies?)`, `next`, `previous`, `stateAt(body,t)`, `stateAll(t)` | Most recent direct/retrograde station at/before query timestamp |
| Ingresses | `onDate(date,tz)`, `between(start,end,bodies?)`, `next`, `previous`, `signAt(body,t)`, `lunarEvents(year,month,tz)` | Latest valid ingress determines sign, including retrograde re-entries |
| Unified | `events.between(start,end,filter?,limit?)`, `events.onDate(date,tz,filter?)` | Stable chronological merge of all three families, with event type and body |

`onDate` accepts local ISO calendar dates (e.g. 2030-03-31) and IANA zone names; convert to [UTC start of local day, UTC start of next local day). No ambiguous local timestamp parsing. Functions that accept an instant require an explicit `Z` or offset. Expose a Rust UTC-range primitive; wrappers may use a versioned timezone implementation.

## Date, boundary and failure policy

- A query exactly at an event instant includes it in the current state and in a [start,end) range beginning at that instant. Next/previous skip exact equality.
- Return `null`/`None` only for a proven no-neighbor condition within described coverage, and a structured coverage/error status otherwise. API must not manufacture a station or phase from interpolation.
- Phase naming is **not the same as ordinary motion**. For supported planets D1/R2/R3/D4 phases have explicitly defined threshold semantics; Moon uses growing/waning quarter phases; nodes/apogees may have only an anchor. An absent phase must not be classified as D1.
- The old backend sometimes uses lower-case `d1/r2/r3/d4` and other implementations uppercase. Canonical public values proposed: uppercase codes, with an **optional legacy adapter** preserving original case.
- A return value should include `datasetVersion` and `coverage` when queries depend on incomplete data.

## Python-first minimal API contract — highest delivery priority (NOT IMPLEMENTED)

The project's first **usable** deliverable is the Python synod/new-moon query, **before** JS/TS and before full VSEPH v1. Authoritative schema, packaging and tests: [PYTHON_FIRST_MVP.md](PYTHON_FIRST_MVP.md). Delivery milestones: [DELIVERY_MILESTONES.md](DELIVERY_MILESTONES.md).

**Proposed package:** local distribution `virgostars-event-ephemeris`; import as `virgostars_events`. Final public package naming remains subject to PyPI name availability and approved publication. Implementation should use PyO3/maturin over the **existing Rust** reader, not reimplement the event decoder in Python.

```python
from virgostars_events import VirgoStarsEvents

reader = VirgoStarsEvents.open("local-dataset.vseph-p0", expected_sha256="<trusted digest>")
snapshot = reader.get_synods_at("2026-11-11T12:00:00Z")
planetary_cycles = snapshot["synods"]  # dict keyed by the bodies in this dataset
moon = snapshot["new_moon"]             # same recorded Moon cycle data as synods["Moon"]
current_mercury_phase = planetary_cycles["Mercury"]["phase"]["code"]
```

`get_synods_at(X)` must perform **one latest recorded synod `start <= X` per indexed body** (not "all events during the calendar day"), with original source row ID and full phase timestamp array, normalized ISO UTC timestamps, metadata identifying the dataset, and explicit unknown/coverage status. Default body set comes from the archive; a later optional subset parameter is allowed if it preserves identical semantics.

- **Planet four-part cycle**, with three valid original phase thresholds: D1 at cycle start; R2 at first extra; R3 at second extra; D4 at third extra. Boundary equality selects the new phase. Do not confuse synodic phase R2/R3 with the `moto_pianeti` direct/retrograde state.
- **Moon cycle:** Moon synod anchor is the latest *recorded* new moon (as defined by the source data convention); phase between that anchor, first quarter, full and last quarter maps to CRESC1 / CRESC2 / CAL1 / CAL2. These are **stage labels**, not a continuous illuminated-fraction calculation. Mark the source convention unverified if required by the release manifest.
- **No sufficient phase thresholds:** return `phase.code = None` with a meaningful status; preserve SQL `NULL` separately from `[]`; body with no preceding anchor must return a predictable missing-data record, not a future or imaginary cycle. Avoid claiming a valid final D4/CAL2 period past an unreviewed gap if the next cycle is unavailable.
- **Time input:** accept an ISO-8601 instant with `Z` or numeric offset, or a timezone-aware Python `datetime`, preserve microseconds; reject naive date-times with a typed error. Exact equality with a new-moon/synod anchor counts that anchor as current.
- **Status:** the archive might contain recorded results but lack human-approved completeness windows; expose `coverage_certified=false` and warnings, do not present a historical phase as independently verified. The SHA-256 identifies file bytes, not scientific certainty.
- **Output:** normal Python `dict` / list / int / str / bool / `None` suitable for `json.dumps`, with `synods`, `new_moon`, `requested_at_utc`, and `dataset`. These are **proposed** names until the binding and regression suite exist.

Acceptance includes Python local `pip install .`, an installable wheel, actual `import virgostars_events` in a fresh virtual environment and tested parity against the backend's latest synod and phase logic. The separate dataset licensing decision gates public dataset redistribution, not a local install of the code for evaluation. **Do not report this API as available before it is implemented and tested.**

## Illustrative JS / TS API (not implemented)

~~~ts
const astro = await VirgoStarsEvents.open("./virgostars-events.vseph");
const synod = astro.synods.at("Mercury", "2030-05-15T12:00:00Z");
const next = synod?.next();
const phase = astro.synods.phaseAt("Mercury", "2030-05-15T12:00:00Z");
const moon = astro.synods.lunarEvents(2030, 5, "UTC");
const motions = astro.motions.onDate("2030-05-15", "Europe/Rome");
const direction = astro.motions.stateAt("Saturn", "2030-05-15T12:00:00Z");
const ingress = astro.ingresses.next("Venus", "2030-05-15T12:00:00Z");
const events = astro.events.between("2030-05-01T00:00:00Z", "2030-06-01T00:00:00Z");
~~~

## Illustrative Python API (not implemented)

~~~python
astro = VirgoStarsEvents.open("virgostars-events.vseph")
synod = astro.synods.at("Mercury", "2030-05-15T12:00:00Z")
next_synod = synod.next() if synod else None
motion = astro.motions.state_at("Saturn", "2030-05-15T12:00:00Z")
events = astro.events.between("2030-05-01T00:00:00Z", "2030-06-01T00:00:00Z")
~~~

Rust is authoritative; Python uses snake_case and JS uses camelCase. Async only for dataset loading where required; after open, indexed in-memory/file-backed reads are synchronous where supported. WASM packaging, streaming/file access and browser memory limits require dedicated tests.

## API stability

Freeze naming, inclusivity, errors, time representation, phase schemas and ordering **before v1.0**. Changes that affect these semantics require versioned contracts, fixtures, migration notes and explicit maintainer approval.
