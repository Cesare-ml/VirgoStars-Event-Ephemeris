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
const { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const reviewer = path.resolve(__dirname, '../vseph_review_latitude_goldens.cjs');
const commit = 'a'.repeat(40);
const reference = Object.freeze({
  provider: 'SYNTHETIC independent oracle (NOT ASTRONOMICAL)',
  version: 'fixture-v1',
  origin: 'inline-test-only',
  method: 'constant fixture (no astronomy)',
  source_license: 'synthetic-test-data',
  frame: 'geocentric-ecliptic-latitude-degrees',
  ecliptic_plane: 'true-ecliptic-and-equinox-of-date (IAU-2006/2000B)',
  position_corrections: 'one-way-light-time;solar-deflection;annual-aberration;date-frame-rotation',
  dynamical_time_scale: 'TDB (approximate from civil UTC; signed rate per TDB day)',
  input_time_scale: 'utc-posix-microseconds',
  speed_definition: 'signed-d-beta-dt-degrees-per-day',
});
const samples = Object.freeze([
  { body: 'Mars', utc_microseconds: '-315619200000000',
    latitude_degrees: 1.25, latitude_speed_degrees_per_day: -0.125 },
  { body: 'Moon', utc_microseconds: '946684800000000',
    latitude_degrees: -2.5, latitude_speed_degrees_per_day: 0.375 },
]);

function sha(bytes) { return createHash('sha256').update(bytes).digest('hex'); }

function fixture(t, engineSource = null) {
  const directory = mkdtempSync(path.join(tmpdir(), 'vseph-lat-golden-'));
  t.after(() => rmSync(directory, { force: true, recursive: true }));
  const kernel = path.join(directory, 'tiny-FAKE-not-JPL.eph');
  const binding = path.join(directory, 'index.js');
  const addon = path.join(directory, 'native', 'virgostars_ephemeris.node');
  const goldens = path.join(directory, 'independent-FAKE-goldens.json');
  const report = path.join(directory, 'out.json');
  const kernelBytes = Buffer.from('NOT DE441');
  writeFileSync(kernel, kernelBytes);
  mkdirSync(path.dirname(addon));
  writeFileSync(addon, 'NOT A COMPILED ADDON');
  writeFileSync(binding, engineSource || `
    exports.Engine = class Engine {
      calcUt(jd, body, flags) {
        if (flags !== 257) throw Error('unexpected flags');
        const coord = ({ 4:[1.25,-0.125], 1:[-2.5,0.375] })[body];
        if (!coord) throw Error('unexpected body');
        return { xx: [0, coord[0], 1, 0, coord[1], 0], rflag:257,
          timeSource: jd < 2441317.5 ? 'DELTA_T_POLYNOMIAL:FAKE' : 'IERS_LEAP_SECONDS' };
      }
    };
  `);
  const golden = { schema: 'vseph.g3e.latitude-goldens.v1', reference: { ...reference },
    samples: samples.map(x => ({ ...x })) };
  function save() {
    const bytes = JSON.stringify(golden) + '\n';
    writeFileSync(goldens, bytes);
    return sha(bytes);
  }
  return { kernel, binding, addon, goldens, golden, report, kernelSha: sha(kernelBytes), save };
}

function run(f, goldenSha, extra = [], overrides = {}) {
  return spawnSync(process.execPath, [reviewer,
    '--binding', f.binding, '--kernel', f.kernel,
    '--kernel-sha256', overrides.kernelSha ?? f.kernelSha,
    '--source-commit', commit,
    '--goldens', f.goldens,
    '--goldens-sha256', goldenSha,
    '--latitude-tolerance-deg', overrides.latitudeTolerance ?? '0.001',
    '--speed-tolerance-deg-per-day', overrides.speedTolerance ?? '0.0001',
    ...extra], { encoding: 'utf8', timeout: 10000 });
}

test('two synthetic golden epochs agree and report explicitly disclaims scientific approval', t => {
  const f = fixture(t);
  const goldenSha = f.save();
  const result = run(f, goldenSha, ['--report', f.report]);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.schema, 'vseph.g3e.latitude-golden-review.v1');
  assert.equal(report.golden_sha256, goldenSha);
  assert.equal(report.compared_samples, 2);
  assert.equal(report.discrepancies.length, 0);
  assert.equal(report.conclusions.observations_within_tolerance, true);
  for (const name of ['reference_independence_scientifically_established',
    'astronomical_accuracy_verified', 'coverage_certified', 'safe_to_import']) {
    assert.equal(report.conclusions[name], false);
  }
  assert.equal(readFileSync(f.report, 'utf8'), result.stdout);
});

