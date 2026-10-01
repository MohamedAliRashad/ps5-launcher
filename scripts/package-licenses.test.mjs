import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { applicableUpstreamFiles, localDocuments, normalizeExpression, safeRelative, selectPackages } from './package-licenses.mjs';

test('retain SPDX AND; normalize only legacy Cargo alternatives', () => {
  assert.equal(normalizeExpression('MIT/Apache-2.0'), 'MIT OR Apache-2.0');
  assert.equal(normalizeExpression('ISC AND (Apache-2.0 OR ISC)'), 'ISC AND (Apache-2.0 OR ISC)');
  assert.throws(() => normalizeExpression(null), /no declared license/);
});

test('active tree excludes dev-only/optional metadata and includes duplicate versions', () => {
  const metadata = { resolve: { root: 'app' }, packages: [
    { id: 'app', name: 'app', version: '1.0.0' },
    { id: 'a1', name: 'a', version: '1.0.0' }, { id: 'a2', name: 'a', version: '2.0.0' },
    { id: 'unused', name: 'unused', version: '1.0.0' },
  ] };
  assert.deepEqual(selectPackages(metadata, 'app v1.0.0 (/app)\na v1.0.0\na v2.0.0\na v1.0.0 (*)\n').map(p => p.id), ['a1', 'a2']);
  assert.throws(() => selectPackages(metadata, 'missing v1.0.0'), /missing/);
});

test('paths cannot escape or be absolute', () => {
  for (const p of ['../LICENSE', '/etc/passwd', 'a/../b', 'a\\b', '', 'a//b']) assert.throws(() => safeRelative(p));
  assert.equal(safeRelative('vendor/NOTICE'), 'vendor/NOTICE');
});

test('upstream retains applicable notices, but not unrelated package notices', () => {
  const tree = ['LICENSE', 'NOTICE', 'crates/NOTICE', 'crates/a/LICENSE', 'crates/a/vendor/NOTICE', 'crates/b/NOTICE']
    .map(p => ({ type: 'blob', path: p, mode: '100644' }));
  assert.deepEqual(applicableUpstreamFiles(tree, 'crates/a', 'Apache-2.0', 'owner/repo').map(e => e.path), tree.slice(0, 5).map(e => e.path));
});

test('Slint includes royalty-free text, never silently substitutes commercial/GPL', () => {
  const tree = ['LICENSE.md', 'LICENSES/LicenseRef-Slint-Royalty-free-2.0.md', 'api/rs/slint/LICENSES/GPL-3.0-only.txt']
    .map(p => ({ type: 'blob', path: p, mode: '100644' }));
  assert.deepEqual(applicableUpstreamFiles(tree, 'api/rs/slint', 'GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0', 'slint-ui/slint')
    .map(e => e.path), tree.slice(0, 2).map(e => e.path));
  assert.throws(() => applicableUpstreamFiles([], '', 'LicenseRef-Slint-Royalty-free-2.0', 'slint-ui/slint'), /Missing upstream/);
});

test('local traversal preserves nested native notices byte-for-byte and rejects escaping symlinks', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'ps5-license-test-'));
  try {
    const root = path.join(dir, 'crate');
    await fs.mkdir(path.join(root, 'vendor/licenses'), { recursive: true });
    const bytes = Buffer.from('Copyright upstream\r\nOriginal notice\r\n');
    await fs.writeFile(path.join(root, 'vendor/NOTICE'), bytes);
    await fs.writeFile(path.join(root, 'vendor/licenses/terms.txt'), 'license terms');
    await fs.writeFile(path.join(root, 'implementation.rs'), 'not a licensing document');
    const documents = await localDocuments(root);
    assert.equal(documents.length, 2);
    assert.deepEqual(documents.find(d => d.relative === 'vendor/NOTICE').bytes, bytes);
    await assert.rejects(localDocuments(root, 'MISSING.txt'), /absent/);
    await fs.writeFile(path.join(dir, 'outside.txt'), 'outside');
    await fs.symlink('../outside.txt', path.join(root, 'LICENSE'));
    await assert.rejects(localDocuments(root), /escapes package/);
  } finally { await fs.rm(dir, { recursive: true, force: true }); }
});