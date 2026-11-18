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
const { mkdtempSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const reviewer = path.resolve(__dirname, '../vseph_review_latitude_event_goldens.cjs');
const kinds = [
  'PlaneAscending', 'PlaneDescending', 'StationNorthward',
  'StationSouthward', 'NorthwardSpeedMaximum', 'SouthwardSpeedMagnitudeMaximum',
];
const conventions = {
  frame: 'geocentric-ecliptic-latitude-degrees',
  ecliptic_plane: 'true-ecliptic-and-equinox-of-date (IAU-2006/2000B)',
  position_corrections: 'one-way-light-time;solar-deflection;annual-aberration;date-frame-rotation',
  dynamical_time_scale: 'TDB (approximate from civil UTC; signed rate per TDB day)',
  input_time_scale: 'utc-posix-microseconds',
  speed_definition: 'signed-d-beta-dt-degrees-per-day',
};

function hash(value) { return createHash('sha256').update(value).digest('hex'); }
function fixture(t) {
  const dir = mkdtempSync(path.join(tmpdir(), 'vseph-g3e-event-golden-'));
  t.after(() => rmSync(dir, { force: true, recursive: true }));
  const auditPath = path.join(dir, 'audit.json');
  const goldensPath = path.join(dir, 'reference.json');
  const reportPath = path.join(dir, 'report.json');
  const candidates = kinds.map((kind, index) => {
    const base = (index + 1) * 1000000;
    return {
      body: 'Mars', kind,
      bracket_start_us: String(base - 10),
      bracket_end_us: String(base + 10),
      signed_speed_degrees_per_day: index === 4 ? 0.5 : index === 5 ? -0.7 : null,
    };
  });
  const audit = {
    kind: 'experimental_latitude_provider_comparison', body: 'Mars', mode: 'events',
    window: { start_us: '0', end_us: '10000000', step_us: '1000000', precision_us: '10' },
    primary_provider: { provider: 'FAKE primary', version: 'fixture', source_license: 'synthetic-only', ...conventions },
    reference_provider: { provider: 'FAKE secondary', version: 'fixture', ...conventions },
    events: { pairs: candidates.map(c => ({ primary: c, reference: { ...c },
      reference_offset_us: '0', signed_speed_offset_degrees_per_day: null, speed_within_tolerance: null })),
    unmatched_primary: [], unmatched_reference: [] },
    provider_independence_verified: false, astronomical_accuracy_verified: false,
    coverage_certified: false, safe_to_import: false,
  };
  const goldens = {
    schema: 'vseph.g3e.latitude-event-goldens.v1', body: 'Mars',
    window: { start_us: '0', end_us: '10000000' },
    reference: { provider: 'SYNTHETIC NOT A REAL ORACLE', version: 'fixture',
      origin: 'test-only data', method: 'hand-crafted synthetic timestamps',
      source_license: 'synthetic-only', ...conventions },
    events: candidates.map((c, index) => ({ kind: c.kind, at_us: String((index + 1) * 1000000),
      signed_speed_degrees_per_day: c.signed_speed_degrees_per_day })),
  };
  function save() {
    const a = JSON.stringify(audit) + '\n';
    const b = JSON.stringify(goldens) + '\n';
    writeFileSync(auditPath, a);
    writeFileSync(goldensPath, b);
    return { auditSha: hash(a), goldenSha: hash(b) };
  }
  return { audit, goldens, candidates, auditPath, goldensPath, reportPath, save };
}

function run(f, pins, extra = [], overrides = {}) {
  return spawnSync(process.execPath, [reviewer,
    '--audit', f.auditPath,
    '--audit-sha256', overrides.auditSha ?? pins.auditSha,
    '--goldens', f.goldensPath,
    '--goldens-sha256', overrides.goldenSha ?? pins.goldenSha,
    '--time-tolerance-us', overrides.time ?? '100',
    '--speed-tolerance-deg-per-day', overrides.speed ?? '0.1',
    ...extra], { encoding: 'utf8', timeout: 10000 });
}

test('all six synthetic G3E kinds match UTC brackets without certifying coverage', t => {
  const f = fixture(t);
  const result = run(f, f.save(), ['--report', f.reportPath]);
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.matched.length, 6);
  assert.deepEqual(report.matched.map(row => row.kind), kinds);
  assert.ok(report.matched.every(row => row.candidate_midpoint_minus_golden_us === '0'));
  assert.ok(report.matched.every(row => row.golden_within_candidate_bracket === true));
  assert.equal(report.missing_observed_goldens.length, 0);
  assert.equal(report.unmatched_observed_candidates.length, 0);
  assert.equal(report.conclusions.observed_events_within_supplied_tolerances, true);
  for (const key of ['reference_independence_scientifically_established',
    'astronomical_accuracy_verified', 'coverage_certified', 'safe_to_import']) {
    assert.equal(report.conclusions[key], false);
  }
  assert.equal(readFileSync(f.reportPath, 'utf8'), result.stdout);
});

