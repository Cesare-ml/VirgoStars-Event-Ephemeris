# Synthetic fixtures v0

Seven **deliberately fabricated** examples, not an astronomy reference, not exported from PostgreSQL and not official VSEPH binary files. **Never interpret these dates as real astronomical events.**

## Files

| File | Purpose | Expected |
| --- | --- | --- |
| 01-lunar-cycles.json | Two lunar cycles, three phase dates per anchor, microseconds, month overlap | valid |
| 02-optional-and-historical.json | Pre-1970, null phases, variable two phases, planetary three phases | valid |
| 03-motion-stations.json | Direct and retrograde stations; same-time different bodies | valid |
| 04-retrograde-ingresses.json | Sign 0, retrograde 11, back to 0; near midnight | valid |
| 05-boundary-and-dst.json | UTC instants during Europe/Rome DST change; cross-family same instant | valid |
| 06-invalid-duplicate.json | Duplicate family/body/timestamp | invalid: DUPLICATE_EVENT |
| 07-invalid-sign.json | Sign outside 0..11 | invalid: SIGN_RANGE |

Each record has an explicit UTC ISO instant called at and a decimal-string i64-compatible atUs representing signed POSIX microseconds since 1970-01-01T00:00:00Z. Phases use the same timestamp pair. Null phases and empty phases are distinct. The synthetic flag is mandatory. The validator recognizes event ordering, duplicate family/body/timestamp and phase ordering; it does **not** perform astronomical calculations or prove dataset coverage.

## Reproduce (Node.js 22+, no dependencies)

~~~sh
node tools/generate-synthetic-fixtures.mjs
node --test test/synthetic-fixtures.test.mjs
~~~

The generator is deterministic. The test runner checks exact byte equivalence, valid and invalid samples, pre-1970 times, microsecond precision, repeated signs and two UTC instants crossing Europe's autumn DST transition. UTC reference instants do not imply that the validator implements IANA time zone transformations.

## Next milestones

Issues #2 and #3 cover the minimal Rust workspace and byte-level VSEPH specification. Use these JSON fixtures as the test-only conceptual contract, then introduce golden binary fixtures and Rust decoder round-trip tests. Do not convert these fabricated events into claimed astronomical source data.
