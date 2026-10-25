// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { createRecoveryWorkspace, type RecoveryTarget } from './recovery-workspace.ts';
test('recovery reads exact game context and requests existing host actions without executing them', async () => {
  const calls: unknown[] = [], client = createRecoveryWorkspace(async value => { calls.push(value); return { requested: true }; });
  await client.snapshot({ gameId: 'game' });
  for (const target of [{ kind: 'modUndo', operationId: 'install-known' }, { kind: 'configRestore', configId: 'c'.repeat(64), backupId: '100-' + 'b'.repeat(64) }, { kind: 'runtimeSettings' }] as RecoveryTarget[]) await client.openRequest({ gameId: 'game', revision: 'a'.repeat(64), target });
  assert.deepEqual(calls[0], { method: 'recovery.snapshot', params: { gameId: 'game' } });
  assert.equal(calls.length, 4); assert(!JSON.stringify(calls).includes('execute'));
});
test('recovery rejects path-shaped targets, forged action kinds and missing revisions before transport', () => {
  const client = createRecoveryWorkspace(async () => { throw Error('must not call transport'); });
  assert.throws(() => client.snapshot({ gameId: '../outside' }), /RECOVERY_TARGET_INVALID/);
  assert.throws(() => client.openRequest({ gameId: 'game', revision: '', target: { kind: 'runtimeSettings' } }), /RECOVERY_SNAPSHOT_CHANGED/);
  for (const invalid of [{ kind: 'execute', command: 'anything' }, { kind: 'modUndo', operationId: '../path' }, { kind: 'runtimeSettings', path: '/private' }, { kind: 'configRestore', configId: 'c'.repeat(64), backupId: 'backup', extra: true }]) assert.throws(() => client.openRequest({ gameId: 'game', revision: 'a'.repeat(64), target: invalid as RecoveryTarget }), /RECOVERY_TARGET_INVALID/);
});
