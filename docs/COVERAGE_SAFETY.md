# Coverage safety: recorded events are not certified states

**Experimental VSEPH-P0 adapter, 2026-11-10 snapshot.** Code and data are on [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), not on `main`. This document does not claim the library can recalculate missing astronomy.

## Why this matters

An indexed file can tell us the last **recorded** ingress or station, but cannot prove that no intermediate events were **omitted from the source database**. For example, the Moon has ingress data through 2099 and a disconnected Aries-only segment starting in 4836. Returning its most recent 2099 sign for the year 3000 would be scientifically unjustified.

The existing experimental `VSEPHReader.sign_state` and `motion_state` return only the last recorded row and explicitly warn about unknown coverage. The new `CoverageAwareReader` is an **opt-in fail-closed API** that will return an actual state **only inside a documented human-reviewed interval**, and only after encountering a source event within that interval.

## Current published policy

[prototype/coverage-policy-20261110.json](../prototype/coverage-policy-20261110.json) binds the policy to the exact P0 binary SHA-256:

```text
c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

The policy currently contains **zero reviewed windows**. This is intentional. Every current lookup returns `unknown_coverage` instead of a certified sign/motion state. Raw `at/next/previous/between` queries remain available for inspection and gap research.

## Python example (after assembling the dataset)

```python
import sys
sys.path.insert(0, "prototype")
from virgostars_vseph_reader import VSEPHReader
from coverage_policy import CoverageAwareReader, CoveragePolicy

binary = "datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0"
policy = CoveragePolicy.load("prototype/coverage-policy-20261110.json", binary)
safe = CoverageAwareReader(VSEPHReader(binary), policy)

result = safe.sign_at("Moon", "3000-01-01T00:00:00Z")
assert result["status"] == "unknown_coverage"
assert result["state"] is None
```

The snippet runs from the repository root. The current Python P0 prototype uses top-level imports; adding `prototype` to `sys.path` preserves that behavior until a package is defined. No database or network is used.

For individual observed events:

```python
reader = VSEPHReader(binary)
last_recorded = reader.at("ingress", "Moon", "3000-01-01T00:00:00Z")
# Raw evidence only; do not interpret as a scientifically valid sign.
```

## Contract

`CoverageAwareReader.sign_at(body, t)` and `direction_at(body, t)` return a dictionary:

- `status="unknown_coverage"`: no human-reviewed interval contains `t`; `state=None`.
- `status="unknown_initial_state"`: `t` lies in a reviewed interval, but no event within that interval has yet established an initial state; `state=None`.
- `status="verified"`: the timestamp is in a reviewed interval, with an event recorded at/after its start and at/before `t`; `state` includes source ID, event microseconds and sign, with direction for motion, and reviewer/evidence fields.

Time values are signed `i64` POSIX microsecond strings in policy JSON. Windows are **[start_us, end_us_exclusive)**. Disjoint reviewed windows are allowed; overlapping ones for one family/body are rejected. A policy must bind to the actual archive SHA-256; mismatched versions fail before querying. A review record requires `family`, `body`, `start_us`, `end_us_exclusive`, `reviewer` and `evidence`.

## How to review a missing interval

1. Inspect `yearly_counts.csv` and `candidate_large_intervals.csv`, which are **heuristic indicators**, not proof of missing physical events.
2. Choose a family/body/time interval and independently recompute expected events using a documented method and reference version. Cross-check all events and boundaries, including phase extras and retrograde re-entries.
3. Open a **separate review PR** with reproducible calculations/tests, methodology and provenance. Do not modify the immutable 2026-11-10 snapshot or fill gaps by interpolation.
4. Only after independent review and maintainer approval, declare the verified [start,end) interval in a **new versioned coverage policy** bound to its specific dataset digest. The historical policy stays unchanged.
5. Later the authoritative Rust implementation must implement identical unknown/verified semantics and the bindings must expose them.

The policy does not certify astrological interpretation or precision in arcseconds; it only records independently reviewed **event completeness** for a named family, body and interval.

## Verification

```bash
python tools/assemble_dataset.py
python prototype/test_coverage_policy.py
python prototype/test_vseph_prototype.py
```

The new coverage tests use intentionally fabricated data; they check zero-window fail-closed results, exact inclusive/exclusive boundaries, initial-state prerequisites, direction-vs-ingress separation, invalid intervals, missing evidence and an incorrect archive hash. They do not claim coverage for the real 380,844 events.

See also [DATASET_UPLOAD](DATASET_UPLOAD.md), [API_CONTRACTS](API_CONTRACTS.md), and issues [#7](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/7) and [#8](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/8).
