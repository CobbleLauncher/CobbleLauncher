// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { createClient, type Request } from './index.ts';

test('private documents preserve scope and revision without caller-selected plugin identity', async () => {
  const calls: Request[] = [];
  const client = createClient(async request => { calls.push(request); return { value: null, revision: null }; });
  const scope = { gameId: 'game', worldId: '我的世界' };
  await client.data.read({ scope, key: 'locations-v1' });
  await client.data.write({ scope, key: 'locations-v1', value: { locations: [] }, expectedRevision: null });
  await client.data.remove({ scope, key: 'locations-v1', expectedRevision: 'revision' });
  assert.deepEqual(calls, [
    { method: 'data.read', params: { scope, key: 'locations-v1' } },
    { method: 'data.write', params: { scope, key: 'locations-v1', value: { locations: [] }, expectedRevision: null } },
    { method: 'data.remove', params: { scope, key: 'locations-v1', expectedRevision: 'revision' } },
  ]);
  assert.equal('pluginId' in calls[0]!.params, false);
});

test('reject invalid keys, world scopes and revisions before transport', () => {
  let calls = 0;
  const client = createClient(async () => { calls++; return null; });
  for (const key of ['', '../other', 'a/b', 'x'.repeat(65)]) assert.throws(() => client.data.read({ scope: {}, key }));
  for (const scope of [{ worldId: 'World' }, { gameId: 'g', worldId: '../other' }, { gameId: '' }]) assert.throws(() => client.data.read({ scope, key: 'book' }));
  assert.throws(() => client.data.write({ scope: {}, key: 'book', value: null, expectedRevision: '' }));
  assert.throws(() => client.view.setDirty({ dirty: 'yes' as never }));
  assert.equal(calls, 0);
});

test('dirty state uses the current native-authorized view only', async () => {
  const calls: Request[] = [];
  const client = createClient(async request => { calls.push(request); return null; });
  await client.view.setDirty({ dirty: true });
  await client.view.setDirty({ dirty: false });
  assert.deepEqual(calls, [
    { method: 'view.setDirty', params: { dirty: true } },
    { method: 'view.setDirty', params: { dirty: false } },
  ]);
});

test('server documents require explicit exclusive game-bound opaque IDs', async () => {
  const calls: Request[] = [];
  const client = createClient(async request => { calls.push(request); return null; });
  const serverId = 'a'.repeat(64);
  await client.data.read({scope:{gameId:'g',serverId},key:'commands'});
  assert.deepEqual(calls[0]!.params, {scope:{gameId:'g',serverId},key:'commands'});
  for (const scope of [{serverId}, {gameId:'g',worldId:'w',serverId}, {gameId:'g',serverId:'example.org'}, {gameId:'g',serverId:'0'}]) {
    assert.throws(() => client.data.read({scope,key:'commands'}), /PLUGIN_SERVER_SCOPE/);
  }
  assert.equal(calls.length, 1);
});
