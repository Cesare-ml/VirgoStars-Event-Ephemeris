/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const bridge = path.resolve(__dirname, '../vseph_virgostars_latitude_provider.cjs');
const revision = 'a'.repeat(40);

function fixture(t, engineSource = null) {
  const dir = mkdtempSync(path.join(tmpdir(), 'vseph-lat-bridge-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const kernel = path.join(dir, 'tiny-SYNTHETIC-not-DE441.eph');
  const binding = path.join(dir, 'index.js');
  const addon = path.join(dir, 'native', 'virgostars_ephemeris.node');
  const bytes = Buffer.from('NOT AN EPHEMERIS: synthetic test only');
  const addonBytes = Buffer.from('SYNTHETIC ADDON FINGERPRINT, NEVER A REAL NATIVE BINARY');
  writeFileSync(kernel, bytes);
  mkdirSync(path.dirname(addon));
  writeFileSync(addon, addonBytes);
  writeFileSync(binding, engineSource || `
    exports.Engine = class Engine {
      constructor(file) { if (!file.endsWith('.eph')) throw Error('wrong file'); }
      calcUt(jd, body, flags) {
        if (flags !== 257 || body !== 4) throw Error('wrong target or flags');
        return { xx: [0, 1.25, 1, 0, -0.125, 0], rflag: 257,
          timeSource: jd < 2441317.5 ? 'DELTA_T_POLYNOMIAL:TEST:42' : 'IERS_LEAP_SECONDS',
          notes: [] };
      }
    };
  `);
  const sha = createHash('sha256').update(bytes).digest('hex');
  const loaderSha = createHash('sha256').update(require('node:fs').readFileSync(binding)).digest('hex');
  const addonSha = createHash('sha256').update(addonBytes).digest('hex');
  const sourceDigest = createHash('sha256')
    .update('virgostars-node-binding-v1\0').update(loaderSha).update('\0').update(addonSha).digest('hex');
  return { binding, addon, kernel, sha, sourceDigest };
}

function run(f, requests, overrides = {}) {
  return spawnSync(process.execPath, [bridge, f.binding, f.kernel,
    overrides.sha ?? f.sha, overrides.revision ?? revision], {
    input: requests.map(r => JSON.stringify(r)).join('\n') + '\n',
    encoding: 'utf8',
    timeout: 10000,
  });
}

test('real executable transport calls pinned native API fields without inventing events', t => {
  const f = fixture(t);
  const us = '1731283200123456';
  const result = run(f, [{ body: 'Mars', utc_microseconds: us }]);
  assert.equal(result.status, 0, result.stderr);
  const [hello, sample] = result.stdout.trim().split('\n').map(JSON.parse);
  assert.equal(hello.protocol, 'virgostars.latitude.v1');
  assert.equal(hello.kernel_digest, f.sha);
  assert.equal(hello.version, revision);
  assert.equal(hello.source_digest, f.sourceDigest);
  assert.equal(hello.ecliptic_plane, 'true-ecliptic-and-equinox-of-date (IAU-2006/2000B)');
  assert.deepEqual(sample, {
    body: 'Mars', utc_microseconds: us,
    latitude_degrees: 1.25, latitude_speed_degrees_per_day: -0.125,
  });
});

test('historical signed timestamps use declared historical time policy', t => {
  const f = fixture(t);
  const result = run(f, [{ body: 'Mars', utc_microseconds: '-315619200000000' }]); // 1960-01-01
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout.trim().split('\n')[1]).utc_microseconds, '-315619200000000');
});

test('wrong trusted hash or unpinned source refuses to announce an identity', t => {
  const f = fixture(t);
  const r = { body: 'Mars', utc_microseconds: '1731283200000000' };
  for (const overrides of [{ sha: '0'.repeat(64) }, { revision: 'unreviewed' }]) {
    const result = run(f, [r], overrides);
    assert.equal(result.status, 2);
    assert.equal(result.stdout, '');
  }
});

test('binding fingerprint changes on addon byte changes and absent addon fails before identity', t => {
  const f = fixture(t);
  const request = [{ body: 'Mars', utc_microseconds: '1731283200000000' }];
  writeFileSync(f.addon, Buffer.from('DIFFERENT SYNTHETIC ADDON'));
  const changed = run(f, request);
  assert.equal(changed.status, 0, changed.stderr);
  assert.notEqual(JSON.parse(changed.stdout.split('\n')[0]).source_digest, f.sourceDigest);
  rmSync(f.addon);
  const missing = run(f, request);
  assert.equal(missing.status, 2);
  assert.equal(missing.stdout, '');
});

test('invalid body, uncatalogued UTC and malformed echoed time abort instead of fabricating', t => {
  const f = fixture(t);
  for (const request of [
    { body: 'Mars', utc_microseconds: '-2208988800000001' },
    { body: 'Mars', utc_microseconds: '1900000000000000' },
    { body: 'Mars', utc_microseconds: '00001' },
    { body: 'Mars', utc_microseconds: 100 },
    { body: 'Earth', utc_microseconds: '1731283200000000' },
  ]) {
    const result = run(f, [request]);
    assert.equal(result.status, 2, JSON.stringify(request));
    assert.equal(result.stdout.trim().split('\n').length, 1, 'only hello, no invented sample');
  }
});

test('wrong native time policy, speed flags and nonfinite fields reject samples', t => {
  const badSources = [
    'return { xx: [0,1,1,0,0,0], rflag:1, timeSource:"IERS_LEAP_SECONDS" };',
    'return { xx: [0,1,1,0,0,0], rflag:257, timeSource:"TDB_INPUT" };',
    'return { xx: [0,NaN,1,0,0,0], rflag:257, timeSource:"IERS_LEAP_SECONDS" };',
  ];
  for (const source of badSources) {
    const f = fixture(t, `exports.Engine = class { calcUt(){ ${source} } };`);
    const result = run(f, [{ body: 'Mars', utc_microseconds: '1731283200000000' }]);
    assert.equal(result.status, 2);
    assert.equal(result.stdout.trim().split('\n').length, 1);
  }
});
