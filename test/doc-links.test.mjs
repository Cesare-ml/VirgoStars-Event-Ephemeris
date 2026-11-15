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
// Modification record (2026-11-15): synthetic offline local-link regressions.

import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {checkLocalMarkdownLinks} from '../tools/check-local-markdown-links.mjs';

const cli = fileURLToPath(new URL('../tools/check-local-markdown-links.mjs', import.meta.url));

async function fixture(files, run) {
  const root = await mkdtemp(join(tmpdir(), 'vseph-links-'));
  try {
    for (const [file, content] of Object.entries(files)) {
      const path = join(root, file);
      await mkdir(dirname(path), {recursive: true});
      await writeFile(path, content);
    }
    return await run(root);
  } finally {
    await rm(root, {recursive: true, force: true});
  }
}

test('valid root/doc and parent-relative links with fragments and queries', async () => {
  await fixture({
    'README.md': '[Spec](docs/spec.md#anchor)\n',
    'docs/spec.md': '[Root](../README.md?view=1#intro)\n',
  }, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), []));
});

test('reports missing target with source file and precise line; CLI exits nonzero', async () => {
  await fixture({'README.md': '# Title\n[missing](docs/missing.md)\n'}, async root => {
    assert.deepEqual(await checkLocalMarkdownLinks(root), [{
      file: 'README.md', line: 2, target: 'docs/missing.md', reason: 'target does not exist',
    }]);
    const result = spawnSync(process.execPath, [cli], {cwd: root, encoding: 'utf8'});
    assert.equal(result.status, 1);
    assert.match(result.stderr, /README\.md:2: target does not exist: docs\/missing\.md/);
  });
});

test('ignores pure anchors, external URLs, mailto and fenced/inline code examples', async () => {
  await fixture({'README.md': [
    '[same](#intro) [web](https://example.org/no.md) [mail](mailto:hello@example.org)',
    '```md',
    '[fake](no-file.md)',
    '```',
    '~~~',
    '[fake too](missing.md)',
    '~~~',
    '`[inline-code](also-missing.md)`',
  ].join('\n')}, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), []));
});

test('resolves percent-encoded paths, including encoded spaces and hashes', async () => {
  await fixture({
    'README.md': '[encoded](docs/one%20two%23three.md?raw=1#section)',
    'docs/one two#three.md': 'Found.',
  }, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), []));
});

test('validates reference definitions, images, and nested parentheses in relative targets', async () => {
  await fixture({
    'README.md': '![image](docs/icon.png)\n[ref]: docs/folder(1)/notes.md\n[use][ref]',
    'docs/icon.png': 'png-placeholder',
    'docs/folder(1)/notes.md': 'Note.',
  }, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), []));
});

test('rejects traversal outside the checkout, malformed encoding, and nested missing docs', async () => {
  await fixture({
    'docs/nested/index.md': '[traversal](../../../outside.md)\n[bad](%XX.md)\n[missing](../not-here.md)',
  }, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), [
    {file: 'docs/nested/index.md', line: 1, target: '../../../outside.md', reason: 'target outside repository'},
    {file: 'docs/nested/index.md', line: 2, target: '%XX.md', reason: 'invalid percent encoding'},
    {file: 'docs/nested/index.md', line: 3, target: '../not-here.md', reason: 'target does not exist'},
  ]));
});

test('success CLI output is deterministic and exits zero', async () => {
  await fixture({'README.md': '[Valid](docs/a.md)', 'docs/a.md': 'Hello'}, async root => {
    const result = spawnSync(process.execPath, [cli], {cwd: root, encoding: 'utf8'});
    assert.equal(result.status, 0);
    assert.match(result.stdout, /Repository-local Markdown links: OK/);
  });
});

test('nested quoted fences ignore example links and preserve following source lines', async () => {
  await fixture({
    'README.md': [
      '> ```md',
      '> [ignored](missing-in-code.md)',
      '> ```',
      '> > ~~~markdown',
      '> > [also ignored](another-missing.md)',
      '> > ~~~',
      '> [exists](docs/real.md)',
      '[broken](after-quote.md)',
    ].join('\n'),
    'docs/real.md': 'Present.',
  }, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), [
    {file: 'README.md', line: 8, target: 'after-quote.md', reason: 'target does not exist'},
  ]));
});

test('ignores space- and tab-indented code including inside blockquotes', async () => {
  await fixture({'README.md': [
    '    [ignored](missing-spaces.md)',
    '\t[ignored](missing-tab.md)',
    '>     [ignored](missing-quoted-code.md)',
    '> [real reference]: docs/actual.md',
    '> [broken](missing-real.md)',
  ].join('\n'), 'docs/actual.md': 'Present.'}, async root => {
    assert.deepEqual(await checkLocalMarkdownLinks(root), [
      {file: 'README.md', line: 5, target: 'missing-real.md', reason: 'target does not exist'},
    ]);
  });
});

test('fenced quote does not swallow unquoted content when its container ends', async () => {
  await fixture({'README.md': [
    '> ```md',
    '> [example](never-check.md)',
    '[outside](missing.md)',
  ].join('\n')}, async root => assert.deepEqual(await checkLocalMarkdownLinks(root), [
    {file: 'README.md', line: 3, target: 'missing.md', reason: 'target does not exist'},
  ]));
});
