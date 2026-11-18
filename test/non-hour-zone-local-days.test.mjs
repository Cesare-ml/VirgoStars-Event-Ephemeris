/*
EXHIBIT A. Common Public Attribution License Version 1.0.
The contents of this file are subject to the Common Public Attribution License Version 1.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at https://opensource.org/license/CPAL-1.0.
Software distributed under the License is distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License for the specific language governing rights and limitations under the License.
The Original Code is VirgoStars-Event-Ephemeris source code released under CPAL-1.0 by the Initial Developer, excluding third-party material licensed separately.
The Original Developer is Cesare-ml (VirgoStars project). Copyright (c) 2026 Cesare-ml. All Rights Reserved.
Contributors retain applicable rights in their own contributions under the terms of this License.
SPDX-License-Identifier: CPAL-1.0
*/
// Modification (2026-11-15): fixed IANA non-hour and 30-minute DST boundary oracles.

import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const fixture = JSON.parse(await readFile(new URL('./fixtures/non-hour-zone-local-days.json', import.meta.url), 'utf8'));

// Independently transcribed fixed reference instants; never calculate expected
// bounds using the (currently unimplemented) onDate wrapper under test.
const GOLDEN = Object.freeze({
  'kathmandu-quarter-hour': ['2026-11-15', 'Asia/Kathmandu', '2026-11-14T18:15:00Z', '2026-11-15T18:15:00Z', 86400],
  'chatham-summer-quarter-hour': ['2026-01-15', 'Pacific/Chatham', '2026-01-14T10:15:00Z', '2026-01-15T10:15:00Z', 86400],
  'chatham-winter-quarter-hour': ['2026-07-15', 'Pacific/Chatham', '2026-07-14T11:15:00Z', '2026-07-15T11:15:00Z', 86400],
  'lord-howe-fall-back-half-hour': ['2026-04-05', 'Australia/Lord_Howe', '2026-04-04T13:00:00Z', '2026-04-05T13:30:00Z', 88200],
  'lord-howe-spring-forward-half-hour': ['2026-10-04', 'Australia/Lord_Howe', '2026-10-03T13:30:00Z', '2026-10-04T13:00:00Z', 84600],
});

function utcMillis(value) {
  assert.match(value, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/, 'UTC timestamp must be canonical seconds with Z');
  const milliseconds = Date.parse(value);
  assert.ok(Number.isFinite(milliseconds), 'UTC timestamp must parse');
  assert.equal(new Date(milliseconds).toISOString().replace('.000Z', 'Z'), value, 'UTC timestamp must not normalize invalid fields');
  return milliseconds;
}

function assertOracle(document) {
  assert.equal(document.schema_version, 1, 'fixture version');
  assert.match(document.reference, /IANA tzdb/);
  assert.equal(document.interval_semantics, '[start_utc, end_exclusive_utc)');
  assert.ok(Array.isArray(document.cases));
  assert.equal(document.cases.length, Object.keys(GOLDEN).length, 'exact golden case count');
  const ids = new Set();
  const zoneDates = new Set();
  for (const item of document.cases) {
    assert.ok(item && typeof item === 'object');
    assert.ok(Object.hasOwn(GOLDEN, item.id), 'unexpected golden case');
    assert.ok(!ids.has(item.id), 'duplicate id');
    ids.add(item.id);
    assert.equal(typeof item.iana_zone, 'string');
    assert.ok(item.iana_zone.includes('/'), 'missing IANA zone');
    const key = item.iana_zone + '|' + item.local_date;
    assert.ok(!zoneDates.has(key), 'duplicate zone/local date');
    zoneDates.add(key);
  }
  for (const item of document.cases) {
    assert.deepEqual(
      [item.local_date, item.iana_zone, item.start_utc, item.end_exclusive_utc, item.elapsed_seconds],
      GOLDEN[item.id],
      'independent fixed boundary mismatch: ' + item.id,
    );
    const start = utcMillis(item.start_utc);
    const end = utcMillis(item.end_exclusive_utc);
    assert.ok(end > start, 'reversed or empty half-open interval');
    assert.ok(Number.isSafeInteger(item.elapsed_seconds) && item.elapsed_seconds > 0, 'invalid duration');
    assert.equal((end - start) / 1000, item.elapsed_seconds, 'duration differs from UTC bounds');
  }
}

const clone = () => structuredClone(fixture);

test('five independently fixed non-hour offsets and 30-minute DST golden UTC intervals', () => {
  assertOracle(fixture);
  assert.deepEqual(fixture.cases.map(c => c.elapsed_seconds), [86400, 86400, 86400, 88200, 84600]);
  assert.ok(fixture.cases.slice(0, 3).every(c => /:15:00Z$/.test(c.start_utc)));
});

test('duplicate golden case identifiers are invalid', () => {
  const value = clone();
  value.cases[1] = {...value.cases[0]};
  assert.throws(() => assertOracle(value), /duplicate id/);
});

test('duplicate IANA-zone and local-date keys are invalid', () => {
  const value = clone();
  value.cases[1].iana_zone = value.cases[2].iana_zone;
  value.cases[1].local_date = value.cases[2].local_date;
  assert.throws(() => assertOracle(value), /duplicate zone\/local date/);
});

test('missing IANA zone cannot silently become a host-local date', () => {
  const value = clone();
  value.cases[0].iana_zone = '';
  assert.throws(() => assertOracle(value), /missing IANA zone/);
});

test('reversed UTC bounds fail closed', () => {
  const value = clone();
  const c = value.cases[3];
  [c.start_utc, c.end_exclusive_utc] = [c.end_exclusive_utc, c.start_utc];
  assert.throws(() => assertOracle(value), /independent fixed boundary mismatch/);
});

test('incorrect 23h/25h assumption cannot replace 23.5h/24.5h expectations', () => {
  const value = clone();
  value.cases[3].elapsed_seconds = 90000;
  assert.throws(() => assertOracle(value), /independent fixed boundary mismatch/);
});

test('rounded integer-hour UTC offset fails independent golden parity', () => {
  const value = clone();
  value.cases[0].start_utc = '2026-11-14T18:00:00Z';
  assert.throws(() => assertOracle(value), /independent fixed boundary mismatch/);
});

test('malformed or noncanonical UTC timestamp is rejected', () => {
  assert.throws(() => utcMillis('2026-01-14T10:15:00+00:00'), /canonical seconds with Z/);
  assert.throws(() => utcMillis('2026-02-30T00:00:00Z'), /must not normalize invalid fields/);
});

test('unknown case cannot replace one of the five reference examples', () => {
  const value = clone();
  value.cases[0].id = 'unexpected';
  assert.throws(() => assertOracle(value), /unexpected golden case/);
});
