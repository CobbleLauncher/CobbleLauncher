// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createConfigWorkspace, type ConfigWorkspaceRequest } from './config-workspace.ts';

const fileId = 'a'.repeat(64), backupId = `1761234567890-${'b'.repeat(64)}`, revision = 'c'.repeat(64);

test('sends only opaque managed-config identifiers and expected revisions', async () => {
  const calls: ConfigWorkspaceRequest[] = [];
  const client = createConfigWorkspace(async request => { calls.push(request); return {}; });
  await client.list('game-a');
  await client.read('game-a', fileId);
  await client.readBackup('game-a', fileId, backupId);
  await client.write('game-a', fileId, revision, '{"safe":true}');
  await client.restore('game-a', fileId, backupId, revision);
  assert.deepEqual(calls.map(call => call.method), [
    'configWorkspace.list', 'configWorkspace.read', 'configWorkspace.readBackup', 'configWorkspace.write', 'configWorkspace.restore',
  ]);
  assert.deepEqual(calls[2]?.params, { gameId: 'game-a', configId: fileId, backupId });
  assert.deepEqual(calls[4]?.params, { gameId: 'game-a', configId: fileId, backupId, expectedRevision: revision });
});

test('rejects paths, malformed history IDs, NUL and oversized content before transport', async () => {
  let calls = 0;
  const client = createConfigWorkspace(async () => { calls++; return {}; });
  assert.throws(() => client.read('game-a', '../config.json'));
  assert.throws(() => client.readBackup('game-a', fileId, '../backup'));
  assert.throws(() => client.readBackup('game-a', fileId, `${'1'.repeat(40)}-${'a'.repeat(64)}`));
  assert.throws(() => client.write('game-a', fileId, revision, 'bad\0text'));
  assert.throws(() => client.write('game-a', fileId, revision, 'x'.repeat(1024 * 1024 + 1)));
  assert.equal(calls, 0);
});
