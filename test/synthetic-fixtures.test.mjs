/*
 * VirgoStars Event Ephemeris — synthetic-fixture tests.
 * CPAL-1.0 Exhibit A: Copyright (c) 2026 Cesare-ml (VirgoStars project).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Distributed under CPAL-1.0 WITHOUT WARRANTY; see LICENSE and NOTICE.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import {
  FIXTURE_SCHEMA, fixtureBytes, makeFixtures, toMicros, validateFixture
} from '../tools/generate-synthetic-fixtures.mjs';

const directory = fileURLToPath(new URL('./fixtures/v0/', import.meta.url));
const cases = makeFixtures();

test('generation is byte-for-byte deterministic and seven fixtures are checked in', () => {
  assert.equal(Object.keys(cases).length, 7);
  for (const [name, doc] of Object.entries(cases)) {
    const file = readFileSync(join(directory, name), 'utf8');
    assert.equal(file, fixtureBytes(doc), name);
    assert.equal(JSON.parse(file).schema, FIXTURE_SCHEMA);
    assert.equal(JSON.parse(file).synthetic, true);
  }
});

test('valid fixtures pass; invalid examples produce expected error codes', () => {
  for (const [name, doc] of Object.entries(cases)) {
    const errors = validateFixture(JSON.parse(readFileSync(join(directory, name), 'utf8')));
    if (doc.expected.valid) assert.deepEqual(errors, [], name);
    else assert.ok(errors.includes(doc.expected.errorCode), name + ': ' + errors.join(','));
  }
});

test('signed epoch microseconds preserve six fractional digits', () => {
  assert.equal(toMicros('1969-12-31T23:59:59.123456Z'), '-876544');
  assert.equal(toMicros('1970-01-01T00:00:00Z'), '0');
  assert.equal(toMicros('1970-01-01T00:00:00.000001Z'), '1');
  assert.throws(() => toMicros('2030-02-30T00:00:00Z'), /INVALID_UTC_INSTANT/);
  assert.throws(() => toMicros('2030-01-01T00:00:00+01:00'), /INVALID_UTC_INSTANT/);
});

test('lunar phases in sample cross month boundaries without reversing order', () => {
  const rows = cases['01-lunar-cycles.json'].records;
  assert.equal(rows.length, 2);
  assert.equal(rows[0].phases.length, 3);
  assert.ok(BigInt(rows[0].phases[2].atUs) < BigInt(rows[1].atUs));
});

test('sign re-entries and DST UTC instants are preserved', () => {
  const ingresses = cases['04-retrograde-ingresses.json'].records.filter(r => r.body === 'Mercury');
  assert.deepEqual(ingresses.map(r => r.sign), [0, 11, 0]);
  const dst = cases['05-boundary-and-dst.json'].records.filter(r => r.family === 'ingress');
  assert.equal((BigInt(dst[1].atUs) - BigInt(dst[0].atUs)).toString(), '3600000000');
});

test('mutation checks reject inconsistent fields and invalid schema', () => {
  const bad = structuredClone(cases['01-lunar-cycles.json']);
  bad.records[0].phases[1].atUs = bad.records[0].phases[0].atUs;
  assert.ok(validateFixture(bad).includes('INVALID_PHASE'));
  const station = structuredClone(cases['03-motion-stations.json']);
  station.records[0].direction = 'unknown';
  assert.ok(validateFixture(station).includes('INVALID_DIRECTION'));
  const corrupted = structuredClone(cases['04-retrograde-ingresses.json']);
  corrupted.records[0].atUs = '0';
  assert.ok(validateFixture(corrupted).includes('TIME_MISMATCH'));
  assert.deepEqual(validateFixture({}), ['INVALID_SCHEMA']);
});
