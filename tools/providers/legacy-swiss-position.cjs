/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
// Local development adapter only. This file does not bundle Swiss Ephemeris,
// proprietary datasets, or private VirgoStars backend routines.
// Run it with the *owner's* authorized backend checkout and license in place.

'use strict';

const path = require('node:path');
const fs = require('node:fs');
const crypto = require('node:crypto');
const readline = require('node:readline');

const sourceModule = process.env.VSEPH_LEGACY_FUNC_MODULE;
const sourceRevision = process.env.VSEPH_LEGACY_COMMIT;
if (!sourceModule || !path.isAbsolute(sourceModule) || !/^[0-9a-f]{40}$/.test(sourceRevision || '')) {
  process.stderr.write('Set VSEPH_LEGACY_FUNC_MODULE (absolute func.js path) and VSEPH_LEGACY_COMMIT (40-character source SHA)\n');
  process.exit(2);
}

// Resolved in the original private backend, so its installed swisseph/luxon
// versions, kernel location and license remain controlled by that checkout.
const { getPianetiCaseByNomiElCelesti, nomeToIDElementoCeleste } = require(sourceModule);
if (typeof getPianetiCaseByNomiElCelesti !== 'function') {
  throw new Error('configured legacy func.js has no getPianetiCaseByNomiElCelesti');
}

const protocol = {
  protocol: 'virgostars.longitude.v2',
  provider: 'legacy-virgostars-swiss-development-only',
  version: sourceRevision,
  frame: 'geocentric-tropical-ecliptic-degrees',
  input_time_scale: 'utc-posix-microseconds',
  flags: 'legacy func.js; SEFLG_SPEED|SEFLG_SWIEPH; UTC rounded down to seconds',
  kernel_digest: process.env.VSEPH_LEGACY_KERNEL_SHA256 || null,
  source_digest: crypto.createHash('sha256').update(fs.readFileSync(sourceModule)).digest('hex'),
};
process.stdout.write(`${JSON.stringify(protocol)}\n`);

const usPerSecond = 1_000_000n;
function utcSecondsText(microseconds) {
  // BigInt makes ancient/future epochs exact even beyond JS Number.MAX_SAFE_INTEGER.
  const seconds = microseconds >= 0n
    ? microseconds / usPerSecond
    : -((-microseconds + usPerSecond - 1n) / usPerSecond);
  const milliseconds = seconds * 1_000n;
  const ms = Number(milliseconds);
  if (!Number.isSafeInteger(ms)) {
    throw new Error('epoch is outside JavaScript Date safe range');
  }
  const iso = new Date(ms).toISOString();
  if (!/^\d{4}-\d{2}-\d{2}T/.test(iso)) {
    throw new Error('legacy Luxon yyyy parser does not support this epoch');
  }
  return iso.slice(0, 19).replace('T', ' ');
}

const lines = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
lines.on('line', (raw) => {
  try {
    const request = JSON.parse(raw);
    if (typeof request.body !== 'string'
      || !Object.prototype.hasOwnProperty.call(nomeToIDElementoCeleste, request.body)
      || typeof request.utc_microseconds !== 'string'
      || !/^-?(0|[1-9]\d*)$/.test(request.utc_microseconds)) {
      throw new Error('invalid body or exact UTC/POSIX microsecond string');
    }
    const dateUtc = utcSecondsText(BigInt(request.utc_microseconds));
    const result = getPianetiCaseByNomiElCelesti(
      dateUtc, 0, 0, [request.body], false, false,
    );
    const data = result?.planets?.[request.body];
    if (!data || !Number.isFinite(data.longitude) || !Number.isFinite(data.longitudeSpeed)) {
      throw new Error('legacy Swiss returned missing or invalid longitude/speed');
    }
    process.stdout.write(`${JSON.stringify({
      body: request.body,
      utc_microseconds: request.utc_microseconds,
      longitude_degrees: data.longitude,
      longitude_speed_degrees_per_day: data.longitudeSpeed,
    })}\n`);
  } catch (error) {
    // Fatal: never output a plausible sample after any failed request.
    process.stderr.write(`legacy Swiss position error: ${error.message}\n`);
    process.exitCode = 2;
    lines.close();
  }
});
