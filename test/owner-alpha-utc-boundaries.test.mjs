/*
 * VirgoStars Event Ephemeris — synthetic UTC microsecond boundary tests.
 * CPAL-1.0 Exhibit A: Copyright (c) 2026 Cesare-ml (VirgoStars project).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Distributed under CPAL-1.0 WITHOUT WARRANTY; see LICENSE and NOTICE.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { makeFixtures, toMicros, validateFixture } from '../tools/generate-synthetic-fixtures.mjs';

// Expected values are computed by hand from POSIX seconds (no leap seconds), not
// from toMicros itself: seconds * 1_000_000 + fractional microseconds.
test('UTC epoch zero and the microseconds either side of it', () => {
  assert.equal(toMicros('1970-01-01T00:00:00Z'), '0');
  assert.equal(toMicros('1970-01-01T00:00:00.000000Z'), '0');
  assert.equal(toMicros('1969-12-31T23:59:59.999999Z'), '-1');
  assert.equal(toMicros('1970-01-01T00:00:00.000001Z'), '1');
});

test('fractional seconds keep all six digits, with and without padding', () => {
  // 1 s + 0.5 s; a single fractional digit is right-padded, never rounded.
  assert.equal(toMicros('1970-01-01T00:00:01.5Z'), '1500000');
  // -2 s + 1 µs.
  assert.equal(toMicros('1969-12-31T23:59:58.000001Z'), '-1999999');
  // 2000-01-01 = 946684800 s; sub-millisecond digits must survive.
  assert.equal(toMicros('2000-01-01T00:00:00.000123Z'), '946684800000123');
  // 2^31 - 1 seconds (the 32-bit time_t limit) plus 999999 µs.
  assert.equal(toMicros('2038-01-19T03:14:07.999999Z'), '2147483647999999');
  // Leap day in a leap year: 2024-02-29 = 1709164800 s.
  assert.equal(toMicros('2024-02-29T00:00:00Z'), '1709164800000000');
});

test('adjacent microseconds stay strictly ordered across second and day boundaries', () => {
  const pairs = [
    ['2030-12-31T23:59:59.999999Z', '2031-01-01T00:00:00Z'],
    ['1969-12-31T23:59:59.999998Z', '1969-12-31T23:59:59.999999Z'],
    ['2026-10-25T00:59:59.999999Z', '2026-10-25T01:00:00Z']
  ];
  for (const [before, after] of pairs) {
    assert.equal(BigInt(toMicros(after)) - BigInt(toMicros(before)), 1n, before + ' -> ' + after);
  }
});

test('malformed or ambiguous timestamps are rejected', () => {
  const invalid = [
    '2030-01-01T00:00:00+01:00',      // explicit offset instead of Z
    '2030-01-01T00:00:00+00:00',      // UTC offset spelled without Z
    '2030-01-01T00:00:00',            // no zone at all (host-local ambiguity)
    '2030-01-01T00:00:00z',           // lower-case designator
    '2030-01-01T00:00:00.1234567Z',   // more than six fractional digits
    '2030-01-01T00:00:00.Z',          // empty fraction
    '2030-01-01 00:00:00Z',           // space separator
    '2030-01-01T00:00Z',              // missing seconds
    '2030-02-30T00:00:00Z',           // invalid civil day
    '2030-04-31T00:00:00Z',           // 30-day month
    '2100-02-29T00:00:00Z',           // 2100 is not a leap year
    '2030-13-01T00:00:00Z',           // invalid month
    '2030-01-01T24:00:00Z',           // hour 24 is not normalised
    '2030-06-30T23:59:60Z',           // leap-second label is not representable
    ''
  ];
  for (const value of invalid) {
    assert.throws(() => toMicros(value), /INVALID_UTC_INSTANT/, JSON.stringify(value));
  }
});

test('validateFixture detects a single-microsecond mismatch in a valid synthetic event', () => {
  const source = makeFixtures()['04-retrograde-ingresses.json'];
  assert.deepEqual(validateFixture(source), []);

  // Stored microseconds one too high, ISO instant unchanged.
  const shiftedMicros = structuredClone(source);
  shiftedMicros.records[0].atUs = (BigInt(shiftedMicros.records[0].atUs) + 1n).toString();
  assert.ok(validateFixture(shiftedMicros).includes('TIME_MISMATCH'));

  // ISO instant one microsecond later, stored microseconds unchanged.
  const shiftedIso = structuredClone(source);
  shiftedIso.records[0].at = '2030-01-01T00:00:00.000001Z';
  assert.ok(validateFixture(shiftedIso).includes('TIME_MISMATCH'));

  // The last record already sits at .999999; one microsecond less must also be caught.
  const moon = structuredClone(source);
  const last = moon.records.at(-1);
  assert.equal(last.at, '2030-01-10T23:59:59.999999Z');
  last.atUs = (BigInt(last.atUs) - 1n).toString();
  assert.ok(validateFixture(moon).includes('TIME_MISMATCH'));

  // Mutations work on clones: the source fixture remains valid.
  assert.deepEqual(validateFixture(source), []);
});

test('validateFixture detects a single-microsecond mismatch in a synod phase', () => {
  const source = makeFixtures()['01-lunar-cycles.json'];
  const mutated = structuredClone(source);
  const phase = mutated.records[0].phases[0];
  phase.atUs = (BigInt(phase.atUs) + 1n).toString();
  assert.ok(validateFixture(mutated).includes('INVALID_PHASE'));
  assert.deepEqual(validateFixture(source), []);
});

test('validateFixture reports a malformed instant instead of accepting it', () => {
  const mutated = structuredClone(makeFixtures()['03-motion-stations.json']);
  mutated.records[0].at = '2026-10-24T23:30:00+00:00';
  assert.ok(validateFixture(mutated).includes('INVALID_UTC_INSTANT'));
});
