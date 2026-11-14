# Moon synods: recorded source discontinuities and M0 guard

**Status 2026-11-11:** this document records **source-data anomalies**, not astronomical calculations, certified coverage, or a repair to the historical dataset. No rows are inserted or changed. The owner-authorized 2026-11-10 CSV/P0 snapshot remains separate on draft [PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). The code change is in draft [PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5).

## Evidence from original normalized `sinodi.csv`

A read-only inspection of the previously supplied authorized normalized CSV found **37,054 recorded Moon synod anchors**. Among **37,053 adjacent Moon-anchor pairs**, all but two intervals are **less than 30 days**; the maximum of these non-anomalous source intervals is approximately **29.8352 days**. Two adjacent records are separated by more than a year:

| Last recorded Moon new moon before gap (UTC) | Original ID | Next recorded Moon new moon (UTC) | Original ID | Interval |
| --- | ---: | --- | ---: | ---: |
| 1918-12-03 15:18:52 | 44934 | 1920-01-21 05:26:29 | 44935 | ~413.5886 days |
| 2017-12-18 06:30:25 | 46146 | 2019-01-06 01:28:11 | 46147 | ~383.7901 days |

The original source also contains **three stored phase timestamps** after each preceding Moon anchor:

- ID 44934: 1918-12-11 02:30:59; 1918-12-17 19:17:06; 1918-12-25 06:30:10 (UTC)
- ID 46146: 2017-12-26 09:20:06; 2018-01-02 02:24:05; 2018-01-08 22:25:13 (UTC)

Without an additional continuity rule, a naïve last-recorded-anchor/next-recorded-anchor query assigns the fourth **CAL2** stage after the final stored phase threshold until the *next* recorded new moon, i.e. nearly all of 1919 or 2018. That is an invalid inference from missing records. **This evidence does not establish the missing new-moon timestamps.** It only shows missing or incomplete source coverage. Scientific recalculation and independent review remain in [issue #7](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/7).

## Interim, deliberately conservative M0 behavior

- Retain **the actual latest recorded Moon anchor, source ID and stored extra phase timestamps**; never silently substitute a guessed new moon.
- In `virgostars-events-core::assess_synod_stage`, if the next recorded Moon start is **more than 45 days** after the preceding start, the fourth stage *after the third stored source threshold* is reported as `phase.code=None`, `phase.started_at_utc=None`, and `phase.status="unknown_discontinuous_moon_anchors"`. Earlier stored phases remain readable as **recorded**, unreviewed evidence.
- The `virgostars_events` Rust/PyO3 result reports `coverage.status="unreviewed"`, `coverage_certified=false` and warnings including `no_reviewed_coverage_policy` and `moon_recorded_cycle_gap_over_45_days`. The gap warning applies for the anomalous pair even at an early recorded stage.
- The **45-day bound is an anomaly-detection threshold**, not a Moon-period prediction. It is deliberately above every otherwise observed interval in this particular snapshot, and is **not a substitute for a reviewed, SHA-bound scientific coverage policy**. Other family/body cycles use their own semantics; the bound **must never be applied to planets**.
- If the next recorded Moon anchor does not exist, the final stored `CAL2` stage remains `unknown_open_ended_cycle` as before. Before the first anchor the answer remains `no_preceding_recorded_cycle`.

This is a **fail-closed safety improvement, not completion of issue #7**. Potentially incomplete recorded spans shorter than 45 days are *not detected* by the interim guard. For a scientifically verified result, an independently reviewed and versioned coverage manifest is still necessary.

## Regressions and required execution

- Rust core `long_moon_record_gap_is_not_reported_as_a_year_long_cal2_phase` checks that the final Moon stage becomes unknown for a 413-day source gap, earlier recorded stages remain available, a 30-day Moon interval retains CAL2, and long **Mars** cycles are unaffected.
- Independent source CSV parity oracle `tools/verify_python_synods.py` compares the native output against source rows and explicitly checks the new warning. Probe coverage includes **1919-06-01**, **2018-06-01**, mid-gap, and moments just before the next recorded Moon anchor. `tools/test_verify_python_synods.py` has a synthetic discontinuity test that requires unknown stage and gap warning.
- With the original owner archive supplied explicitly, `python/tests/test_native_smoke.py::test_known_source_moon_gaps_do_not_fabricate_a_year_long_phase` checks June 1919 (source ID 44934) and June 2018 (source ID 46146) through the compiled Rust/PyO3 package.

These new tests are **written but not yet run on the Rust/PyO3 source**. The actual original CSV gap dates, record IDs, gap durations and extra timestamp ordering were independently checked in a read-only local data inspection. That is **source-data verification**, not a successful native test or a scientific coverage certification.

Recommended Windows acceptance commands after a safe checkout and local wheel build:

```powershell
py -3 tools\validate_rust.py
.\.venv\Scripts\python.exe tools\validate_python_mvp.py --archive $env:VSEPH_P0_PATH --sha256 $env:VSEPH_P0_SHA256 --synods-csv "D:\path\to\authorized\sinodi.csv"
```

For remaining coverage issues, also see [#8](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/8) (ingress gaps), [#9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9) (separate dataset reuse license), and [Python-first MVP](PYTHON_FIRST_MVP.md).

**Powered by VirgoStars — https://www.virgostars.it/**
