/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

// Development-only review of observed LATITUDE EVENT CANDIDATES. Never an
// astronomical completeness proof, source attestation or approved data import.
const { createHash } = require('node:crypto');
const { createReadStream, readFileSync, statSync, writeFileSync } = require('node:fs');
const path = require('node:path');

const REQUIRED = ['--audit', '--audit-sha256', '--goldens', '--goldens-sha256',
  '--time-tolerance-us', '--speed-tolerance-deg-per-day'];
const VALID = new Set([...REQUIRED, '--report']);
const CONVENTIONS = ['frame', 'ecliptic_plane', 'position_corrections',
  'dynamical_time_scale', 'input_time_scale', 'speed_definition'];
const KINDS = new Set(['PlaneAscending', 'PlaneDescending',
  'StationNorthward', 'StationSouthward',
  'NorthwardSpeedMaximum', 'SouthwardSpeedMagnitudeMaximum']);
const PEAKS = new Set(['NorthwardSpeedMaximum', 'SouthwardSpeedMagnitudeMaximum']);
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;

function fail(message) { throw new Error(message); }
function exactFields(value, fields, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).length !== fields.length ||
      fields.some(field => !Object.hasOwn(value, field))) fail(`invalid ${label} fields`);
}
function integer(value, label) {
  if (typeof value !== 'string' || !/^(?:0|-?[1-9][0-9]*)$/.test(value)) {
    fail(`${label} must be canonical decimal-string integer`);
  }
  const parsed = BigInt(value);
  if (parsed < I64_MIN || parsed > I64_MAX) fail(`${label} outside i64 bounds`);
  return parsed;
}
function nonnegativeFinite(value, label) {
  if (typeof value !== 'string' || !/^(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$|^0\.\d+(?:[eE][+-]?\d+)?$/.test(value) ||
      !Number.isFinite(Number(value)) || Number(value) < 0) fail(`${label} must be nonnegative finite decimal`);
  return Number(value);
}
function parseArgs(argv) {
  if (argv.length % 2 !== 0) fail('every option requires one value');
  const m = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    if (!VALID.has(argv[index]) || m.has(argv[index]) || !argv[index + 1]) {
      fail(`unknown, empty or duplicated option ${argv[index]}`);
    }
    m.set(argv[index], argv[index + 1]);
  }
  for (const name of REQUIRED) if (!m.has(name)) fail(`missing ${name}`);
  for (const name of ['--audit-sha256', '--goldens-sha256']) {
    if (!/^[0-9a-f]{64}$/.test(m.get(name))) fail(`${name} must be independently pinned lowercase SHA-256`);
  }
  const tolerance = integer(m.get('--time-tolerance-us'), 'time tolerance');
  if (tolerance < 0n) fail('time tolerance must be nonnegative');
  return {
    audit: path.resolve(m.get('--audit')),
    auditSha: m.get('--audit-sha256'),
    goldens: path.resolve(m.get('--goldens')),
    goldensSha: m.get('--goldens-sha256'),
    timeToleranceUs: tolerance,
    speedTolerance: nonnegativeFinite(m.get('--speed-tolerance-deg-per-day'), 'speed tolerance'),
    report: m.has('--report') ? path.resolve(m.get('--report')) : null,
  };
}
async function sha256(file) {
  const digest = createHash('sha256');
  for await (const chunk of createReadStream(file)) digest.update(chunk);
  return digest.digest('hex');
}
async function pinnedJson(file, trusted, maxBytes, label) {
  if (statSync(file).size > maxBytes) fail(`${label} exceeds ${maxBytes} bytes`);
  if (await sha256(file) !== trusted) fail(`${label} SHA-256 differs from pinned trust anchor`);
  try { return JSON.parse(readFileSync(file, 'utf8')); }
  catch { fail(`invalid ${label} JSON`); }
}
function isPeak(kind) { return PEAKS.has(kind); }
function checkedSpeed(kind, speed, label) {
  if (isPeak(kind)) {
    if (typeof speed !== 'number' || !Number.isFinite(speed) ||
        (kind === 'NorthwardSpeedMaximum' ? speed <= 0 : speed >= 0)) {
      fail(`invalid signed peak speed in ${label}`);
    }
  } else if (speed !== null) fail(`unexpected speed for plane crossing/station in ${label}`);
}

