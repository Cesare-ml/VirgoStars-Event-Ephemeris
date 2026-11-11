# Python-first MVP — latest synods, previous new moon and cycle phases

**Priority: FIRST USER-EXECUTABLE MILESTONE.** Rust synod stage + batch query and Python PyO3/maturin package **source now exist** on draft PR #5, but **the native wheel has not yet been built or installed**. This remains an implementation/acceptance contract, not a claim that the Python package is currently usable. Work on draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); data snapshot is separately staged on draft [PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). No changes to production/backend or historical records.

## User story

> Install the library easily from Python. Given an instant X, obtain an object containing the **latest synodic cycle anchor at or before X for each available celestial body**, the **latest recorded new moon at or before X**, and each cycle's **current phase as of X**, when the original phase thresholds permit a determination.

MVP is a **precomputed event reader**, not an astronomical calculator. It must work offline and must not require PostgreSQL, Swiss Ephemeris, VirgoStars API access, or a second independent scientific Python algorithm.

### Intended local installation — commands become valid AFTER implementation

Initial target: **Windows 11 64-bit, CPython 3.10–3.13**, then wheel/build verification on Linux/macOS. Prefer a PyO3 Rust extension + maturin mixed Python package. Add `pyproject.toml` and a dedicated binding crate/directory, without duplicating the P0 parser in Python. A local wheel and an editable install must both work:

```powershell
py -3 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -U pip
.\.venv\Scripts\python.exe -m pip install .
# developer-only alternative after the packaging exists:
.\.venv\Scripts\python.exe -m pip install -e .
```

The build MAY need Rust/Cargo on the developer computer; once a platform-specific wheel is built, installing **that wheel** must not require Rust. The first MVP requires the caller to supply a local **separately authorized** VSEPH-P0 archive path. Never quietly download, bundle or ship the owner dataset in a package without the separate dataset-license/release decision ([issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9)). Only **after** approval and public PyPI publication may docs promise `pip install virgostars-event-ephemeris`; check availability/name ownership first. Wheel package name and Python import name must be kept distinct and documented.

