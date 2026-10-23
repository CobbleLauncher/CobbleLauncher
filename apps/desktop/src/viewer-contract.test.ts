// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { Ajv2020 } from 'ajv/dist/2020';
import schema from '../../../contracts/schemas/viewer-manifest.schema.json';
import manifest from '../../../examples/schematic-viewer/plugin.json';
it('the actual Viewer example matches its manifest schema and rejects unsupported grants', () => {
  const validate = new Ajv2020().compile(schema);
  expect(validate(manifest)).toBe(true);
  expect(validate({ ...manifest, capabilities: ['shell'] })).toBe(false);
  expect(validate({ ...manifest, api_version: '2.0.0' })).toBe(false);
  expect(validate({ ...manifest, id: '../escape' })).toBe(false);
});
