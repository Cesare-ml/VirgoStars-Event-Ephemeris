/*
EXHIBIT A. Common Public Attribution License Version 1.0.
The contents of this file are subject to the Common Public Attribution License Version 1.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at https://opensource.org/license/CPAL-1.0. The License is based on the Mozilla Public License Version 1.1 but Sections 14 and 15 have been added to cover use of software over a computer network and provide for limited attribution for the Original Developer. In addition, Exhibit A has been modified to be consistent with Exhibit B.
Software distributed under the License is distributed on an "AS IS" basis, WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License for the specific language governing rights and limitations under the License.
The Original Code is the original VirgoStars-Event-Ephemeris source code released under CPAL-1.0 by the Initial Developer, excluding third-party programs, data, and other materials licensed separately.
The Original Developer is the Initial Developer.
The Initial Developer of the Original Code is Cesare-ml (VirgoStars project). All portions of the code written by Cesare-ml are Copyright (c) 2026 Cesare-ml. All Rights Reserved.
Contributors retain applicable rights in their own contributions under the terms of this License.
SPDX-License-Identifier: CPAL-1.0
*/
// Modification record (2026-11-15): offline repository-local Markdown link checks.
// Modification record (2026-11-18): reject Windows cross-volume paths.

import {readdir, readFile, realpath, stat} from 'node:fs/promises';
import {dirname, extname, isAbsolute, relative, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;

// Use the same containment rule for lexical paths and resolved symlink paths.
export function insideRoot(root, pathname, paths = {relative, resolve, sep, isAbsolute}) {
  const candidate = paths.relative(root, pathname);
  // Windows returns an absolute path, not "..", when volumes differ.
  return !paths.isAbsolute(candidate) && candidate !== '..' &&
    !candidate.startsWith('..' + paths.sep) &&
    (!candidate || paths.resolve(root, candidate) === pathname);
}

async function markdownFiles(root, canonicalRoot) {
  const result = [];
  const entries = await readdir(root, {withFileTypes: true});
  for (const entry of entries.sort((a, b) => compare(a.name, b.name))) {
    if (entry.isFile() && extname(entry.name).toLowerCase() === '.md')
      result.push(resolve(root, entry.name));
  }
  const docs = resolve(root, 'docs');
  // A missing docs/ directory is optional; a dangling symlink is not.
  const docsEntry = entries.find(entry => entry.name === 'docs');
  if (!docsEntry || (!docsEntry.isDirectory() && !docsEntry.isSymbolicLink())) return result;
  // A root docs/ symlink must not make the walker leave the checkout.
  try {
    const realDocs = await realpath(docs);
    if (!insideRoot(canonicalRoot, realDocs))
      throw new Error('docs directory resolves outside repository');
    if (!(await stat(docs)).isDirectory()) return result;
  } catch (error) {
    if (['ENOENT', 'ENOTDIR', 'ELOOP'].includes(error.code))
      throw new Error('docs directory cannot be resolved');
    throw error;
  }
  async function walk(directory) {
    const children = await readdir(directory, {withFileTypes: true});
    for (const entry of children.sort((a, b) => compare(a.name, b.name))) {
      const pathname = resolve(directory, entry.name);
      if (entry.isDirectory()) await walk(pathname);
      else if (entry.isFile() && extname(entry.name).toLowerCase() === '.md')
        result.push(pathname);
    }
  }
  await walk(docs);
  return result.sort(compare);
}

// Markdown link destinations, including images and reference definitions.
// A balanced parenthesis in a destination is legal; a space may introduce a title.
function destinations(line) {
  const found = [];
  const inline = /\]\(\s*/g;
  for (const match of line.matchAll(inline)) {
    let i = match.index + match[0].length;
    if (line[i] === '<') {
      const end = line.indexOf('>', i + 1);
      if (end >= 0 && /^\s*\)/.test(line.slice(end + 1)))
        found.push(line.slice(i + 1, end));
      continue;
    }
    const start = i;
    let depth = 0;
    for (; i < line.length; i++) {
      if (line[i] === '\\' && i + 1 < line.length) { i++; continue; }
      if (line[i] === '(') depth++;
      else if (line[i] === ')') {
        if (depth === 0) break;
        depth--;
      } else if (/\s/.test(line[i]) && depth === 0) break;
    }
    const target = line.slice(start, i).replace(/\\([ ()])/g, '$1');
    if (target && /^\s*(?:(?:"[^"]*"|'[^']*'|\([^)]*\))\s*)?\)/.test(line.slice(i)))
      found.push(target);
  }
  const reference = line.match(/^ {0,3}\[[^\]]+\]:\s*(<[^>]*>|[^\s]+)(?:\s|$)/);
  if (reference) {
    const target = reference[1];
    found.push(target.startsWith('<') && target.endsWith('>') ? target.slice(1, -1) : target);
  }
  return found;
}

// Strip CommonMark blockquote containers before recognizing code blocks.
// Keep depth so a fenced block cannot swallow text after the quote ends.
function stripBlockquoteMarkers(line) {
  let content = line;
  let depth = 0;
  while (true) {
    const marker = content.match(/^ {0,3}> ?/);
    if (!marker) return {content, depth};
    content = content.slice(marker[0].length);
    depth++;
  }
}

function targetsFromMarkdown(source) {
  const result = [];
  let fence = null;
  const lines = source.split(/\r?\n/);
  for (let index = 0; index < lines.length; index++) {
    const {content, depth} = stripBlockquoteMarkers(lines[index]);
    // A quote's fenced block ends when its container depth changes.
    if (fence && fence.depth !== depth) fence = null;
    const marker = content.match(/^ {0,3}(`{3,}|~{3,})/);
    if (fence) {
      if (marker && marker[1][0] === fence.marker[0] &&
          marker[1].length >= fence.marker.length &&
          content.slice(marker[0].length).trim() === '') fence = null;
      continue;
    }
    // Indented code blocks are examples, not link-bearing prose.
    if (/^(?: {4,}|\t)/.test(content)) continue;
    if (marker) { fence = {marker: marker[1], depth}; continue; }
    const withoutInlineCode = content.replace(/(`+)(.*?)\1/g, '');
    for (const target of destinations(withoutInlineCode))
      result.push({line: index + 1, target});
  }
  return result;
}

function localPath(root, sourceFile, destination) {
  const target = destination.trim();
  if (!target || target.startsWith('#') || target.startsWith('//') ||
      target.startsWith('/') || /^[a-z][a-z\d+.-]*:/i.test(target)) return null;
  const withoutSuffix = target.split(/[?#]/, 1)[0];
  if (!withoutSuffix) return null;
  let decoded;
  try { decoded = decodeURIComponent(withoutSuffix); }
  catch { return {error: 'invalid percent encoding'}; }
  const full = resolve(dirname(sourceFile), decoded);
  if (!insideRoot(root, full))
    return {error: 'target outside repository'};
  return {path: full};
}

/** Deterministic, network-free result; caller decides how to report failures. */
export async function checkLocalMarkdownLinks(repoRoot) {
  const root = resolve(repoRoot);
  const canonicalRoot = await realpath(root);
  const errors = [];
  for (const file of await markdownFiles(root, canonicalRoot)) {
    const text = await readFile(file, 'utf8');
    for (const {line, target} of targetsFromMarkdown(text)) {
      const local = localPath(root, file, target);
      if (!local) continue;
      let reason = local.error;
      if (!reason) {
        try {
          const resolvedTarget = await realpath(local.path);
          if (!insideRoot(canonicalRoot, resolvedTarget))
            reason = 'target outside repository';
        } catch (error) {
          if (error.code === 'ENOENT' || error.code === 'ENOTDIR')
            reason = 'target does not exist';
          else if (error.code === 'ELOOP') reason = 'target cannot be resolved';
          else throw error;
        }
      }
      if (reason) errors.push({file: relative(root, file).split(sep).join('/'), line, target, reason});
    }
  }
  return errors;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const errors = await checkLocalMarkdownLinks(process.cwd());
    for (const issue of errors)
      console.error(`${issue.file}:${issue.line}: ${issue.reason}: ${issue.target}`);
    if (errors.length) {
      console.error(`Found ${errors.length} broken repository-local Markdown link(s).`);
      process.exitCode = 1;
    } else {
      console.log('Repository-local Markdown links: OK');
    }
  } catch (error) {
    console.error(`Markdown link check failed: ${error.message}`);
    process.exitCode = 1;
  }
}
