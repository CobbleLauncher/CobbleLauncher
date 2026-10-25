// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { createClient, type Request } from './index.ts';
test('clipboard exposes only bounded text writing and preserves OS failures', async () => {
  const calls: Request[] = [];
  const client = createClient(async request => { calls.push(request); return null; });
  await client.clipboard.writeText('X -81 · Y 64 · Z 120');
  assert.deepEqual(calls, [{ method: 'clipboard.writeText', params: { text: 'X -81 · Y 64 · Z 120' } }]);
  assert.equal('readText' in client.clipboard, false);
  assert.throws(() => client.clipboard.writeText('a\0b'));
  assert.throws(() => client.clipboard.writeText('界'.repeat(3000)));
  assert.equal(calls.length, 1);
  await assert.rejects(createClient(async () => { throw Error('PLUGIN_CLIPBOARD_UNAVAILABLE'); }).clipboard.writeText('hello'), /PLUGIN_CLIPBOARD_UNAVAILABLE/);
});
