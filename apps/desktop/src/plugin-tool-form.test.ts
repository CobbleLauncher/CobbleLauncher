// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { isSupportedCatalogPlugin, type CatalogManifest } from './plugin-catalogs';
const manifest: CatalogManifest = {
  id: 'pure-tool', name: 'Pure tool', version: '0.1.0', publisher: 'Original fixture',
  api_version: '0.1.0', capabilities: ['tool-form'], file_extensions: [], permissions: [],
  component_sha256: 'a'.repeat(64),
  tool_form: { schema_version: 1, schema: { type: 'object', additionalProperties: false,
    properties: { text: { type: 'string', default: 'Hello', maxLength: 256 } }, required: ['text'] } },
};
it('recognizes the pure tool ABI with a transient host form and no ambient capabilities', () => {
  const supported = (candidate: CatalogManifest) => isSupportedCatalogPlugin({ manifest: candidate, description: '' });
  expect(supported(manifest)).toBe(true);
  expect(supported({ ...manifest, tool_form: undefined })).toBe(false);
  expect(supported({ ...manifest, api_version: '0.2.0' })).toBe(false);
  expect(supported({ ...manifest, file_extensions: ['json'] })).toBe(false);
  expect(supported({ ...manifest, permissions: [{ permission: 'launcher:resource/selected/read', extensions: ['json'] }] })).toBe(false);
  expect(supported({ ...manifest, capabilities: ['report'] })).toBe(false);
  expect(supported({ ...manifest, interfaces: { provides: [{ id: 'launcher:tool-form', version: '2.0.0' }] } })).toBe(false);
});
