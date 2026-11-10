/*
 * VirgoStars Event Ephemeris — synthetic test fixtures only.
 * CPAL-1.0 Exhibit A: Copyright (c) 2026 Cesare-ml (VirgoStars project).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Distributed under CPAL-1.0 WITHOUT WARRANTY; see LICENSE and NOTICE.
 */
import { fileURLToPath } from 'node:url';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export const FIXTURE_SCHEMA = 'vseph-synthetic-fixtures/v0';

// Test-only UTC conversion, not the production reader or an astronomical calculator.
export function toMicros(iso) {
  const match = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,6}))?Z$/.exec(iso);
  if (!match) throw new Error('INVALID_UTC_INSTANT: ' + iso);
  const digits = (match[2] || '').padEnd(6, '0');
  const millisIso = match[1] + '.' + digits.slice(0, 3) + 'Z';
  const millis = Date.parse(millisIso);
  if (!Number.isFinite(millis) || new Date(millis).toISOString() !== millisIso) {
    throw new Error('INVALID_UTC_INSTANT: ' + iso);
  }
  return (BigInt(millis) * 1000n + BigInt(digits.slice(3))).toString();
}

function event(family, body, at, fields = {}) {
  const record = { family, body, at, atUs: toMicros(at), ...fields };
  if (Array.isArray(record.phases)) {
    record.phases = record.phases.map(p => ({ name: p.name, at: p.at, atUs: toMicros(p.at) }));
  }
  return record;
}

function fixture(id, expected, rows) {
  const records = [...rows].sort((a, b) =>
    a.family.localeCompare(b.family, 'en') ||
    a.body.localeCompare(b.body, 'en') ||
    (BigInt(a.atUs) < BigInt(b.atUs) ? -1 : BigInt(a.atUs) > BigInt(b.atUs) ? 1 : 0)
  );
  return { schema: FIXTURE_SCHEMA, synthetic: true, id, expected, records };
}

export function makeFixtures() {
  return {
    '01-lunar-cycles.json': fixture('lunar-cycles', { valid: true }, [
      event('synod', 'Moon', '2030-03-25T00:00:00.123456Z', { phases: [
        { name: 'firstQuarter', at: '2030-04-01T00:00:00Z' },
        { name: 'fullMoon', at: '2030-04-08T00:00:00Z' },
        { name: 'lastQuarter', at: '2030-04-15T00:00:00Z' }
      ] }),
      event('synod', 'Moon', '2030-04-23T00:00:00Z', { phases: [
        { name: 'firstQuarter', at: '2030-04-30T00:00:00Z' },
        { name: 'fullMoon', at: '2030-05-07T00:00:00Z' },
        { name: 'lastQuarter', at: '2030-05-14T00:00:00Z' }
      ] })
    ]),
    '02-optional-and-historical.json': fixture('optional-and-historical', { valid: true }, [
      event('synod', 'true Node', '1900-01-01T00:00:00Z', { phases: null }),
      event('synod', 'osc. Apogee', '1969-12-31T23:59:59.123456Z', { phases: [
        { name: 'conjunction2', at: '1970-01-02T00:00:00Z' },
        { name: 'conjunction3', at: '1970-01-03T00:00:00Z' }
      ] }),
      event('synod', 'Mercury', '2026-10-01T00:00:00Z', { phases: [
        { name: 'R2', at: '2026-10-05T00:00:00Z' },
        { name: 'R3', at: '2026-10-10T00:00:00Z' },
        { name: 'D4', at: '2026-10-15T00:00:00Z' }
      ] })
    ]),
    '03-motion-stations.json': fixture('motion-stations', { valid: true }, [
      event('motion', 'Mars', '2026-10-24T23:30:00Z', { direction: 'retrograde', sign: 4 }),
      event('motion', 'Mars', '2026-10-25T01:30:00Z', { direction: 'direct', sign: 4 }),
      event('motion', 'Saturn', '2026-10-25T01:30:00Z', { direction: 'retrograde', sign: 11 })
    ]),
    '04-retrograde-ingresses.json': fixture('retrograde-ingresses', { valid: true }, [
      event('ingress', 'Mercury', '2030-01-01T00:00:00Z', { sign: 0 }),
      event('ingress', 'Mercury', '2030-01-05T00:00:00Z', { sign: 11 }),
      event('ingress', 'Mercury', '2030-01-10T00:00:00Z', { sign: 0 }),
      event('ingress', 'Moon', '2030-01-10T23:59:59.999999Z', { sign: 2 })
    ]),
    '05-boundary-and-dst.json': fixture('boundary-and-dst', { valid: true }, [
      event('ingress', 'Moon', '2026-10-25T00:30:00Z', { sign: 3 }),
      event('ingress', 'Moon', '2026-10-25T01:30:00Z', { sign: 4 }),
      event('motion', 'Mercury', '2026-10-25T01:30:00Z', { direction: 'direct', sign: 4 }),
      event('synod', 'Moon', '2026-10-25T01:30:00Z', { phases: [] })
    ]),
    '06-invalid-duplicate.json': fixture('invalid-duplicate', { valid: false, errorCode: 'DUPLICATE_EVENT' }, [
      event('motion', 'Venus', '2050-01-01T00:00:00Z', { direction: 'direct', sign: 1 }),
      event('motion', 'Venus', '2050-01-01T00:00:00Z', { direction: 'retrograde', sign: 1 })
    ]),
    '07-invalid-sign.json': fixture('invalid-sign', { valid: false, errorCode: 'SIGN_RANGE' }, [
      event('ingress', 'Venus', '2050-01-01T00:00:00Z', { sign: 12 })
    ])
  };
}

