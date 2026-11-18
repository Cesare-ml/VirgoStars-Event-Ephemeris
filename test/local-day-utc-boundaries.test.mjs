/*
 * The contents of this file are subject to the Common Public Attribution License Version 1.0
 * (the "License"); you may not use this file except in compliance with the License.
 * See LICENSE and NOTICE in the repository root for CPAL-1.0 Exhibits A/B,
 * attribution and the original developer's rights.
 * Original Developer: Cesare-ml (VirgoStars project), copyright (c) 2026.
 * Powered by VirgoStars — https://www.virgostars.it/
 * Modification (2026-11-15): independent local-day UTC contract fixture tests.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/local-day-utc-boundaries.json', import.meta.url), 'utf8'));
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;

function canonicalUtc(instant) {
  if (typeof instant !== 'string' || !INSTANT.test(instant)) return NaN;
  const millis = Date.parse(instant);
  if (!Number.isFinite(millis) || new Date(millis).toISOString() !== instant.replace(/Z$/, '.000Z')) {
    return NaN;
  }
  return millis;
}
function validLocalDate(localDate) {
  return typeof localDate === 'string' && DATE.test(localDate)
    && Number.isFinite(Date.parse(`${localDate}T00:00:00Z`))
    && new Date(`${localDate}T00:00:00Z`).toISOString().startsWith(`${localDate}T`);
}
function validate(document) {
  assert.equal(document?.schema, 'virgostars-local-day-utc-boundaries/1');
  assert.ok(Array.isArray(document.cases) && document.cases.length > 0);
  assert.ok(typeof document.provenance === 'string' && document.provenance.trim());
  const seen = new Set();
  for (const row of document.cases) {
    assert.ok(typeof row.id === 'string' && row.id.trim());
    assert.ok(validLocalDate(row.localDate), `invalid local date: ${row.id}`);
    assert.ok(typeof row.timeZone === 'string' && row.timeZone.trim(), `missing zone: ${row.id}`);
    assert.ok(typeof row.reference === 'string' && row.reference.trim(), `missing reference: ${row.id}`);
    const key = `${row.timeZone}\0${row.localDate}`;
    assert.ok(!seen.has(key), `duplicate input: ${row.id}`);
    seen.add(key);
    const start = canonicalUtc(row.startUtc);
    const end = canonicalUtc(row.endUtc);
    assert.ok(Number.isFinite(start) && Number.isFinite(end), `malformed UTC bounds: ${row.id}`);
    assert.ok(start < end, `non-increasing bounds: ${row.id}`);
    assert.ok(Number.isSafeInteger(row.durationSeconds) && row.durationSeconds > 0, `bad duration: ${row.id}`);
    assert.equal(end - start, row.durationSeconds * 1000, `duration mismatch: ${row.id}`);
  }
  return document.cases;
}

const expected = Object.freeze({
  'UTC|2026-11-15': ['2026-11-15T00:00:00Z', '2026-11-16T00:00:00Z', 86400],
  'Europe/Rome|2026-03-28': ['2026-03-27T23:00:00Z', '2026-03-28T23:00:00Z', 86400],
  'Europe/Rome|2026-03-29': ['2026-03-28T23:00:00Z', '2026-03-29T22:00:00Z', 82800],
  'Europe/Rome|2026-03-30': ['2026-03-29T22:00:00Z', '2026-03-30T22:00:00Z', 86400],
  'Europe/Rome|2026-10-24': ['2026-10-23T22:00:00Z', '2026-10-24T22:00:00Z', 86400],
  'Europe/Rome|2026-10-25': ['2026-10-24T22:00:00Z', '2026-10-25T23:00:00Z', 90000],
  'Europe/Rome|2026-10-26': ['2026-10-25T23:00:00Z', '2026-10-26T23:00:00Z', 86400],
});

test('golden fixture validates and contains exactly seven distinct date/zone inputs', () => {
  assert.equal(validate(fixture).length, 7);
});
test('every golden UTC interval exactly matches independently prepared reference bounds', () => {
  for (const row of validate(fixture)) {
    assert.deepEqual([row.startUtc, row.endUtc, row.durationSeconds], expected[`${row.timeZone}|${row.localDate}`], row.id);
  }
  assert.equal(Object.keys(expected).length, fixture.cases.length);
});
test('adjacent Rome local days connect without gaps or overlaps at both DST changes', () => {
  for (const [before, change, after] of [[1, 2, 3], [4, 5, 6]]) {
    assert.equal(fixture.cases[before].endUtc, fixture.cases[change].startUtc);
    assert.equal(fixture.cases[change].endUtc, fixture.cases[after].startUtc);
  }
});
test('duplicate local date/time-zone pair fails even with a different fixture ID', () => {
  const changed = structuredClone(fixture);
  changed.cases.push({ ...changed.cases[0], id: 'duplicate' });
  assert.throws(() => validate(changed), /duplicate input/);
});
test('missing zone or missing provenance fails', () => {
  for (const property of ['timeZone', 'reference']) {
    const changed = structuredClone(fixture);
    delete changed.cases[0][property];
    assert.throws(() => validate(changed));
  }
});
test('malformed and noncanonical UTC instants fail closed', () => {
  for (const bad of ['2026-03-29T23:00:00+00:00', '2026-02-30T00:00:00Z', '2026-03-29T22:00Z', '2026-03-29T22:00:60Z']) {
    const changed = structuredClone(fixture);
    changed.cases[2].endUtc = bad;
    assert.throws(() => validate(changed), /malformed UTC bounds/);
  }
});
test('reversed or identical bounds fail closed', () => {
  for (const bad of [fixture.cases[2].startUtc, '2026-03-28T22:00:00Z']) {
    const changed = structuredClone(fixture);
    changed.cases[2].endUtc = bad;
    assert.throws(() => validate(changed), /non-increasing bounds/);
  }
});
test('incorrect elapsed seconds or invalid local calendar date fails closed', () => {
  const wrongDuration = structuredClone(fixture);
  wrongDuration.cases[5].durationSeconds = 86400;
  assert.throws(() => validate(wrongDuration), /duration mismatch/);
  const wrongDate = structuredClone(fixture);
  wrongDate.cases[0].localDate = '2026-02-30';
  assert.throws(() => validate(wrongDate), /invalid local date/);
});