test('peak signed-speed difference reports exit 1 with full review JSON', t => {
  const f = fixture(t);
  f.goldens.events[4].signed_speed_degrees_per_day = 0.3;
  const result = run(f, f.save());
  assert.equal(result.status, 1, result.stderr);
  const report = JSON.parse(result.stdout);
  const northPeak = report.matched.find(row => row.kind === 'NorthwardSpeedMaximum');
  assert.ok(northPeak.candidate_minus_golden_speed_degrees_per_day > 0.19);
  assert.equal(northPeak.speed_within_tolerance, false);
  assert.equal(report.conclusions.safe_to_import, false);
});

test('midpoint tolerance cannot accept a golden instant outside its candidate bracket', t => {
  const f = fixture(t);
  // The candidate spans [999990, 1000010], but the 100-us midpoint
  // tolerance alone would incorrectly admit this reference at 1000050.
  f.goldens.events[0].at_us = '1000050';
  let result = run(f, f.save());
  assert.equal(result.status, 1, result.stderr);
  let report = JSON.parse(result.stdout);
  const crossing = report.matched.find(row => row.kind === 'PlaneAscending');
  assert.equal(crossing.candidate_midpoint_minus_golden_us, '-50');
  assert.equal(crossing.golden_within_candidate_bracket, false);
  assert.equal(report.conclusions.observed_events_within_supplied_tolerances, false);
  assert.equal(report.conclusions.coverage_certified, false);

  // A root exactly at the bracket's last endpoint remains admissible.
  f.goldens.events[0].at_us = '1000010';
  result = run(f, f.save());
  assert.equal(result.status, 0, result.stderr);
  report = JSON.parse(result.stdout);
  assert.equal(report.matched.find(row => row.kind === 'PlaneAscending')
    .golden_within_candidate_bracket, true);
});

test('missing golden and unpaired observed candidate are both retained', t => {
  const f = fixture(t);
  f.audit.events.pairs.splice(1, 1);
  f.audit.events.unmatched_primary.push({ ...f.candidates[1], bracket_start_us: '8900000',
    bracket_end_us: '8900020' });
  const result = run(f, f.save());
  assert.equal(result.status, 1, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.missing_observed_goldens.length, 1);
  assert.equal(report.missing_observed_goldens[0].kind, 'PlaneDescending');
  assert.equal(report.unmatched_observed_candidates.length, 1);
  assert.equal(report.unmatched_observed_candidates[0].kind, 'PlaneDescending');
});

test('two adjacent same-kind golden/candidate events inside ambiguity threshold fail closed', t => {
  const f = fixture(t);
  f.goldens.events.push({ ...f.goldens.events[0], at_us: '1000150' });
  let result = run(f, f.save());
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  f.goldens.events.pop();
  f.audit.events.unmatched_primary.push({ ...f.candidates[0],
    bracket_start_us: '1000140', bracket_end_us: '1000160' });
  result = run(f, f.save());
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
});

test('SHA pin failure, frame mismatch, malformed UTC, and nonpeak speed reject without JSON', t => {
  const f = fixture(t);
  const pins = f.save();
  for (const overrides of [{ auditSha: '0'.repeat(64) }, { goldenSha: '1'.repeat(64) }]) {
    const bad = run(f, pins, [], overrides);
    assert.equal(bad.status, 2);
    assert.equal(bad.stdout, '');
  }
  const modifications = [
    x => { x.goldens.reference.ecliptic_plane = 'mean-of-date'; },
    x => { x.goldens.events[0].at_us = '0001'; },
    x => { x.goldens.events[0].signed_speed_degrees_per_day = 0.2; },
    x => { x.audit.events.pairs[0].primary.bracket_end_us = '0'; },
    x => { x.audit.coverage_certified = true; },
  ];
  for (const modify of modifications) {
    const isolated = fixture(t);
    modify(isolated);
    const bad = run(isolated, isolated.save());
    assert.equal(bad.status, 2, bad.stderr);
    assert.equal(bad.stdout, '');
  }
});

test('empty candidate and golden sets are observational only, not certified completeness', t => {
  const f = fixture(t);
  f.audit.events.pairs = [];
  f.goldens.events = [];
  const result = run(f, f.save());
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.matched.length, 0);
  assert.equal(report.conclusions.coverage_certified, false);
});

test('existing report remains untouched even on successful review', t => {
  const f = fixture(t);
  writeFileSync(f.reportPath, 'user report');
  const result = run(f, f.save(), ['--report', f.reportPath]);
  assert.equal(result.status, 2);
  assert.equal(result.stdout, '');
  assert.equal(readFileSync(f.reportPath, 'utf8'), 'user report');
});
