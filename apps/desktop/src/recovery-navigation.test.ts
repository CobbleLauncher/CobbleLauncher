// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { parseRecoveryNavigation, recoveryNavigationPage } from './recovery-navigation';
import { enterPage, initialNavigation, pageKey } from './navigation';
it('routes only known recovery targets to the existing host or config tool context', () => {
  const request = (target: unknown) => parseRecoveryNavigation({ game_id: 'game', plugin_id: 'crash-doctor', target })!;
  expect(recoveryNavigationPage(request({ kind: 'modUndo', operationId: 'install-test' }))).toEqual({ view: 'details', gameId: 'game', recoveryTarget: { kind: 'modUndo', operationId: 'install-test' } });
  expect(recoveryNavigationPage(request({ kind: 'runtimeSettings' }))).toEqual({ view: 'details', gameId: 'game', recoveryTarget: { kind: 'runtimeSettings' } });
  expect(recoveryNavigationPage(request({ kind: 'configRestore', configId: 'a'.repeat(64), backupId: '100-backup' }))).toEqual({ view: 'tools', location: { pluginId: 'config-doctor', instanceId: 'game', configRestoreTarget: { configId: 'a'.repeat(64), backupId: '100-backup' } } });
});
it('rejects malformed events and path-shaped or extra action fields', () => {
  for (const target of [{ kind: 'exec', command: 'anything' }, { kind: 'modUndo', operationId: '../private' }, { kind: 'runtimeSettings', path: '/private' }, { kind: 'configRestore', configId: 'bad', backupId: 'anything' }]) expect(parseRecoveryNavigation({ game_id: 'game', plugin_id: 'crash-doctor', target })).toBeNull();
  expect(parseRecoveryNavigation({ game_id: '../outside', plugin_id: 'crash-doctor', target: { kind: 'runtimeSettings' } })).toBeNull();
});
it('keeps recovery selections transient when leaving their page', () => {
  const current = enterPage(initialNavigation(), { view: 'tools', location: { pluginId: 'config-doctor', instanceId: 'game', configRestoreTarget: { configId: 'a'.repeat(64), backupId: 'backup' } } });
  const next = enterPage(current, { view: 'details', gameId: 'game' });
  expect(next.parents.at(-1)).toEqual({ view: 'tools', location: { pluginId: 'config-doctor', instanceId: 'game' } });
  expect(pageKey(current.current)).not.toBe(pageKey({ view: 'tools', location: { pluginId: 'config-doctor', instanceId: 'game' } }));
});
