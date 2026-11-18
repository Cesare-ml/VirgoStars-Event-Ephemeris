/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

// Development-only, owner-supplied coordinate oracle. Numerical comparison is
// useful evidence, NOT proof that the reference is independent or correct.
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { createReadStream, writeFileSync } = require('node:fs');
const path = require('node:path');
const { readPinnedJson } = require('./pinned_json.cjs');

const BRIDGE = path.resolve(__dirname, 'vseph_virgostars_latitude_provider.cjs');
const REQUIRED = ['--binding', '--kernel', '--kernel-sha256', '--source-commit',
  '--goldens', '--goldens-sha256', '--latitude-tolerance-deg', '--speed-tolerance-deg-per-day'];
const VALID = new Set([...REQUIRED, '--report']);
const COMPARABLE = ['frame', 'ecliptic_plane', 'position_corrections', 'dynamical_time_scale',
  'input_time_scale', 'speed_definition'];
const SAMPLE_KEYS = ['body', 'latitude_degrees', 'latitude_speed_degrees_per_day', 'utc_microseconds'];
const BODIES = new Set(['Sun', 'Moon', 'Mercury', 'Venus', 'Mars', 'Jupiter',
  'Saturn', 'Uranus', 'Neptune', 'Pluto']);
const MIN_UTC_US = BigInt(Date.parse('1900-01-01T00:00:00Z')) * 1000n;
const MAX_UTC_US = BigInt(Date.parse('2027-06-28T00:00:00Z')) * 1000n;

function fail(message) { throw new Error(message); }

function parseArgs(argv) {
  if (argv.length % 2 !== 0) fail('each option requires one value');
  const args = new Map();
  for (let i = 0; i < argv.length; i += 2) {
    if (!VALID.has(argv[i]) || args.has(argv[i]) || !argv[i + 1]) {
      fail(`unknown, empty or duplicate argument: ${argv[i]}`);
    }
    args.set(argv[i], argv[i + 1]);
  }
  for (const option of REQUIRED) if (!args.has(option)) fail(`missing ${option}`);
  for (const name of ['--kernel-sha256', '--goldens-sha256']) {
    if (!/^[0-9a-f]{64}$/.test(args.get(name))) fail(`${name} must be a pinned lowercase SHA-256`);
  }
  if (!/^[0-9a-f]{40}$/.test(args.get('--source-commit'))) fail('expected pinned lowercase 40-char Git revision');
  for (const name of ['--latitude-tolerance-deg', '--speed-tolerance-deg-per-day']) {
    const value = args.get(name);
    if (!/^(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$|^0\.\d+(?:[eE][+-]?\d+)?$/.test(value) ||
        !Number.isFinite(Number(value)) || Number(value) < 0) {
      fail(`${name} must be a finite nonnegative decimal`);
    }
  }
  return {
    binding: path.resolve(args.get('--binding')),
    kernel: path.resolve(args.get('--kernel')),
    kernelSha: args.get('--kernel-sha256'),
    commit: args.get('--source-commit'),
    goldens: path.resolve(args.get('--goldens')),
    goldensSha: args.get('--goldens-sha256'),
    latitudeTolerance: Number(args.get('--latitude-tolerance-deg')),
    speedTolerance: Number(args.get('--speed-tolerance-deg-per-day')),
    report: args.has('--report') ? path.resolve(args.get('--report')) : null,
  };
}

async function hashFile(file) {
  const hash = createHash('sha256');
  for await (const part of createReadStream(file)) hash.update(part);
  return hash.digest('hex');
}

function strictKeys(value, fields, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).length !== fields.length ||
      fields.some(field => !Object.hasOwn(value, field))) fail(`invalid ${label} fields`);
}

