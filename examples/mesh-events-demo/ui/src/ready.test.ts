// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import type { JsonValue } from '../../../../packages/plugin-web-sdk/src/index.ts';
import { readiness } from './ready.ts';
test('a ready update requires its application schema and a real boolean', () => {
  assert.equal(readiness({ schemaVersion: 1, ready: true }), true);
  assert.equal(readiness({ schemaVersion: 1, ready: false }), false);
  const invalid: JsonValue[] = [null, [], 'ready', { ready:true }, {schemaVersion:2,ready:true}, {schemaVersion:1,ready:'false'}];
  for (const payload of invalid) assert.equal(readiness(payload), null);
});
