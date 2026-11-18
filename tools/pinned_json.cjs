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
  try {
    const decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    return JSON.parse(decoded);
  } catch {
    throw new Error(`invalid ${label} UTF-8/JSON`);
  }
}

module.exports = { readPinnedJson };