function validateGolden(golden) {
  strictKeys(golden, ['schema', 'reference', 'samples'], 'golden root');
  if (golden.schema !== 'vseph.g3e.latitude-goldens.v1') fail('unknown golden schema');
  const fields = ['provider', 'version', 'origin', 'method', 'source_license', ...COMPARABLE];
  strictKeys(golden.reference, fields, 'golden reference');
  if (fields.some(field => typeof golden.reference[field] !== 'string' ||
      !golden.reference[field].trim())) fail('golden reference is incomplete');
  if (!Array.isArray(golden.samples) || golden.samples.length < 1 || golden.samples.length > 256) {
    fail('golden sample count must be 1..256');
  }
  const seen = new Set();
  for (const [index, sample] of golden.samples.entries()) {
    strictKeys(sample, SAMPLE_KEYS, `golden sample ${index}`);
    const time = sample.utc_microseconds;
    if (!BODIES.has(sample.body) || typeof time !== 'string' ||
        !/^(?:0|-?[1-9]\d*)$/.test(time) ||
        BigInt(time) < MIN_UTC_US || BigInt(time) >= MAX_UTC_US ||
        !Number.isSafeInteger(Number(time))) {
      fail(`invalid body/UTC at golden sample ${index}`);
    }
    if (typeof sample.latitude_degrees !== 'number' ||
        !Number.isFinite(sample.latitude_degrees) || Math.abs(sample.latitude_degrees) > 90 ||
        typeof sample.latitude_speed_degrees_per_day !== 'number' ||
        !Number.isFinite(sample.latitude_speed_degrees_per_day)) {
      fail(`invalid finite latitude/speed at golden sample ${index}`);
    }
    const id = `${sample.body}:${time}`;
    if (seen.has(id)) fail(`duplicate golden body/UTC ${id}`);
    seen.add(id);
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const golden = readPinnedJson(options.goldens, options.goldensSha, 256 * 1024, 'goldens');
  validateGolden(golden);
  const loaderSha = await hashFile(options.binding);
  const addonSha = await hashFile(path.join(path.dirname(options.binding), 'native', 'virgostars_ephemeris.node'));
  const bindingDigest = createHash('sha256').update('virgostars-node-binding-v1\0')
    .update(loaderSha).update('\0').update(addonSha).digest('hex');

  const child = spawnSync(process.execPath, [BRIDGE, options.binding, options.kernel,
    options.kernelSha, options.commit], {
    input: golden.samples.map(({ body, utc_microseconds }) => JSON.stringify({ body, utc_microseconds })).join('\n') + '\n',
    encoding: 'utf8', windowsHide: true, timeout: 240000, maxBuffer: 1024 * 1024,
  });
  if (child.error) fail(`provider launch: ${child.error.message}`);
  if (child.status !== 0) fail(`provider refused samples: ${(child.stderr || '').trim()}`);
  if (child.stderr.trim()) fail('provider emitted unexpected stderr');
  const lines = child.stdout.trim().split('\n');
  if (lines.length !== golden.samples.length + 1) fail('wrong number of provider replies');
  let values;
  try { values = lines.map(line => JSON.parse(line)); }
  catch { fail('malformed provider JSON'); }
  const [identity, ...samples] = values;
  if (!identity || identity.protocol !== 'virgostars.latitude.v1' ||
      identity.source_digest !== bindingDigest || identity.kernel_digest !== options.kernelSha ||
      identity.version !== options.commit) fail('provider identity/pins changed');
  for (const field of COMPARABLE) {
    if (identity[field] !== golden.reference[field]) fail(`reference ${field} differs from provider convention`);
  }

  const discrepancies = [];
  let largestLatitude = { absolute_degrees: 0, body: null, utc_microseconds: null };
  let largestSpeed = { absolute_degrees_per_day: 0, body: null, utc_microseconds: null };
  for (let index = 0; index < samples.length; index++) {
    const actual = samples[index];
    const expected = golden.samples[index];
    strictKeys(actual, SAMPLE_KEYS, `native response ${index}`);
    if (actual.body !== expected.body || actual.utc_microseconds !== expected.utc_microseconds ||
        typeof actual.latitude_degrees !== 'number' || !Number.isFinite(actual.latitude_degrees) ||
        Math.abs(actual.latitude_degrees) > 90 ||
        typeof actual.latitude_speed_degrees_per_day !== 'number' ||
        !Number.isFinite(actual.latitude_speed_degrees_per_day)) fail(`invalid native response ${index}`);
    const latitudeDelta = actual.latitude_degrees - expected.latitude_degrees;
    const speedDelta = actual.latitude_speed_degrees_per_day - expected.latitude_speed_degrees_per_day;
    if (!Number.isFinite(latitudeDelta) || !Number.isFinite(speedDelta)) fail('non-finite residual');
    if (Math.abs(latitudeDelta) > largestLatitude.absolute_degrees) {
      largestLatitude = { absolute_degrees: Math.abs(latitudeDelta), body: expected.body,
        utc_microseconds: expected.utc_microseconds };
    }
    if (Math.abs(speedDelta) > largestSpeed.absolute_degrees_per_day) {
      largestSpeed = { absolute_degrees_per_day: Math.abs(speedDelta), body: expected.body,
        utc_microseconds: expected.utc_microseconds };
    }
    if (Math.abs(latitudeDelta) > options.latitudeTolerance || Math.abs(speedDelta) > options.speedTolerance) {
      discrepancies.push({ body: expected.body, utc_microseconds: expected.utc_microseconds,
        golden_latitude_degrees: expected.latitude_degrees,
        actual_latitude_degrees: actual.latitude_degrees,
        actual_minus_golden_latitude_degrees: latitudeDelta,
        golden_speed_degrees_per_day: expected.latitude_speed_degrees_per_day,
        actual_speed_degrees_per_day: actual.latitude_speed_degrees_per_day,
        actual_minus_golden_speed_degrees_per_day: speedDelta });
    }
  }
  const result = {
    schema: 'vseph.g3e.latitude-golden-review.v1',
    source_identity: identity,
    golden_sha256: options.goldensSha,
    reference: golden.reference,
    tolerances: { latitude_degrees: options.latitudeTolerance,
      signed_speed_degrees_per_day: options.speedTolerance },
    compared_samples: samples.length,
    largest_absolute_latitude_residual: largestLatitude,
    largest_absolute_signed_speed_residual: largestSpeed,
    discrepancies,
    conclusions: { observations_within_tolerance: discrepancies.length === 0,
      reference_independence_scientifically_established: false,
      astronomical_accuracy_verified: false, coverage_certified: false, safe_to_import: false },
    warning: 'Numerical differences only. Reference provenance, timing, ecliptic conventions and scientific independence require external expert review.',
  };
  const output = JSON.stringify(result, null, 2) + '\n';
  if (options.report) writeFileSync(options.report, output, { encoding: 'utf8', flag: 'wx' });
  process.stdout.write(output);
  if (discrepancies.length) process.exitCode = 1;
}

main().catch(error => {
  process.stderr.write(`G3E golden review: ${error.message}\n`);
  process.exitCode = 2;
});