function readAudit(audit) {
  if (!audit || audit.kind !== 'experimental_latitude_provider_comparison' ||
      !['events', 'both'].includes(audit.mode) ||
      !audit.events || typeof audit.body !== 'string' ||
      audit.provider_independence_verified !== false ||
      audit.astronomical_accuracy_verified !== false ||
      audit.coverage_certified !== false || audit.safe_to_import !== false) {
    fail('not an explicitly uncertified G3E events audit');
  }
  const window = audit.window;
  exactFields(window, ['start_us', 'end_us', 'step_us', 'precision_us'], 'audit window');
  const start = integer(window.start_us, 'audit start');
  const end = integer(window.end_us, 'audit end');
  if (end <= start || integer(window.step_us, 'step') <= 0n ||
      integer(window.precision_us, 'precision') <= 0n ||
      integer(window.precision_us, 'precision') > integer(window.step_us, 'step')) {
    fail('invalid audit half-open UTC window');
  }
  if (!audit.primary_provider || ['provider', 'version', 'source_license', ...CONVENTIONS].some(key =>
    typeof audit.primary_provider[key] !== 'string' || !audit.primary_provider[key].trim())) {
    fail('incomplete primary provider identity');
  }
  const report = audit.events;
  if (!Array.isArray(report.pairs) || !Array.isArray(report.unmatched_primary) ||
      !Array.isArray(report.unmatched_reference) ||
      report.pairs.length + report.unmatched_primary.length > 10000) {
    fail('invalid or excessive event candidates');
  }
  const observed = [...report.pairs.map(pair => {
    if (!pair || typeof pair !== 'object' || !pair.primary || !pair.reference ||
        pair.primary.kind !== pair.reference.kind ||
        pair.primary.body !== pair.reference.body) fail('malformed paired source event');
    return pair.primary;
  }), ...report.unmatched_primary];
  return { observed, start, end, body: audit.body, provider: audit.primary_provider };
}

function readGoldens(goldens, audit) {
  exactFields(goldens, ['schema', 'body', 'window', 'reference', 'events'], 'event goldens');
  if (goldens.schema !== 'vseph.g3e.latitude-event-goldens.v1' || goldens.body !== audit.body) {
    fail('golden schema/body does not match audit');
  }
  exactFields(goldens.window, ['start_us', 'end_us'], 'golden window');
  if (integer(goldens.window.start_us, 'golden start') !== audit.start ||
      integer(goldens.window.end_us, 'golden end') !== audit.end) fail('golden UTC window differs from audit');
  const fields = ['provider', 'version', 'origin', 'method', 'source_license', ...CONVENTIONS];
  exactFields(goldens.reference, fields, 'reference provenance');
  if (fields.some(key => typeof goldens.reference[key] !== 'string' || !goldens.reference[key].trim())) {
    fail('incomplete reference provenance');
  }
  for (const key of CONVENTIONS) {
    if (goldens.reference[key] !== audit.provider[key]) fail(`reference ${key} incompatible with audit`);
  }
  if (!Array.isArray(goldens.events) || goldens.events.length > 10000) fail('invalid golden event count');
  return goldens.events;
}
function validateObserved(candidates, audit) {
  const byKind = new Map();
  for (const [index, c] of candidates.entries()) {
    exactFields(c, ['body', 'kind', 'bracket_start_us', 'bracket_end_us',
      'signed_speed_degrees_per_day'], `candidate ${index}`);
    if (c.body !== audit.body || !KINDS.has(c.kind)) fail(`wrong candidate body/kind at ${index}`);
    const first = integer(c.bracket_start_us, 'candidate bracket start');
    const last = integer(c.bracket_end_us, 'candidate bracket end');
    if (last <= first || first < audit.start || last > audit.end) fail('candidate bracket outside audit window');
    checkedSpeed(c.kind, c.signed_speed_degrees_per_day, `candidate ${index}`);
    const item = { kind: c.kind, midpoint: first + (last - first) / 2n,
      bracket_start_us: c.bracket_start_us, bracket_end_us: c.bracket_end_us,
      signed_speed_degrees_per_day: c.signed_speed_degrees_per_day };
    if (!byKind.has(c.kind)) byKind.set(c.kind, []);
    byKind.get(c.kind).push(item);
  }
  return byKind;
}
function validateGoldenEvents(events, audit) {
  const byKind = new Map();
  for (const [index, e] of events.entries()) {
    exactFields(e, ['kind', 'at_us', 'signed_speed_degrees_per_day'], `golden event ${index}`);
    if (!KINDS.has(e.kind)) fail(`invalid golden kind at ${index}`);
    const at = integer(e.at_us, 'golden event UTC');
    if (at < audit.start || at >= audit.end) fail('golden event is outside audit window');
    checkedSpeed(e.kind, e.signed_speed_degrees_per_day, `golden event ${index}`);
    if (!byKind.has(e.kind)) byKind.set(e.kind, []);
    byKind.get(e.kind).push({ ...e, at });
  }
  return byKind;
}