export function validateFixture(doc) {
  const errors = [];
  if (!doc || doc.schema !== FIXTURE_SCHEMA || doc.synthetic !== true || !Array.isArray(doc.records)) {
    return ['INVALID_SCHEMA'];
  }
  const seen = new Set();
  const previous = new Map();
  for (const item of doc.records) {
    if (!['synod', 'motion', 'ingress'].includes(item.family) || typeof item.body !== 'string' || !item.body) {
      errors.push('INVALID_EVENT'); continue;
    }
    let instant;
    try {
      const parsed = toMicros(item.at);
      if (parsed !== item.atUs) errors.push('TIME_MISMATCH');
      instant = BigInt(parsed);
    } catch {
      errors.push('INVALID_UTC_INSTANT'); continue;
    }
    const series = item.family + ':' + item.body;
    const key = series + ':' + instant;
    if (seen.has(key)) errors.push('DUPLICATE_EVENT');
    seen.add(key);
    if (previous.has(series) && instant < previous.get(series)) errors.push('UNSORTED_EVENT');
    previous.set(series, instant);
    if (item.family === 'synod') {
      if (!(item.phases === null || Array.isArray(item.phases))) {
        errors.push('INVALID_PHASES');
      } else if (Array.isArray(item.phases)) {
        let prior = instant;
        for (const p of item.phases) {
          try {
            const phaseInstant = BigInt(toMicros(p.at));
            if (!p.name || p.atUs !== phaseInstant.toString()) errors.push('INVALID_PHASE');
            if (phaseInstant <= prior) errors.push('PHASE_ORDER');
            prior = phaseInstant;
          } catch { errors.push('INVALID_PHASE'); }
        }
      }
    } else {
      if (!Number.isInteger(item.sign) || item.sign < 0 || item.sign > 11) errors.push('SIGN_RANGE');
      if (item.family === 'motion' && !['direct', 'retrograde'].includes(item.direction)) {
        errors.push('INVALID_DIRECTION');
      }
    }
  }
  return errors;
}

export function fixtureBytes(doc) {
  return JSON.stringify(doc, null, 2) + '\n';
}

export function writeFixtures(root = new URL('../test/fixtures/v0/', import.meta.url)) {
  const directory = fileURLToPath(root);
  mkdirSync(directory, { recursive: true });
  for (const [name, doc] of Object.entries(makeFixtures())) {
    writeFileSync(join(directory, name), fixtureBytes(doc), 'utf8');
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  writeFixtures();
}
