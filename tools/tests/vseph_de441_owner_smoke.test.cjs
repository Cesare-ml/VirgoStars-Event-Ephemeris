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
const { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const smoke = path.resolve(__dirname, '../vseph_de441_owner_smoke.cjs');
const revision = 'a'.repeat(40);

function fixture(t, overrideEngine = null) {
  const directory = mkdtempSync(path.join(tmpdir(), 'vseph-g3e-smoke-'));
  t.after(() => rmSync(directory, { force: true, recursive: true }));
  const kernel = path.join(directory, 'SYNTHETIC-NOT-DE441.eph');
  const binding = path.join(directory, 'index.js');
  const addon = path.join(directory, 'native', 'virgostars_ephemeris.node');
  const report = path.join(directory, 'smoke.json');
  const bytes = Buffer.from('not a real JPL kernel');
  writeFileSync(kernel, bytes);
  mkdirSync(path.dirname(addon));
  writeFileSync(addon, 'not a native binary');
  writeFileSync(binding, overrideEngine || `
    exports.Engine = class Engine {
      calcUt(_jd, _body, flags) {
        if (flags !== 257) throw Error('wrong flags');
        const timeSource = _jd < 2441317.5 ? 'DELTA_T_POLYNOMIAL:SYNTHETIC' : 'IERS_LEAP_SECONDS';
        return { xx: [0, 1.25, 1, 0, -0.125, 0], rflag: 257, timeSource };
      }
    };
  `);
  const sha = createHash('sha256').update(bytes).digest('hex');
  return { binding, addon, kernel, report, sha };
}

function run(f, extra = [], sha = f.sha, rev = revision) {
  return spawnSync(process.execPath, [smoke, '--binding', f.binding, '--kernel', f.kernel,
    '--kernel-sha256', sha, '--source-commit', rev, ...extra], {
    encoding: 'utf8', timeout: 10000,
  });
}

function requests(f, probes) {
  const file = path.join(path.dirname(f.report), 'requests.json');
  const bytes = JSON.stringify({ schema: 'vseph.g3e.owner-probe-requests.v1', probes }) + '\n';
  writeFileSync(file, bytes);
  const sha = createHash('sha256').update(bytes).digest('hex');
  return { file, sha };
}

test('cross-platform synthetic owner smoke captures three signed-speed probes and explicit uncertified flags', t => {
  const f = fixture(t);
  const result = run(f, ['--report', f.report]);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.schema, 'vseph.g3e.owner-smoke.v1');
  assert.equal(report.identity.version, revision);
  assert.equal(report.identity.kernel_digest, f.sha);
  assert.match(report.identity.source_digest, /^[0-9a-f]{64}$/);
  assert.equal(report.executed_utc_probes.length, 3);
  assert.deepEqual(report.executed_utc_probes.map(row => row.body), ['Mars', 'Moon', 'Jupiter']);
  assert.deepEqual(report.executed_utc_probes.map(row => row.latitude_speed_degrees_per_day), [-0.125, -0.125, -0.125]);
  assert.equal(report.result.protocol_smoke_passed, true);
  for (const key of ['native_binary_source_commit_attested', 'independent_provider_compared',
    'astronomical_accuracy_verified', 'coverage_certified', 'safe_to_import']) {
    assert.equal(report.result[key], false);
  }
  assert.equal(readFileSync(f.report, 'utf8'), result.stdout);
});

test('wrong kernel trust pin or malformed source commit produces no report or stdout', t => {
  const f = fixture(t);
  for (const [sha, rev] of [['0'.repeat(64), revision], [f.sha, 'not-a-commit']]) {
    const result = run(f, ['--report', f.report], sha, rev);
    assert.equal(result.status, 2);
    assert.equal(result.stdout, '');
  }
});

test('missing addon and incorrect sample/native time policy both fail closed', t => {
  const f = fixture(t);
  rmSync(f.addon);
  assert.equal(run(f).status, 2);
  const bad = fixture(t, `exports.Engine = class { calcUt() {
    return { xx:[0,0,0,0,0,0], rflag:257, timeSource:'INVALID' }; } };`);
  const result = run(bad);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
});

test('report is create-only: existing report is not overwritten', t => {
  const f = fixture(t);
  writeFileSync(f.report, 'original material');
  const result = run(f, ['--report', f.report]);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  assert.equal(readFileSync(f.report, 'utf8'), 'original material');
});

test('owner smoke accepts bounded pinned UTC probes without calling them independent goldens', t => {
  const f = fixture(t);
  const extra = requests(f, [
    { body: 'Venus', utc_microseconds: '-2208988800000000' },
    { body: 'Saturn', utc_microseconds: '1514764800000000' },
  ]);
  const result = run(f, ['--probe-requests', extra.file,
    '--probe-requests-sha256', extra.sha, '--report', f.report]);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.schema, 'vseph.g3e.owner-smoke.v2');
  assert.equal(report.fixed_probe_count, 3);
  assert.equal(report.requested_probe_count, 2);
  assert.equal(report.probe_requests_sha256, extra.sha);
  assert.equal(report.executed_utc_probes.length, 5);
  assert.deepEqual(report.executed_utc_probes.slice(3).map(p => p.body), ['Venus', 'Saturn']);
  assert.equal(report.result.independent_provider_compared, false);
  assert.equal(report.result.astronomical_accuracy_verified, false);
  assert.equal(report.result.coverage_certified, false);
  assert.equal(readFileSync(f.report, 'utf8'), result.stdout);
});

test('missing request SHA and altered correctly-shaped requests fail before provider output', t => {
  const f = fixture(t);
  const extra = requests(f, [{ body: 'Mars', utc_microseconds: '946684800000000' }]);
  let result = run(f, ['--probe-requests', extra.file]);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  writeFileSync(extra.file, JSON.stringify({ schema: 'vseph.g3e.owner-probe-requests.v1',
    probes: [{ body: 'Moon', utc_microseconds: '946684800000000' }] }));
  result = run(f, ['--probe-requests', extra.file, '--probe-requests-sha256', extra.sha]);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
});

test('duplicate, out-of-policy, malformed and fixed-probe repeats fail closed', t => {
  const f = fixture(t);
  const cases = [
    [{ body: 'Mars', utc_microseconds: '-315619200000000' }],
    [{ body: 'Moon', utc_microseconds: '1814313600000000' }],
    [{ body: 'Mars', utc_microseconds: '0946684800000000' }],
    [{ body: 'Unknown', utc_microseconds: '946684800000000' }],
    [{ body: 'Mars', utc_microseconds: '946684800000000' },
      { body: 'Mars', utc_microseconds: '946684800000000' }],
  ];
  for (const probes of cases) {
    const extra = requests(f, probes);
    const result = run(f, ['--probe-requests', extra.file, '--probe-requests-sha256', extra.sha]);
    assert.equal(result.status, 2, result.stdout);
    assert.equal(result.stdout, '');
  }
});
