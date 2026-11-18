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
const { mkdtempSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { readPinnedJson } = require('../pinned_json.cjs');

function fixture(t, bytes) {
  const dir = mkdtempSync(path.join(tmpdir(), 'vseph-pinned-json-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'source.json');
  writeFileSync(file, bytes);
  return { file, sha256: createHash('sha256').update(bytes).digest('hex') };
}

test('parses precisely the UTF-8 bytes authenticated by the supplied SHA-256', t => {
  const bytes = Buffer.from('{"body":"Luna","note":"β eclittica"}\n');
  const f = fixture(t, bytes);
  assert.deepEqual(readPinnedJson(f.file, f.sha256, bytes.length, 'oracle'),
    { body: 'Luna', note: 'β eclittica' });
});

test('rejects mutated content even when the replacement is valid JSON', t => {
  const original = Buffer.from('{"candidate":true}\n');
  const f = fixture(t, original);
  writeFileSync(f.file, '{"candidate":false}\n');
  assert.throws(() => readPinnedJson(f.file, f.sha256, 1024, 'oracle'),
    /SHA-256 differs/);
});

test('bounds bytes actually read and allows files exactly at the limit', t => {
  const bytes = Buffer.from('{"a":1}');
  const f = fixture(t, bytes);
  assert.deepEqual(readPinnedJson(f.file, f.sha256, bytes.length, 'oracle'), { a: 1 });
  assert.throws(() => readPinnedJson(f.file, f.sha256, bytes.length - 1, 'oracle'),
    /exceeds 6 bytes/);
});

test('rejects invalid UTF-8 and malformed JSON despite a valid byte digest', t => {
  for (const bytes of [Buffer.from([0x7b, 0x22, 0xff, 0x22, 0x3a, 0x31, 0x7d]),
    Buffer.from('{"malformed":}')]) {
    const f = fixture(t, bytes);
    assert.throws(() => readPinnedJson(f.file, f.sha256, 1024, 'oracle'),
      /invalid oracle UTF-8\/JSON/);
  }
});
