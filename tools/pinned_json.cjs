/*
 * Common Public Attribution License Version 1.0 (CPAL-1.0).
 * Original Developer: Cesare-ml (VirgoStars project).
 * Copyright (c) 2026 Cesare-ml. All Rights Reserved.
 * See LICENSE, Exhibits A/B and NOTICE; provided AS IS.
 * Attribution: Powered by VirgoStars — https://www.virgostars.it/
 */
'use strict';

// Development-only trust anchor for small JSON inputs. Hash exactly the bytes
// that are subsequently parsed; a path-based second read would be a TOCTOU gap.
const { createHash } = require('node:crypto');
const { openSync, fstatSync, readSync, closeSync } = require('node:fs');
const { TextDecoder } = require('node:util');

// JSON.parse silently keeps the final member of an object with repeated names.
// Inspect keys before parsing, including names written with JSON escapes.
function rejectDuplicateObjectKeys(source, label) {
  let cursor = 0;
  const invalid = () => { throw new Error(`invalid ${label} JSON structure`); };
  const whitespace = char => char === ' ' || char === '\t' || char === '\r' || char === '\n';
  function skipSpace() {
    while (cursor < source.length && whitespace(source[cursor])) cursor++;
  }
  function scanString(decode) {
    if (source[cursor] !== '"') invalid();
    const start = cursor++;
    while (cursor < source.length) {
      if (source[cursor] === '\\') {
        cursor += 2;
      } else if (source[cursor] === '"') {
        cursor++;
        if (!decode) return;
        try { return JSON.parse(source.slice(start, cursor)); }
        catch { invalid(); }
      } else {
        cursor++;
      }
    }
    invalid();
  }
  function scanValue(depth) {
    if (depth > 128) throw new Error(`${label} JSON nesting exceeds 128 levels`);
    skipSpace();
    if (source[cursor] === '{') {
      cursor++;
      skipSpace();
      if (source[cursor] === '}') { cursor++; return; }
      const seen = new Set();
      while (true) {
        const key = scanString(true);
        if (seen.has(key)) throw new Error(`${label} has duplicate JSON object keys`);
        seen.add(key);
        skipSpace();
        if (source[cursor] !== ':') invalid();
        cursor++;
        scanValue(depth + 1);
        skipSpace();
        if (source[cursor] === '}') { cursor++; return; }
        if (source[cursor] !== ',') invalid();
        cursor++;
        skipSpace();
      }
    }
    if (source[cursor] === '[') {
      cursor++;
      skipSpace();
      if (source[cursor] === ']') { cursor++; return; }
      while (true) {
        scanValue(depth + 1);
        skipSpace();
        if (source[cursor] === ']') { cursor++; return; }
        if (source[cursor] !== ',') invalid();
        cursor++;
      }
    }
    if (source[cursor] === '"') { scanString(false); return; }
    const start = cursor;
    while (cursor < source.length && !whitespace(source[cursor]) &&
        !',]}'.includes(source[cursor])) cursor++;
    if (cursor === start) invalid();
  }
  scanValue(0);
  skipSpace();
  if (cursor !== source.length) invalid();
}

function readPinnedJson(file, expectedSha256, maxBytes, label) {
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 ||
      typeof expectedSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(expectedSha256)) {
    throw new Error(`invalid ${label} trust parameters`);
  }
  const fd = openSync(file, 'r');
  let bytes;
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile()) throw new Error(`${label} must be a regular file`);
    if (stat.size > maxBytes) throw new Error(`${label} exceeds ${maxBytes} bytes`);

    // Read one extra byte to catch growth after fstat without unbounded I/O.
    const buffer = Buffer.alloc(maxBytes + 1);
    let used = 0;
    while (used < buffer.length) {
      const n = readSync(fd, buffer, used, buffer.length - used, null);
      if (n === 0) break;
      used += n;
    }
    if (used > maxBytes) throw new Error(`${label} exceeds ${maxBytes} bytes`);
    bytes = buffer.subarray(0, used);
  } finally {
    closeSync(fd);
  }

  if (createHash('sha256').update(bytes).digest('hex') !== expectedSha256) {
    throw new Error(`${label} SHA-256 differs from pinned trust anchor`);
  }
  let decoded;
  try { decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes); }
  catch { throw new Error(`invalid ${label} UTF-8`); }
  rejectDuplicateObjectKeys(decoded, label);
  try { return JSON.parse(decoded); }
  catch { throw new Error(`invalid ${label} JSON`); }
}

module.exports = { readPinnedJson };
