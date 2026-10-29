// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { catalogEntry, digest } from './package-release.mjs';
test('signed package descriptors bind exact archive bytes and localized metadata', () => {
  const bytes = Buffer.from('archive');
  const manifest = { name: 'Example', description: { 'zh-CN': '本地工具' } };
  const entry = catalogEntry(manifest, bytes, 'https://example.org/v1/', 'example-0.1.0.zip');
  assert.deepEqual(entry.package, { url: 'https://example.org/v1/example-0.1.0.zip', sha256: digest(bytes), size: bytes.length });
  assert.equal(entry.description, '本地工具');
  assert.equal(entry.manifest, manifest);
  assert.equal(entry.component_url, undefined);
});
test('release descriptors reject unsafe origins and archive names', () => {
  for (const base of ['http://example.org', 'https://user:secret@example.org', 'https://example.org/#x', 'https://example.org/?x']) assert.throws(() => catalogEntry({}, Buffer.alloc(1), base, 'ok.zip'));
  for (const name of ['../bad.zip', 'bad.zip?x', 'bad\\name.zip']) assert.throws(() => catalogEntry({}, Buffer.alloc(1), 'https://example.org', name));
});
