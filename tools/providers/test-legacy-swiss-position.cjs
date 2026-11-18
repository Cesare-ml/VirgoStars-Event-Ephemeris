/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 */
'use strict';

const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const path = require('node:path');
const test = require('node:test');

test('legacy adapter preserves high precision signed i64 timestamps as decimal strings', async () => {
  const proc = spawn(process.execPath, [path.join(__dirname, 'legacy-swiss-position.cjs')], {
    env: {
      ...process.env,
      VSEPH_LEGACY_FUNC_MODULE: path.join(__dirname, 'fixtures/mock-func.cjs'),
      VSEPH_LEGACY_COMMIT: 'a'.repeat(40),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const input = [
    { body: 'Mars', utc_microseconds: '32000000000000001' },
    { body: 'Moon', utc_microseconds: '-32000000000000001' },
  ];
  for (const value of input) {
    proc.stdin.write(`${JSON.stringify(value)}\n`);
  }
  proc.stdin.end();
  const chunks = [];
  const errors = [];
  proc.stdout.on('data', (data) => chunks.push(data));
  proc.stderr.on('data', (data) => errors.push(data));
  const exit = await new Promise((resolve) => proc.on('close', resolve));
  assert.equal(exit, 0, Buffer.concat(errors).toString());
  const rows = Buffer.concat(chunks).toString().trim().split('\n').map(JSON.parse);
  assert.equal(rows.length, 3);
  assert.equal(rows[0].protocol, 'virgostars.longitude.v2');
  assert.equal(rows[1].utc_microseconds, input[0].utc_microseconds);
  assert.equal(rows[2].utc_microseconds, input[1].utc_microseconds);
  assert.equal(rows[1].longitude_speed_degrees_per_day, 2);
});

test('adapter rejects malformed body without returning a fake astronomical sample', async () => {
  const proc = spawn(process.execPath, [path.join(__dirname, 'legacy-swiss-position.cjs')], {
    env: {
      ...process.env,
      VSEPH_LEGACY_FUNC_MODULE: path.join(__dirname, 'fixtures/mock-func.cjs'),
      VSEPH_LEGACY_COMMIT: 'b'.repeat(40),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  proc.stdin.end('{"body":"unknown","utc_microseconds":"0"}\n');
  const chunks = [];
  proc.stdout.on('data', (data) => chunks.push(data));
  const exit = await new Promise((resolve) => proc.on('close', resolve));
  assert.equal(exit, 2);
  const rows = Buffer.concat(chunks).toString().trim().split('\n');
  assert.equal(rows.length, 1);
  assert.equal(JSON.parse(rows[0]).protocol, 'virgostars.longitude.v2');
});
