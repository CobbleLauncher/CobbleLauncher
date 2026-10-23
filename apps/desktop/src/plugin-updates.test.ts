// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { availablePluginUpdates, batchCompatible } from './plugin-updates';
import type { CatalogManifest } from './plugin-catalogs';
const previous: CatalogManifest = { id: 'test', name: 'Test', version: '1.0.0', publisher: 'Publisher', api_version: '1.0.0', capabilities: ['viewer'], file_extensions: ['schem'], component_sha256: 'a'.repeat(64) };
const next = { ...previous, version: '1.1.0' };
it('offers only strictly newer stable versions and preserves all source choices', () => {
  expect(availablePluginUpdates([previous], [previous, { ...next, version: '0.9.0' }, { ...next, version: '2.0.0-beta' }], [])).toEqual([]);
  expect(availablePluginUpdates([previous], [next], [
    { source: { id: 'disabled', name: 'Disabled', url: 'https://example.org', enabled: false }, snapshot: null },
    { source: { id: 'other', name: 'Other', url: 'https://example.org', enabled: true }, snapshot: { source_id: 'other', source_url: 'https://example.org', retrieved_at: 1, index: { schema_version: 1, name: 'Other', plugins: [{ manifest: next, description: '' }] } } },
  ])).toHaveLength(2);
});
it('requires individual review for changed publishers, contracts and expanded permissions', () => {
  expect(batchCompatible(next, previous)).toBe(true);
  expect(batchCompatible({ ...next, publisher: 'Other' }, previous)).toBe(false);
  expect(batchCompatible({ ...next, api_version: '2.0.0' }, previous)).toBe(false);
  expect(batchCompatible({ ...next, file_extensions: ['schem', 'litematic'] }, previous)).toBe(false);
  expect(batchCompatible({ ...next, permissions: [{ permission: 'launcher:game-keybindings/write' }] }, previous)).toBe(false);
});
