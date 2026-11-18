# Independent ephemeris-engine validation: proposed protocol

**Status: preparation only, 2026-11-18. No real-engine execution, scientific
acceptance, event import or coverage certification is performed by this plan.**
The owner directs this repository not to use VirgoStars-Ephemeris as a
calculation source until the engine has been independently validated. The
independent assessment must therefore be completed separately, for a specific
engine build and explicitly bounded use, before the authorized real
smoke/binding suite is run here.

This is a reviewable proposal, not evidence of a passing gate. All thresholds,
case matrices, reference choices and the decision authority below must be
agreed and recorded before seeing the new comparison results. A failed or
missing item remains open; do not silently widen tolerances or relabel a
transport smoke as an accuracy test.

## Offline structural preflight

`tools/engine_validation_preflight.py` checks the shape of a single UTF-8
JSON plan without invoking the engine or reference, accessing any artifact
named inside it, or contacting the network. An explicitly **invented**
example is available at
[`tools/fixtures/engine_validation_plan.synthetic.json`](../tools/fixtures/engine_validation_plan.synthetic.json).
Its names, digests, values and review statements are fabricated for tests and
must never be treated as actual engine, kernel or reviewer evidence.

```sh
python tools/engine_validation_preflight.py tools/fixtures/engine_validation_plan.synthetic.json
python -m unittest discover -s tools -p test_engine_validation_preflight.py -v
```

Copy the example to a separately managed plan and replace its declarations
when preparing a real independent assessment. Version 1 requires an explicit
nonempty body/quantity scope and signed microsecond interval; source, binary,
binding and kernel digest **syntax** plus declared build/time/kernel
provenance; reference and shared-dependency descriptions; case selection,
holdout and expected count; one finite nonnegative maximum-absolute threshold
with matching units for every body/quantity pair; and reviewer/preregistration
declarations. Supported quantities and exact units are in the synthetic
example and the tool's `UNITS` mapping. It rejects duplicate/unknown keys,
duplicate matrix entries, nonfinite or nonstandard numbers, incomplete
matrices, invalid UTF-8 and oversized inputs. Invalid input produces a
nonzero exit and no JSON report.

On a syntactically valid plan, the output includes the plan's local SHA-256
and `declarations_well_formed: true`; all independent-validation,
astronomical-accuracy, coverage and import-safety flags remain **false**. The
tool does not authenticate hashes, open declared files, establish reviewer
identity or approval timing, prove independence, execute cases, compare
results, or assess physical correctness. The digest identifies only the bytes
read, not a trusted or immutable preregistration. Version 1 also does not
encode the complete vector, percentile, uncertainty or event-time criteria
below; those remain mandatory human-reviewed evidence.

## Scope of the eventual decision

The proposed initial capability is **geocentric apparent longitude and
latitude and their signed time derivatives**, for the specific bodies,
epochs, time inputs and frames needed by the event-generator drafts. Record
exactly which planet centers or system barycenters are meant. Do not extend
the decision to stars, topocentric output, other epochs, every event family,
the source archive, or production activation by implication.

Record the reviewed engine source commit, clean or dirty build tree, compiler,
flags, target platform, dependency lockfiles, binary and binding digests, and
an auditable link between that build and the source. Record each kernel and
time-data file's source, version, segment identifiers/ranges, size and SHA-256;
the expected digests must come from an independently reviewed trusted source.
A locally calculated digest alone identifies bytes, not their authenticity.

## Reference and independence

1. An independent reviewer selects and documents a reference implementation,
   version, acquisition route, command or complete query, response metadata
   and retained raw results. Record reviewer identity, review date and their
   relationship to the engine implementation. The reference must not call
   the engine under test, its binding or its event solver.
2. Separate **implementation independence** from **orbital-model/data
   independence**. A Horizons/NAIF comparison against the same DE solution can
   independently check evaluation and coordinate handling but is not an
   independent orbital solution or a measurement of physical truth. Report
   shared kernels, models, algorithms, libraries and reference inputs.
3. Use a separately documented cross-implementation apparent-coordinate and
   rate reference with matched settings. Any shared model or correction must be
   disclosed. A position-derived numerical speed is a different observable
   from a reference's directly reported speed; do not silently substitute one
   for the other. If finite differences are used, predeclare the stencil,
   wrapped-angle arithmetic, step sizes and convergence/sensitivity checks.