function checkSeparation(series, position, tol, label) {
  series.sort((a, b) => position(a) < position(b) ? -1 : position(a) > position(b) ? 1 : 0);
  for (let i = 1; i < series.length; i++) {
    if (position(series[i]) - position(series[i - 1]) <= 2n * tol) {
      fail(`ambiguous or duplicate ${label} events within twice the tolerance`);
    }
  }
}
function reconcile(primary, goldens, opts) {
  const matches = [];
  const missedGolden = [];
  const extraObserved = [];
  for (const kind of KINDS) {
    const observed = primary.get(kind) || [];
    const expected = goldens.get(kind) || [];
    checkSeparation(observed, e => e.midpoint, opts.timeToleranceUs, `observed ${kind}`);
    checkSeparation(expected, e => e.at, opts.timeToleranceUs, `golden ${kind}`);
    let j = 0;
    for (const target of expected) {
      while (j < observed.length && observed[j].midpoint < target.at - opts.timeToleranceUs) {
        extraObserved.push({ kind, bracket_start_us: observed[j].bracket_start_us,
          bracket_end_us: observed[j].bracket_end_us });
        j++;
      }
      if (j >= observed.length || observed[j].midpoint > target.at + opts.timeToleranceUs) {
        missedGolden.push({ kind, at_us: target.at_us });
        continue;
      }
      const actual = observed[j++];
      const speedDelta = isPeak(kind)
        ? actual.signed_speed_degrees_per_day - target.signed_speed_degrees_per_day : null;
      if (speedDelta !== null && !Number.isFinite(speedDelta)) fail('nonfinite signed peak-speed residual');
      matches.push({ kind, golden_at_us: target.at_us,
        candidate_bracket_start_us: actual.bracket_start_us,
        candidate_bracket_end_us: actual.bracket_end_us,
        candidate_midpoint_minus_golden_us: (actual.midpoint - target.at).toString(),
        golden_within_candidate_bracket:
          target.at >= BigInt(actual.bracket_start_us) &&
          target.at <= BigInt(actual.bracket_end_us),
        candidate_signed_speed_degrees_per_day: actual.signed_speed_degrees_per_day,
        golden_signed_speed_degrees_per_day: target.signed_speed_degrees_per_day,
        candidate_minus_golden_speed_degrees_per_day: speedDelta,
        speed_within_tolerance: speedDelta === null ? null : Math.abs(speedDelta) <= opts.speedTolerance });
    }
    for (; j < observed.length; j++) {
      extraObserved.push({ kind, bracket_start_us: observed[j].bracket_start_us,
        bracket_end_us: observed[j].bracket_end_us });
    }
  }
  return { matches, missedGolden, extraObserved };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const rawAudit = await pinnedJson(opts.audit, opts.auditSha, 2 * 1024 * 1024, 'latitude audit');
  const audit = readAudit(rawAudit);
  const golden = await pinnedJson(opts.goldens, opts.goldensSha, 512 * 1024, 'event goldens');
  const goldenEvents = readGoldens(golden, audit);
  const observed = validateObserved(audit.observed, audit);
  const expected = validateGoldenEvents(goldenEvents, audit);
  const result = reconcile(observed, expected, opts);
  const disagreement = result.missedGolden.length > 0 || result.extraObserved.length > 0 ||
    result.matches.some(match => match.speed_within_tolerance === false ||
      match.golden_within_candidate_bracket === false);
  const report = {
    schema: 'vseph.g3e.latitude-event-golden-review.v1',
    audit_sha256: opts.auditSha,
    golden_sha256: opts.goldensSha,
    body: audit.body,
    window: { start_us: audit.start.toString(), end_us: audit.end.toString() },
    reference: golden.reference,
    primary_provider: audit.provider,
    time_tolerance_us: opts.timeToleranceUs.toString(),
    signed_peak_speed_tolerance_degrees_per_day: opts.speedTolerance,
    matched: result.matches,
    missing_observed_goldens: result.missedGolden,
    unmatched_observed_candidates: result.extraObserved,
    conclusions: {
      observed_events_within_supplied_tolerances: !disagreement,
      candidate_times_are_bracket_midpoints_not_refined_exact_roots: true,
      reference_independence_scientifically_established: false,
      astronomical_accuracy_verified: false,
      coverage_certified: false,
      safe_to_import: false,
    },
    warning: 'Observed candidates only. Golden instants outside candidate brackets are discrepancies even within midpoint tolerance. Matching or empty lists never establish continuous-time coverage, exact event roots or reference independence.',
  };
  const encoded = JSON.stringify(report, null, 2) + '\n';
  if (opts.report) writeFileSync(opts.report, encoded, { encoding: 'utf8', flag: 'wx' });
  process.stdout.write(encoded);
  if (disagreement) process.exitCode = 1;
}

main().catch(error => {
  process.stderr.write(`G3E latitude event golden review: ${error.message}\n`);
  process.exitCode = 2;
});
