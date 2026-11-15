/*
 * The contents of this file are subject to the Common Public Attribution License Version 1.0
 * (the "License"); you may not use this file except in compliance with the License.
 * See LICENSE and NOTICE for the CPAL-1.0 Exhibits A/B and contributor rights.
 * Original Developer: Cesare-ml (VirgoStars project), copyright (c) 2026.
 * Powered by VirgoStars — https://www.virgostars.it/
 * Created (2026-11-15): synthetic UTC-microsecond local-day event-membership checks.
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/local-day-event-membership.json', import.meta.url), 'utf8'));
const UTC_MICRO = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})\.(\d{6})Z$/;
const FAMILIES = new Set(['motion', 'ingress']);

// Date parses canonical UTC *whole seconds* only. BigInt keeps all six
// microsecond digits: never represent event timestamps via floating-point ms.
// This is a fixture integrity check, NOT an IANA-zone parser or event reader.
function microseconds(value) {
  assert.equal(typeof value, 'string', 'UTC instant must be a string');
  const match = UTC_MICRO.exec(value);
  assert.ok(match, 'expected exactly six UTC microsecond digits: ' + value);
  const whole = value.slice(0, 19) + 'Z';
  const millis = Date.parse(whole);
  assert.ok(Number.isSafeInteger(millis), 'invalid UTC instant: ' + value);
  assert.equal(new Date(millis).toISOString().slice(0, 19) + 'Z', whole,
    'impossible or noncanonical UTC instant: ' + value);
  return BigInt(millis) * 1000n + BigInt(match[7]);
}

function exactKeys(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'unexpected fixture fields');
}

function validate(document) {
  exactKeys(document, ['schema', 'status', 'sourceLicense', 'attribution', 'provenance', 'tiePolicy', 'events', 'days']);
  assert.equal(document.schema, 'virgostars-local-day-event-membership/1');
  assert.equal(document.sourceLicense, 'CPAL-1.0');
  assert.match(document.attribution, /Powered by VirgoStars.*virgostars\.it/);
  for (const key of ['status', 'provenance', 'tiePolicy'])
    assert.ok(typeof document[key] === 'string' && document[key].trim());
  assert.ok(Array.isArray(document.events) && document.events.length > 0);
  assert.ok(Array.isArray(document.days) && document.days.length > 0);
  const ids = new Set();
  let previous = null;
  for (const event of document.events) {
    exactKeys(event, ['id', 'family', 'atUtc']);
    assert.ok(typeof event.id === 'string' && /^[a-z][a-z0-9-]*$/.test(event.id));
    assert.ok(!ids.has(event.id), 'duplicate event ID: ' + event.id);
    ids.add(event.id);
    assert.ok(FAMILIES.has(event.family), 'unsupported family: ' + event.family);
    const current = microseconds(event.atUtc);
    if (previous !== null) assert.ok(previous < current, 'events must be strictly ordered; tie rule remains unspecified');
    previous = current;
  }
  const seenDays = new Set();
  for (const day of document.days) {
    exactKeys(day, ['localDate', 'timeZone', 'startUtc', 'endUtc', 'expectedEventIds']);
    assert.match(day.localDate, /^2026-(03|10)-\d{2}$/);
    assert.equal(day.timeZone, 'Europe/Rome');
    const key = day.timeZone + '|' + day.localDate;
    assert.ok(!seenDays.has(key), 'duplicate day: ' + key);
    seenDays.add(key);
    const start = microseconds(day.startUtc);
    const end = microseconds(day.endUtc);
    assert.ok(start < end, 'non-increasing UTC interval: ' + key);
    assert.ok(Array.isArray(day.expectedEventIds));
    assert.equal(new Set(day.expectedEventIds).size, day.expectedEventIds.length, 'duplicate expected IDs: ' + key);
    for (const id of day.expectedEventIds) assert.ok(ids.has(id), 'unknown event ID: ' + id);
    const actual = document.events.filter(event => {
      const instant = microseconds(event.atUtc);
      return instant >= start && instant < end;
    }).map(event => event.id);
    assert.deepEqual(day.expectedEventIds, actual, 'incorrect half-open membership: ' + key);
  }
  return document.days;
}

// Independent hard-coded oracle: changes to fixture bounds/IDs must not
// automatically change test expectations. No claim about real event records.
const GOLDEN = Object.freeze({
  '2026-03-28': ['2026-03-27T23:00:00.000000Z', '2026-03-28T23:00:00.000000Z', ['spring-prev-start', 'spring-before-start']],
  '2026-03-29': ['2026-03-28T23:00:00.000000Z', '2026-03-29T22:00:00.000000Z', ['spring-at-start', 'spring-after-start', 'spring-middle', 'spring-before-end']],
  '2026-03-30': ['2026-03-29T22:00:00.000000Z', '2026-03-30T22:00:00.000000Z', ['spring-at-end', 'spring-after-end', 'spring-next-before-end']],
  '2026-10-24': ['2026-10-23T22:00:00.000000Z', '2026-10-24T22:00:00.000000Z', ['autumn-prev-start', 'autumn-before-start']],
  '2026-10-25': ['2026-10-24T22:00:00.000000Z', '2026-10-25T23:00:00.000000Z', ['autumn-at-start', 'autumn-after-start', 'autumn-middle', 'autumn-before-end']],
  '2026-10-26': ['2026-10-25T23:00:00.000000Z', '2026-10-26T23:00:00.000000Z', ['autumn-at-end', 'autumn-after-end', 'autumn-next-before-end']],
});

test('six consecutive spring/autumn days validate against fixed event expectations', () => {
  const days = validate(fixture);
  assert.equal(days.length, 6);
  assert.equal(fixture.events.length, 20);
  for (const day of days) {
    assert.deepEqual([day.startUtc, day.endUtc, day.expectedEventIds], GOLDEN[day.localDate], day.localDate);
  }
  assert.equal(Object.keys(GOLDEN).length, days.length);
});

test('spring and autumn local day lengths are 23 and 25 hours, not 24', () => {
  const days = validate(fixture);
  const byDate = new Map(days.map(day => [day.localDate, day]));
  assert.equal((microseconds(byDate.get('2026-03-29').endUtc) - microseconds(byDate.get('2026-03-29').startUtc)) / 1000000n, 82800n);
  assert.equal((microseconds(byDate.get('2026-10-25').endUtc) - microseconds(byDate.get('2026-10-25').startUtc)) / 1000000n, 90000n);
});

test('all paired neighboring days join without gaps or double counting', () => {
  for (const trio of [[0, 1, 2], [3, 4, 5]]) {
    const [previous, transition, next] = trio.map(index => fixture.days[index]);
    assert.equal(previous.endUtc, transition.startUtc);
    assert.equal(transition.endUtc, next.startUtc);
    for (const [a, b] of [[previous, transition], [transition, next]])
      assert.deepEqual(a.expectedEventIds.filter(id => b.expectedEventIds.includes(id)), []);
  }
});

test('one microsecond separates each transition end from its included previous event', () => {
  const events = new Map(fixture.events.map(event => [event.id, event]));
  for (const [day, beforeEnd, atEnd] of [
    [fixture.days[1], 'spring-before-end', 'spring-at-end'],
    [fixture.days[4], 'autumn-before-end', 'autumn-at-end'],
  ]) {
    assert.equal(microseconds(day.endUtc) - microseconds(events.get(beforeEnd).atUtc), 1n);
    assert.equal(microseconds(day.endUtc), microseconds(events.get(atEnd).atUtc));
    assert.ok(day.expectedEventIds.includes(beforeEnd));
    assert.ok(!day.expectedEventIds.includes(atEnd));
  }
});

test('microsecond precision survives +1 and -1 operations without Number rounding', () => {
  assert.equal(microseconds('2026-03-28T23:00:00.000001Z') - microseconds('2026-03-28T23:00:00.000000Z'), 1n);
  assert.equal(microseconds('2026-10-25T23:00:00.000000Z') - microseconds('2026-10-25T22:59:59.999999Z'), 1n);
});

test('rejects duplicate IDs and unordered event instants', () => {
  const duplicated = structuredClone(fixture);
  duplicated.events[1].id = duplicated.events[0].id;
  assert.throws(() => validate(duplicated), /duplicate event ID/);
  const unordered = structuredClone(fixture);
  [unordered.events[1], unordered.events[2]] = [unordered.events[2], unordered.events[1]];
  assert.throws(() => validate(unordered), /strictly ordered/);
});

test('rejects noncanonical, impossible and lossy UTC microsecond timestamps', () => {
  for (const bad of [
    '2026-03-29T22:00:00Z',
    '2026-03-29T22:00:00.000Z',
    '2026-03-29T22:00:00.000000+00:00',
    '2026-02-30T22:00:00.000000Z',
    '2026-03-29T22:00:60.000000Z',
  ]) {
    const changed = structuredClone(fixture);
    changed.events[4].atUtc = bad;
    assert.throws(() => validate(changed));
  }
});

test('rejects incorrect half-open expectations and stale local-day bounds', () => {
  const wrongMembership = structuredClone(fixture);
  wrongMembership.days[1].expectedEventIds.push('spring-at-end');
  assert.throws(() => validate(wrongMembership), /incorrect half-open membership/);
  const wrongBounds = structuredClone(fixture);
  wrongBounds.days[1].startUtc = wrongBounds.days[1].endUtc;
  assert.throws(() => validate(wrongBounds), /non-increasing/);
  const wrongGolden = structuredClone(fixture);
  wrongGolden.days[1].expectedEventIds = wrongGolden.days[1].expectedEventIds.slice(1);
  assert.notDeepEqual(wrongGolden.days[1].expectedEventIds, GOLDEN['2026-03-29'][2]);
});

test('rejects wrong family, unknown IDs, duplicated days and private fields', () => {
  const wrongFamily = structuredClone(fixture);
  wrongFamily.events[0].family = 'synod';
  assert.throws(() => validate(wrongFamily), /unsupported family/);
  const missing = structuredClone(fixture);
  missing.days[0].expectedEventIds[0] = 'not-an-event';
  assert.throws(() => validate(missing), /unknown event ID/);
  const duplicatedDay = structuredClone(fixture);
  duplicatedDay.days.push(structuredClone(duplicatedDay.days[0]));
  assert.throws(() => validate(duplicatedDay), /duplicate day/);
  const extra = structuredClone(fixture);
  extra.events[0].agentId = 'secret';
  assert.throws(() => validate(extra), /unexpected fixture fields/);
});