4. Preserve input and reference provenance and applicable rights. A checksum
   of a self-generated fixture, or two implementations that merely agree, is
   not proof of independence or physical accuracy.

The engine repository's
[validation protocol at the inspected commit](https://github.com/Cesare-ml/VirgoStars-Ephemeris/blob/ab23b998e34b0ec41e4b0e08df4b73f34d731a5a/docs/VALIDATION.md)
describes a layered matrix. Its
[seasonal speed investigation](https://github.com/Cesare-ml/VirgoStars-Ephemeris/blob/ab23b998e34b0ec41e4b0e08df4b73f34d731a5a/docs/PLANETARY_SPEED_VALIDATION_SEASONAL_2026-11-17.md)
and
[apparent-rate diagnostics](https://github.com/Cesare-ml/VirgoStars-Ephemeris/blob/ab23b998e34b0ec41e4b0e08df4b73f34d731a5a/docs/SWISS_APPARENT_RATE_DISCREPANCIES_2026-11-17.md)
are prior sampled investigations with stated limitations, not acceptance of
the event-generator use case. Their reported effects motivate, but do not
replace, the predeclared independent comparison.

## Contract and test matrix to approve in advance

For every case, record input and output units and the exact frame, equinox,
center, target, correction mode and time scale. Specify UTC/POSIX microsecond
conversion to TT/TDB, leap-second and Delta-T/UT1 data and version, historical
calendar and astronomical year-zero policy. Explicitly mark time ranges or
conventions the chosen build rejects. Compare apparent with apparent, and
geometric with geometric; record light time, aberration, deflection, nutation
and precession conventions. Never infer latitude or its speed from longitude.

Predeclare at least:

| Layer | Required evidence |
| --- | --- |
| Kernel and raw states | Segment boundaries and representative interior epochs; target/center chains; geometric Cartesian position and velocity in matched frame and units. |
| Apparent coordinates | Same-instant longitude and latitude for each in-scope body and epoch, including wraparound, high latitude where relevant, and known difficult configurations. |
| Signed speeds | Longitude and latitude rates with matched per-day time basis, direct/retrograde and northward/southward signs, near-zero values and stations. Keep direct reported rates and position-derived estimates separate. |
| Time and boundaries | Supported interval edges, negative/positive epochs, leap handling and historical conversion policy as applicable; unsupported or extrapolated cases must fail or be explicitly out of scope. |
| Robustness | Invalid inputs, missing or wrong kernel, wrong digest, incomplete reference response, non-finite values and provider failure; require explicit errors rather than partial parity reports. |

Select deterministic representative and edge cases per body and capability,
including individual known difficult cases and an independently chosen
holdout set. Record case counts, selection method and failures, not just
successful examples. A finite sample is not a continuous-error bound.

Before execution, the reviewer must approve a per-body/per-output tolerance
table and uncertainty budget: position/vector units, longitude/latitude
angular units and wrapping, signed speed units and event-time units if event
references are later reviewed. State absolute/relative metrics, maxima,
percentiles, treatment of near-zero rates, outliers, numerical rounding,
reference precision and model differences. Leave unspecified values
**UNSET**, never invent a tolerance or change it retroactively to pass.

## Evidence and decision record

For each run retain, in a stable reviewed location, the exact invocation,
environment and timestamp; source/binary/input hashes and provenance; complete
reference query and raw response; expected versus actual values and signed
residuals; per-body worst cases and distribution; all failures, skips and
unsupported cases. Record operator and independent reviewer, decisions and
links to retained artifacts. Avoid putting proprietary kernel bytes or
unreviewed third-party data in this repository.

The reviewer may accept only the named build, named capability and explicitly
bounded configuration when all approved cases and predeclared tolerances pass,
provenance and independence are substantiated, discrepancies are resolved,
and the evidence can be reproduced. Missing trusted identity, ambiguous
frames/time scales, reference mismatch, unexplained outliers, missing expected
cases, or a changed source/binary/kernel require a new review or leave the
decision **NOT VALIDATED**. This decision does not certify continuous
astronomical event coverage.

Only after that separate decision, the already authorized real
smoke/binding suite in this repository may be run against the exact reviewed
artifacts. Its transport and binding results remain separate from scientific
accuracy. Independent event-level reference timestamps, detection of missed
crossings/tangencies and separate data review are still needed before any
event acceptance, import or coverage claim.
