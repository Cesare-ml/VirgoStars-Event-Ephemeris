/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

// Development-only source bridge. Astronomical mathematics remains exclusively
// in the independently built VirgoStars-Ephemeris native Node binding.
// Usage: node this-file <binding/index.js> <classic-de441.eph> <trusted-sha256> <pinned-commit>
const { createHash } = require('node:crypto');
const { createReadStream } = require('node:fs');
const path = require('node:path');
const readline = require('node:readline');

const MIN_UTC_US = BigInt(Date.parse('1900-01-01T00:00:00Z')) * 1000n;
const MAX_UTC_US = BigInt(Date.parse('2027-06-28T00:00:00Z')) * 1000n;
const IERS_START_US = BigInt(Date.parse('1972-01-01T00:00:00Z')) * 1000n;
const BODY_IDS = Object.freeze({
  Sun: 0, Moon: 1, Mercury: 2, Venus: 3, Mars: 4,
  Jupiter: 5, Saturn: 6, Uranus: 7, Neptune: 8, Pluto: 9,
});
const SUPPORTED_FLAGS = 1 | 256; // SEFLG_JPLEPH | SEFLG_SPEED

function fail(message) {
  throw new Error(message);
}

async function hashFile(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function bindingDigest(bindingPath) {
  // Fingerprint the loader and the native addon that this binding loads.
  // This is a measurement of local files, NOT an attestation of their origin,
  // their build flags or any dependencies loaded by the addon.
  const loaderSha = await hashFile(bindingPath);
  const nativeSha = await hashFile(path.join(path.dirname(bindingPath), 'native', 'virgostars_ephemeris.node'));
  return createHash('sha256')
    .update('virgostars-node-binding-v1\0', 'utf8')
    .update(loaderSha, 'ascii')
    .update('\0', 'utf8')
    .update(nativeSha, 'ascii')
    .digest('hex');
}

function sample(engine, request) {
  if (!request || typeof request !== 'object' || Array.isArray(request) ||
      Object.keys(request).length !== 2 ||
      !Object.hasOwn(request, 'body') || !Object.hasOwn(request, 'utc_microseconds')) {
    fail('expected exactly body and utc_microseconds');
  }
  const { body, utc_microseconds: exactUtc } = request;
  if (typeof body !== 'string' || !Object.hasOwn(BODY_IDS, body)) {
    fail('unsupported celestial body (Sun through Pluto only)');
  }
  if (typeof exactUtc !== 'string' || !/^(?:0|-?[1-9][0-9]*)$/.test(exactUtc)) {
    fail('UTC microseconds must be a canonical decimal string');
  }
  const utc = BigInt(exactUtc);
  if (utc < MIN_UTC_US || utc >= MAX_UTC_US) {
    fail('UTC outside declared civil time policy 1900-01-01..2027-06-28');
  }
  // Within the supported civil interval Number(utc) is an exact integer.
  // The single-f64 Julian date still loses sub-millisecond precision; see docs.
  const jdUt = 2440587.5 + Number(utc) / 86_400_000_000;
  const result = engine.calcUt(jdUt, BODY_IDS[body], SUPPORTED_FLAGS);
  if (!result || !Array.isArray(result.xx) || result.xx.length !== 6 ||
      result.xx.some(x => typeof x !== 'number' || !Number.isFinite(x)) ||
      !Number.isInteger(result.rflag) ||
      (result.rflag & SUPPORTED_FLAGS) !== SUPPORTED_FLAGS ||
      (result.rflag & (8 | 2048)) !== 0 ||
      typeof result.timeSource !== 'string') {
    fail('invalid or incompatible native apparent six-field response');
  }
  const expectedTime = utc < IERS_START_US ? 'DELTA_T_POLYNOMIAL:' : 'IERS_LEAP_SECONDS';
  if (expectedTime === 'IERS_LEAP_SECONDS'
    ? result.timeSource !== expectedTime
    : !result.timeSource.startsWith(expectedTime)) {
    fail('native time policy differs from the declared UTC conversion');
  }
  if (Math.abs(result.xx[1]) > 90) fail('native latitude outside ±90 degrees');
  return {
    body,
    utc_microseconds: exactUtc,
    latitude_degrees: result.xx[1],
    latitude_speed_degrees_per_day: result.xx[4],
  };
}

async function main() {
  if (process.argv.length !== 6) {
    fail('usage: node <bridge.cjs> <ephemeris/bindings/node/index.js> <de441.eph> <trusted-kernel-sha256> <pinned-source-commit>');
  }
  const [, , bindingArg, kernelArg, expectedKernelSha, sourceCommit] = process.argv;
  if (!/^[0-9a-f]{64}$/.test(expectedKernelSha)) fail('expected lowercase SHA-256 of kernel');
  if (!/^[0-9a-f]{40}$/.test(sourceCommit)) fail('expected pinned 40-char Git commit');
  const kernelPath = path.resolve(kernelArg);
  const actualSha = await hashFile(kernelPath);
  if (actualSha !== expectedKernelSha) fail('DE441 kernel SHA-256 does not match trust anchor');
  const bindingPath = path.resolve(bindingArg);
  const sourceDigest = await bindingDigest(bindingPath);
  const binding = require(bindingPath);
  if (typeof binding.Engine !== 'function') fail('native VirgoStars-Ephemeris Engine missing');
  const engine = new binding.Engine(kernelPath);

  const identity = {
    protocol: 'virgostars.latitude.v1',
    provider: 'VirgoStars-Ephemeris (operator-supplied native Node binding)',
    version: sourceCommit,
    frame: 'geocentric-ecliptic-latitude-degrees',
    ecliptic_plane: 'true-ecliptic-and-equinox-of-date (IAU-2006/2000B)',
    position_corrections: 'one-way-light-time;solar-deflection;annual-aberration;date-frame-rotation',
    dynamical_time_scale: 'TDB (approximate from civil UTC; signed rate per TDB day)',
    input_time_scale: 'utc-posix-microseconds',
    speed_definition: 'signed-d-beta-dt-degrees-per-day',
    flags: 'SEFLG_JPLEPH|SEFLG_SPEED;UTC~UT1;native-civil-policy-1900-to-2027',
    source_license: 'CPAL-1.0 code; kernel redistribution rights NOT asserted',
    kernel_digest: actualSha,
    source_digest: sourceDigest, // Measured loader+addon bytes, NOT a source-commit attestation.
  };
  process.stdout.write(JSON.stringify(identity) + '\n');
  const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of input) {
    if (Buffer.byteLength(line, 'utf8') > 8192) fail('input line exceeds 8192 bytes');
    let parsed;
    try { parsed = JSON.parse(line); } catch { fail('malformed sample JSON'); }
    process.stdout.write(JSON.stringify(sample(engine, parsed)) + '\n');
  }
}

main().catch(error => {
  process.stderr.write(`VirgoStars latitude bridge: ${error.message}\n`);
  process.exitCode = 2;
});