References: [maturin local development](https://www.maturin.rs/local_development.html), [maturin PyO3 bindings](https://www.maturin.rs/bindings), [PyO3 project](https://pyo3.rs/).

### Target Python interface (DESIGN ONLY, not yet implemented)

```python
from virgostars_events import VirgoStarsEvents

events = VirgoStarsEvents.open(
    "datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0",
    expected_sha256="c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e",
)
result = events.get_synods_at("2026-11-11T12:00:00Z")
# Also accept timezone-aware datetime.datetime; reject naive datetimes.
print(result["synods"]["Mercury"]["cycle_start_utc"])
print(result["synods"]["Mercury"]["phase"]["code"])
print(result["new_moon"]["cycle_start_utc"])
print(result["new_moon"]["phase"]["code"])
```

All dictionaries, timestamps and primitive fields must be JSON serializable with standard `json.dumps`. `result["new_moon"]` is the **same Moon cycle data** as `result["synods"]["Moon"]`, exposed as a convenience alias without a separate inconsistent query; it is `None` when the snapshot has no preceding Moon new-moon anchor. Do not invent a Moon phase for a data gap.

#### Proposed response shape (schema illustration, values NOT real astronomical output)

```json
{
  "requested_at_utc": "<normalized offset-aware ISO-8601 instant>",
  "dataset": {
    "id": "<manifest dataset ID>",
    "sha256": "<verified whole-file SHA-256>",
    "format": "VSEPH-P0",
    "coverage_certified": false
  },
  "synods": {
    "Mercury": {
      "body": "Mercury",
      "source_id": 123,
      "cycle_start_utc": "<original event timestamp>",
      "next_cycle_start_utc": "<next recorded cycle timestamp or null>",
      "phase_thresholds_utc": [
        "<first extra timestamp>",
        "<second extra timestamp>",
        "<third extra timestamp>"
      ],
      "phase": {
        "code": "R2",
        "started_at_utc": "<first extra timestamp>",
        "status": "derived_from_recorded_thresholds"
      },
      "coverage": {
        "status": "unreviewed",
        "warnings": []
      }
    },
    "Moon": {
      "body": "Moon",
      "source_id": 456,
      "cycle_start_utc": "<recorded last new moon at or before request>",
      "next_cycle_start_utc": null,
      "phase_thresholds_utc": [],
      "phase": {
        "code": null,
        "started_at_utc": null,
        "status": "unknown_missing_phase_thresholds"
      },
      "coverage": {
        "status": "unreviewed",
        "warnings": ["example_warning_if_manifest_indicates_a_gap"]
      }
    }
  },
  "new_moon": {
    "body": "Moon",
    "source_id": 456,
    "cycle_start_utc": "<recorded last new moon at or before request>",
    "next_cycle_start_utc": null,
    "phase_thresholds_utc": [],
    "phase": {
      "code": null,
      "started_at_utc": null,
      "status": "unknown_missing_phase_thresholds"
    },
    "coverage": {
      "status": "unreviewed",
      "warnings": ["example_warning_if_manifest_indicates_a_gap"]
    }
  }
}
```

The sample uses placeholders and fabricated IDs; **it is not a real ephemeris result**. The implementation should return the complete body map found in the dataset (not silently hardcode a smaller planet list), with a deterministic key order. For a body indexed in the archive but without any preceding anchor at X, keep that body in `synods` with `status="no_preceding_recorded_cycle"`, `source_id=None`, `cycle_start_utc=None` and `phase.code=None`; `new_moon=None` if Moon has no predecessor. No fictional anchor or fallback to the next event. The `new_moon` object must compare equal in value to `synods["Moon"]` whenever a Moon anchor exists.

## Required query and phase rules

1. **Inclusive anchor:** for every body, choose the greatest **recorded** `synod.start <= request instant`, preserving exact signed i64 UTC microseconds and original PostgreSQL source IDs. Never select a future anchor. At exact anchor equality, the new cycle begins.
2. **Moon new moon:** the latest *recorded* `synod/Moon` anchor at or before X is the previous/new-current Moon new moon **by the existing data model convention**; separately review that convention for each released dataset. At exact timestamp, the anchor at X is included. The next recorded Moon cycle may optionally close the interval, but its absence must not be fabricated.
3. **Four-threshold phase classification:** for a body using the current VirgoStars D1/R2/R3/D4 convention with exactly three **present, ordered and valid** phase extras `p1<p2<p3`, classify `[start,p1)=D1`, `[p1,p2)=R2`, `[p2,p3)=R3`, `[p3,next_start)=D4` when the queried instant is inside a known recorded cycle. Threshold equality selects the phase **beginning** at that threshold. This is **synodic cycle phase**, not the direction from `moto_pianeti`. Existing legacy lowercase aliases belong in a later adapter, not the canonical result.
4. **Moon phases:** with three valid extras interpret the convention `[new,first_quarter)=CRESC1`, `[first_quarter,full)=CRESC2`, `[full,last_quarter)=CAL1`, `[last_quarter,next_new)=CAL2`. Include descriptive labels (new-to-first-quarter, first-quarter-to-full, full-to-last-quarter, last-quarter-to-next-new). The new-moon instant is the cycle start (not a fifth interpolated event).
5. **Null, empty, or partial phases:** preserve original `NULL` vs `[]` vs present N extras. If there are insufficient or invalid phase thresholds, `phase.code=None` with an explicit reason; **never infer D1/CRESC1**. For bodies with different legitimate phase cardinalities, return stored anchors and extras with `phase.code=None` unless their own verified rule has been defined.
6. **Cycle boundaries and gaps:** if the next recorded anchor exists, treat it as the end-exclusive boundary. If it does not, do not assert that a phase extends indefinitely into the future. A very old latest anchor may be returned as *recorded evidence*, with gap/coverage warnings and an unverified phase if continuity is not independently established. Known Moon candidate gaps around 1919 and 2018 must be surfaced from the versioned data-quality/coverage metadata; **no guessed dates or synthetic completeness labels**.
7. **Time safety:** require ISO-8601 with explicit `Z`/offset, or a timezone-aware Python `datetime`; normalize to UTC with no floating-point timestamp conversion or loss of microseconds. Reject naive datetimes, malformed dates, out-of-range instants and unsupported datasets with typed errors. Future civil-day `Europe/Rome` support is NOT required for first instant-based MVP.
8. **Integrity/attribution:** require an explicit local file and independently supplied expected SHA-256/manifest to establish archive identity; reject invalid/mismatched archives. The code must retain CPAL-1.0 notices and the visible credit **Powered by VirgoStars — https://www.virgostars.it/**. File hashes do not certify astronomy.

## Work breakdown — smallest implementable vertical slices

- [ ] **PY-MVP.0: validate existing Rust and P0 on Windows**, with `cargo fmt`, Clippy `-D warnings`, tests, release build and exact Python/Rust archive SHA/count parity on a read-only assembled copy of the approved 2026-11-10 snapshot. Log full output. No production or dataset changes.
- [ ] **PY-MVP.1: Rust synod aggregation and pure phase engine**, reusing `P0Archive::at` / `next` and the existing `Event::Synod` record. Add a bounded `at_all_synods(t)`-style operation based on the archive's *actual* synod body index; retain source IDs, thresholds and per-body lack-of-coverage diagnostics. Implement deterministic phase codes as a separate, tested Rust routine. Avoid full-archive scan on each call.
- [ ] **PY-MVP.2: Python binding crate**, using PyO3/maturin to invoke the Rust reader and transform its result into a typed, JSON-serializable dictionary. Implement `VirgoStarsEvents.open(path, expected_sha256=...)` and `get_synods_at(t)`; reject invalid/naive timestamps. No independent Python decoder.
- [ ] **PY-MVP.3: local pip installation and offline example**, `pyproject.toml` build system, importable `virgostars_events` module, `pip install .` and `pip install -e .`, reproducible wheel build and installation into a fresh virtual environment. Explicit file path; no automatic network or dataset download.
- [ ] **PY-MVP.4: test and accept the user story**, require `python tools/validate_python_mvp.py` to run 5/5 **non-skipped** synthetic native P0 tests plus 8/8 pure-Python tests after wheel installation; then compare same instant/body/phase against PostgreSQL's `getSinodiPerDataoraUtc` and `getMotiSinodiciPianetiPerDataoraUtc` on agreed reviewed samples. Run edge cases: X at/before/after anchor and each of three phase boundaries; Moon new moon; body with NULL vs []; no preceding/new cycle; gaps 1919/2018; pre-1970; offset-aware datetime; invalid dates; whole-file digest mismatch; JSON serializability. Maintain deliberate improvements separately from legacy bugs.
- [ ] **PY-MVP.5: deliver a reproducible user-ready artifact**, wheel for target CPython/Windows, a 10-line script example, README quick start, example expected fixture output (synthetic clearly labeled), version and visible credits, validated install and smoke test by maintainer. **This acceptance milestone is the first usable Python MVP**. Keep PR draft until review and approval; public PyPI release is a separate decision.

## Implementation checkpoint — 2026-11-11

- **Written:** `crates/events-core/src/synod_stage.rs` (`assess_synod_stage` and 5 Rust unit tests); `P0Archive::synods_at(X)` returning a record and phase assessment per synod-indexed body (4 Rust integration tests); `crates/python-bindings` PyO3 native extension; root `pyproject.toml`; `python/virgostars_events` Python API with microsecond-precise aware date-time conversion and JSON-ready projection; `examples/python_synods.py`; native smoke suite `python/tests/test_native_smoke.py`.
- **Actually run:** 8/8 pure-Python tests PASS using the exact GitHub-committed files (verified `git hash-object`). They test the conversion and wrapper against an explicitly mocked native reader; they do **not** validate the Rust extension.
- **Not yet run:** compilation of the new Rust crate, rustfmt/Clippy/test/build of the current branch, maturin/pip install or wheel, real snapshot smoke test or backend SQL parity. **All M0 boxes remain incomplete until their actual acceptance criteria pass.**
- **Build/test instructions:** [PYTHON_MVP_BUILD.md](PYTHON_MVP_BUILD.md).
- No source scientific events modified, no PyPI release, no merge or GitHub Actions.

## Definition of first MVP DONE

On an ordinary Windows 11 PC with a fresh Python environment, the maintainer can install a **built wheel with pip**, pass an authorized local P0 file and trusted SHA-256, call `get_synods_at(X)`, and obtain the correct **latest recorded** synod for every available body, including **last recorded new moon**, with exact threshold-derived phase labels or explicit unknowns. Output passes regression tests, agrees with the original data for tested instants, uses Rust as sole decoder, and never equates records with certified historical coverage.

A simple `pip install .` from the repository must also work with Rust toolchain present; public `pip install virgostars-event-ephemeris` needs an **authorized PyPI release later**, not assumed as completed.

**Out of scope for the first MVP:** frozen VSEPH v1 byte format, mass astronomical backfill, future-event prediction, automatic dataset download, REST/API servers, browser WASM, Node.js production migration, full civil-date/DST support, PyPI upload and official stable v1 release.
