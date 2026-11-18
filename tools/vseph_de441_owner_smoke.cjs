/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

// Owner-run, development-only transport smoke. NEVER a scientific gold standard.
// Run from Windows with Node; an existing DE441 and separately built binding are required.
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const { createReadStream, writeFileSync } = require('node:fs');
const path = require('node:path');

const BRIDGE = path.join(__dirname, 'vseph_virgostars_latitude_provider.cjs');
const REQUIRED = ['--binding', '--kernel', '--kernel-sha256', '--source-commit'];
const OPTIONS = [...REQUIRED, '--report'];
const PROBES = Object.freeze([
  { body: 'Mars', utc_microseconds: String(Date.parse('1960-01-01T00:00:00Z') * 1000) },
  { body: 'Moon', utc_microseconds: String(Date.parse('2000-01-01T00:00:00Z') * 1000) },
  { body: 'Jupiter', utc_microseconds: String(Date.parse('2026-11-18T00:00:00Z') * 1000) },
]);

function fail(message) { throw new Error(message); }

function argumentsFrom(argv) {
  if (argv.length % 2 !== 0) fail('options require a value');
  const values = new Map();
  for (let i = 0; i < argv.length; i += 2) {
    const option = argv[i];
    if (!OPTIONS.includes(option) || values.has(option) || !argv[i + 1]) {
      fail(`invalid or repeated option ${option}`);
    }
    values.set(option, argv[i + 1]);
  }
  for (const option of REQUIRED) if (!values.has(option)) fail(`missing ${option}`);
  const kernelSha = values.get('--kernel-sha256');
  const commit = values.get('--source-commit');
  if (!/^[0-9a-f]{64}$/.test(kernelSha)) fail('--kernel-sha256 must be a trusted lowercase hex digest');
  if (!/^[0-9a-f]{40}$/.test(commit)) fail('--source-commit must be a pinned lowercase 40-character Git revision');
  return {
    binding: path.resolve(values.get('--binding')),
    kernel: path.resolve(values.get('--kernel')),
    kernelSha,
    commit,
    report: values.has('--report') ? path.resolve(values.get('--report')) : null,
  };
}

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function expectedBindingDigest(binding) {
  const loader = await sha256(binding);
  const addon = await sha256(path.join(path.dirname(binding), 'native', 'virgostars_ephemeris.node'));
  return createHash('sha256')
    .update('virgostars-node-binding-v1\0').update(loader).update('\0').update(addon).digest('hex');
}

async function main() {
  const options = argumentsFrom(process.argv.slice(2));
  // This independent local read detects identity mistakes but cannot attest a build/commit.
  const measuredBinding = await expectedBindingDigest(options.binding);
  const result = spawnSync(process.execPath, [BRIDGE, options.binding, options.kernel,
    options.kernelSha, options.commit], {
    input: PROBES.map(probe => JSON.stringify(probe)).join('\n') + '\n',
    encoding: 'utf8', timeout: 240000, maxBuffer: 1024 * 1024, windowsHide: true,
  });
  if (result.error) fail(`provider launch failed: ${result.error.message}`);
  if (result.status !== 0) fail(`provider refused smoke (${result.status}): ${(result.stderr || '').trim()}`);
  if (result.stderr && result.stderr.trim()) fail('provider emitted unexpected stderr output');
  const lines = result.stdout.trim().split('\n');
  if (lines.length !== PROBES.length + 1) fail('unexpected provider handshake/sample line count');
  const decoded = lines.map(line => {
    try { return JSON.parse(line); } catch { fail('malformed provider JSON output'); }
  });
  const [identity, ...samples] = decoded;
  if (!identity || identity.protocol !== 'virgostars.latitude.v1' ||
      identity.kernel_digest !== options.kernelSha ||
      identity.source_digest !== measuredBinding ||
      identity.version !== options.commit) {
    fail('provider identity does not match pinned kernel and measured binding');
  }
  samples.forEach((sample, index) => {
    const probe = PROBES[index];
    assert.deepEqual(Object.keys(sample).sort(), [
      'body', 'latitude_degrees', 'latitude_speed_degrees_per_day', 'utc_microseconds',
    ]);
    if (sample.body !== probe.body || sample.utc_microseconds !== probe.utc_microseconds ||
        typeof sample.latitude_degrees !== 'number' ||
        !Number.isFinite(sample.latitude_degrees) || Math.abs(sample.latitude_degrees) > 90 ||
        typeof sample.latitude_speed_degrees_per_day !== 'number' ||
        !Number.isFinite(sample.latitude_speed_degrees_per_day)) {
      fail(`invalid or mismatched provider response for probe ${index}`);
    }
  });

  const report = {
    schema: 'vseph.g3e.owner-smoke.v1',
    identity,
    executed_utc_probes: samples,
    result: {
      protocol_smoke_passed: true,
      native_binding_bytes_measured: true,
      native_binary_source_commit_attested: false,
      independent_provider_compared: false,
      astronomical_accuracy_verified: false,
      coverage_certified: false,
      safe_to_import: false,
    },
    warning: 'Experimental local transport test only. Pin trust anchors independently; obtain independent reference/golden events before any scientific approval.',
  };
  const json = JSON.stringify(report, null, 2) + '\n';
  if (options.report) writeFileSync(options.report, json, { encoding: 'utf8', flag: 'wx' });
  process.stdout.write(json);
}

main().catch(error => {
  process.stderr.write(`G3E owner smoke: ${error.message}\n`);
  process.exitCode = 2;
});
