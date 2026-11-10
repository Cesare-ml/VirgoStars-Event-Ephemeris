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
