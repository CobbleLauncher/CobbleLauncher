// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHandoffs, WAYPOINT_IMPORT_INTERFACE as interfaceId, WAYPOINT_IMPORT_VERSION as version, type HandoffRequest } from './handoffs.ts';

test('sends only a bounded typed waypoint through the fixed interface', async () => {
  const calls: HandoffRequest[] = [];
  const client = createHandoffs(async request => { calls.push(request); return { requested: true }; });
  await client.send({ interfaceId, version, targetPluginId: 'coordinate-book', gameId: 'game-a', worldId: 'My world', payload: { name: 'Village', dimension: 'overworld', x: -29_999_984, y: -64, z: 29_999_984, source: 'Seed Compass' } });
  assert.deepEqual(calls, [{ method: 'handoffs.send', params: { interfaceId, version, targetPluginId: 'coordinate-book', gameId: 'game-a', worldId: 'My world', payload: { name: 'Village', dimension: 'overworld', x: -29_999_984, y: -64, z: 29_999_984, source: 'Seed Compass' } } }]);
  assert.throws(() => client.send({ interfaceId, version, targetPluginId: 'coordinate-book', gameId: 'game-a', payload: { name: 'Bad', dimension: 'overworld', x: 29_999_985, z: 0 } }));
  assert.throws(() => client.send({ interfaceId, version, targetPluginId: 'coordinate-book', gameId: 'game-a', worldId: '../private', payload: { name: 'Bad', dimension: 'overworld', x: 0, z: 0 } }));
  assert.equal(calls.length, 1);
});

test('limits handoff calls to the supported interface and offers explicit receiver/take operations', async () => {
  const calls: HandoffRequest[] = [];
  const draft = { handoffId: 'opaque-token', sender: { pluginId: 'seed-compass', name: 'Seed Compass' }, game: { id: 'game-a', name: 'Game A' }, payload: { name: 'Village', dimension: 'overworld' as const, x: 12, z: 34 } };
  const client = createHandoffs(async request => { calls.push(request); return request.method === 'handoffs.receivers' ? [{ pluginId: 'coordinate-book', name: 'Coordinate Book' }] : draft; });
  assert.deepEqual(await client.receivers({ interfaceId, version }), [{ pluginId: 'coordinate-book', name: 'Coordinate Book' }]);
  assert.deepEqual(await client.take({ interfaceId, version }), draft);
  assert.deepEqual(calls, [{ method: 'handoffs.receivers', params: { interfaceId, version } }, { method: 'handoffs.take', params: { interfaceId, version } }]);
  assert.throws(() => client.receivers({ interfaceId: 'launcher:anything' as typeof interfaceId, version }));
});
