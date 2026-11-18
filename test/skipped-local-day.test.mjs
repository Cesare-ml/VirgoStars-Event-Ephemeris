/*
 * The contents of this file are subject to the Common Public Attribution License Version 1.0
 * (the "License"); you may not use this file except in compliance with the License.
 * See LICENSE and NOTICE in the repository root for CPAL-1.0 Exhibits A/B,
 * attribution and the original developer's rights.
 * Original Developer: Cesare-ml (VirgoStars project), copyright (c) 2026.
 * Powered by VirgoStars — https://www.virgostars.it/
 * Modification (2026-11-15): independent skipped-local-calendar-day contract fixture tests.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/skipped-local-day.json', import.meta.url), 'utf8'));
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;

function validDate(value) {
  if (typeof value !== 'string' || !DATE.test(value)) return false;
  const date = new Date(value + 'T00:00:00Z');
  return Number.isFinite(date.getTime()) && date.toISOString().startsWith(value + 'T');
}

function utcMillis(value) {
  if (typeof value !== 'string' || !UTC.test(value)) return NaN;
  const millis = Date.parse(value);
  if (!Number.isFinite(millis)) return NaN;
  return new Date(millis).toISOString() === value.replace(/Z$/, '.000Z') ? millis : NaN;
}

function validate(document) {
  assert.equal(document?.schema, 'virgostars-skipped-local-day/1');
  assert.equal(document?.expectedFailure?.code, 'NonexistentLocalDate');
  assert.ok(typeof document.provenance === 'string' && document.provenance.trim());
  assert.ok(Array.isArray(document.cases) && document.cases.length > 0);
  const seen = new Set();
  for (const row of document.cases) {
    assert.ok(typeof row.id === 'string' && row.id.trim(), 'missing fixture ID');
    assert.ok(validDate(row.localDate), 'invalid local date');
    assert.ok(typeof row.timeZone === 'string' && row.timeZone.trim(), 'missing IANA zone');
    assert.ok(typeof row.reference === 'string' && row.reference.trim(), 'missing reference');
    const key = row.timeZone + '|' + row.localDate;
    assert.ok(!seen.has(key), 'duplicate local date and zone');
    seen.add(key);
    if (row.outcome === 'error') {
      assert.equal(row.errorCode, 'NonexistentLocalDate', 'unknown skipped-day error');
      assert.equal(row.startUtc, null, 'skipped day must have no start instant');
      assert.equal(row.endUtc, null, 'skipped day must have no end instant');
      assert.equal(row.durationSeconds, null, 'skipped day must not be an empty interval');
    } else if (row.outcome === 'interval') {
      assert.equal(row.errorCode, undefined, 'ordinary day cannot report a skipped-day error');
      const start = utcMillis(row.startUtc);
      const end = utcMillis(row.endUtc);
      assert.ok(Number.isFinite(start) && Number.isFinite(end), 'invalid UTC bounds');
      assert.ok(start < end, 'non-increasing interval');
      assert.ok(Number.isSafeInteger(row.durationSeconds) && row.durationSeconds > 0, 'invalid interval duration');
      assert.equal(end - start, row.durationSeconds * 1000, 'interval duration mismatch');
    } else {
      assert.fail('unknown local-day outcome');
    }
  }
  return document.cases;
}

// Hand-reviewed reference UTC instants; not computed using a second timezone engine.
const goldens = Object.freeze({
  '2011-12-29': ['interval', '2011-12-29T10:00:00Z', '2011-12-30T10:00:00Z', 86400],
  '2011-12-30': ['error', null, null, null],
  '2011-12-31': ['interval', '2011-12-30T10:00:00Z', '2011-12-31T10:00:00Z', 86400],
});

test('three Apia date/zone fixture records are explicit, distinct and schema-valid', () => {
  assert.equal(validate(fixture).length, 3);
  assert.deepEqual(fixture.cases.map(row => row.localDate), Object.keys(goldens));
  assert.ok(fixture.cases.every(row => row.timeZone === 'Pacific/Apia'));
});

test('all Apia UTC bounds and the skipped-day outcome match independent hard-coded goldens', () => {
  for (const row of validate(fixture)) {
    assert.deepEqual([row.outcome, row.startUtc, row.endUtc, row.durationSeconds], goldens[row.localDate]);
  }
  assert.equal(fixture.cases[1].errorCode, 'NonexistentLocalDate');
});

test('neighboring valid dates are contiguous without assigning any UTC instant to skipped date', () => {
  const [before, skipped, after] = validate(fixture);
  assert.equal(before.endUtc, after.startUtc);
  assert.equal(skipped.outcome, 'error');
  assert.equal(skipped.startUtc, null);
  assert.equal(skipped.endUtc, null);
  assert.equal(Date.parse(after.endUtc) - Date.parse(before.startUtc), 2 * 86400 * 1000);
});

test('missing or renamed error is not silently converted into an empty day', () => {
  for (const replacement of [undefined, 'MissingCoverage', 'InvalidTimeZone']) {
    const altered = structuredClone(fixture);
    altered.cases[1].errorCode = replacement;
    assert.throws(() => validate(altered), /unknown skipped-day error/);
  }
});

test('fabricated zero-length or adjacent-day interval for skipped date fails closed', () => {
  for (const bad of [
    { outcome: 'interval', startUtc: fixture.cases[0].endUtc, endUtc: fixture.cases[2].startUtc, durationSeconds: 0 },
    { outcome: 'error', startUtc: fixture.cases[0].endUtc },
    { outcome: 'error', endUtc: fixture.cases[2].startUtc },
    { outcome: 'error', durationSeconds: 0 },
  ]) {
    const altered = structuredClone(fixture);
    Object.assign(altered.cases[1], bad);
    assert.throws(() => validate(altered));
  }
});

test('malformed or inconsistent ordinary-day bounds cannot masquerade as valid intervals', () => {
  for (const bad of [
    { endUtc: '2011-12-29T09:00:00Z' },
    { endUtc: '2011-12-30T10:00:00+00:00' },
    { endUtc: '2011-12-30T10:00:60Z' },
    { durationSeconds: 82800 },
  ]) {
    const altered = structuredClone(fixture);
    Object.assign(altered.cases[0], bad);
    assert.throws(() => validate(altered));
  }
});

test('invalid calendar dates and duplicate zone/date inputs fail closed', () => {
  const duplicate = structuredClone(fixture);
  duplicate.cases.push({ ...duplicate.cases[0], id: 'duplicated' });
  assert.throws(() => validate(duplicate), /duplicate local date and zone/);
  const malformed = structuredClone(fixture);
  malformed.cases[0].localDate = '2011-02-30';
  assert.throws(() => validate(malformed), /invalid local date/);
});

test('fixture provenance and CPAL attribution remain visible', () => {
  assert.equal(fixture.sourceLicense, 'CPAL-1.0');
  assert.match(fixture.attribution, /Powered by VirgoStars/);
  assert.match(fixture.provenance, /IANA tzdb 2025b/);
  assert.match(fixture.status, /contract/);
});