test('latitude and signed-speed biases return exit 1 and preserve explicit signed residuals', t => {
  const f = fixture(t);
  f.golden.samples[0].latitude_degrees += 0.005; // primary minus reference is negative
  f.golden.samples[1].latitude_speed_degrees_per_day -= 0.007; // primary minus reference positive
  const goldenSha = f.save();
  const result = run(f, goldenSha, ['--report', f.report]);
  assert.equal(result.status, 1, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.discrepancies.length, 2);
  assert.ok(report.discrepancies[0].actual_minus_golden_latitude_degrees < 0);
  assert.ok(report.discrepancies[1].actual_minus_golden_speed_degrees_per_day > 0);
  assert.equal(report.conclusions.observations_within_tolerance, false);
  assert.equal(report.conclusions.coverage_certified, false);
  assert.equal(readFileSync(f.report, 'utf8'), result.stdout);
});

test('wrong golden/kernel hash fails before report and leaves empty stdout', t => {
  const f = fixture(t);
  const goldenSha = f.save();
  for (const override of [{ golden: '0'.repeat(64) }, { kernel: '1'.repeat(64) }]) {
    const result = run(f, override.golden ?? goldenSha, ['--report', f.report],
      { kernelSha: override.kernel });
    assert.equal(result.status, 2);
    assert.equal(result.stdout, '');
  }
});

test('pinned golden JSON rejects an escaped duplicate schema key', t => {
  const f = fixture(t);
  f.save();
  const original = readFileSync(f.goldens, 'utf8');
  const ambiguous = original.replace('"schema":',
    String.raw`"\u0073chema":"shadow","schema":`);
  writeFileSync(f.goldens, ambiguous);
  const result = run(f, sha(ambiguous));
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /duplicate JSON object keys/);
});

test('duplicate, outside supported UTC, malformed sign and nonfinite tolerance fail closed', t => {
  const f = fixture(t);
  for (const modifier of [
    g => g.samples.push({ ...g.samples[0] }),
    g => { g.samples[0].utc_microseconds = String(Date.parse('2027-06-28T00:00:00Z') * 1000); },
    g => { g.samples[0].utc_microseconds = '000001'; },
    g => { g.samples[0].latitude_degrees = 91; },
    g => { g.reference.source_license = ''; },
  ]) {
    f.golden.samples = samples.map(x => ({ ...x }));
    f.golden.reference = { ...reference };
    modifier(f.golden);
    const result = run(f, f.save());
    assert.equal(result.status, 2, result.stdout);
    assert.equal(result.stdout, '');
  }
  const badTolerance = run(f, f.save(), [], { latitudeTolerance: 'NaN' });
  assert.equal(badTolerance.status, 2);
  assert.equal(badTolerance.stdout, '');
});

test('declared ecliptic reference mismatch or bad native time policy rejects without report', t => {
  const f = fixture(t);
  f.golden.reference.ecliptic_plane = 'mean-ecliptic-of-date';
  assert.equal(run(f, f.save()).status, 2);
  const bad = fixture(t, `exports.Engine = class { calcUt() {
    return { xx: [0,1,1,0,1,0], rflag:257, timeSource:'INVALID' }; } };`);
  assert.equal(run(bad, bad.save()).status, 2);
});

test('create-only report is not overwritten even if comparison succeeds', t => {
  const f = fixture(t);
  const goldenSha = f.save();
  writeFileSync(f.report, 'existing work');
  const result = run(f, goldenSha, ['--report', f.report]);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  assert.equal(readFileSync(f.report, 'utf8'), 'existing work');
});
