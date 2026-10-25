// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import GameWorlds from './GameWorlds.vue';
import { setLocale } from './i18n';
import { enterPage, goToParent, initialNavigation } from './navigation';
import type { LocalWorld } from './games';
import type { WorldSnapshot } from './world-snapshots';
import { snapshotTimestamp } from './world-snapshots';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const world: LocalWorld = { folder_name: 'Survival', name: 'My world', minecraft_version: '1.21.1', game_mode: 0, hardcore: false, last_played: null, metadata_error: false };
const snapshot: WorldSnapshot = { id: 'a'.repeat(64), world_id: world.folder_name, name: 'Before adventure', created_at: '2026-10-23T00:00:00Z', processed_bytes: 1024, added_bytes: 128 };
const target = { worldId: world.folder_name, snapshotId: snapshot.id };
it('orders the native Jiff timestamp format by time rather than snapshot ID', () => {
  const before = snapshotTimestamp('2026-10-23T18:10:15.8271819+08:00[+08:00]');
  const after = snapshotTimestamp('2026-10-23T18:10:15.8851613+08:00[+08:00]');
  expect(after - before).toBe(58);
  expect(snapshotTimestamp('2026-10-23T10:10:15.885Z')).toBe(after);
});
beforeEach(() => { vi.useFakeTimers({toFake:['Date','setTimeout','clearTimeout','setInterval','clearInterval']});
  setLocale('en-US');
  invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'list_game_worlds') return [world];
    if (command === 'list_world_snapshots') return [snapshot];
    if (command === 'get_world_snapshot_storage') return null;
    return [];
  });
});
afterEach(() => {vi.useRealTimers();setLocale('zh-CN');});
const mountWorlds = (locked = false, snapshotId = snapshot.id) => mount(GameWorlds, {
  props: { gameId: 'game', locked, snapshotRestore: { ...target, snapshotId } },
  global: { stubs: { AutomaticWorldBackup: true, teleport:{template:'<slot />'} } },
});

it('opens the exact snapshot confirmation and writes only after a separate user click', async () => {
  const wrapper = mountWorlds();
  await flushPromises();
  expect(wrapper.get('.snapshot-confirm').text()).toContain('Restore Before adventure?');
  expect(wrapper.get('.snapshot-restore-target').text()).toContain('My world');
  expect(invoke.mock.calls.some(([command]) => command === 'restore_world_snapshot')).toBe(false);
  invoke.mockImplementation(async (command: string) => command === 'restore_world_snapshot' ? { worlds: [world], rollback_id: 'rollback' } : []);
  await vi.advanceTimersByTimeAsync(3000); await wrapper.get('.snapshot-confirm .confirmation-confirm').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('restore_world_snapshot', { gameId: 'game', folderName: 'Survival', snapshotId: snapshot.id, replaceExisting: true });
  wrapper.unmount();
});

it('respects the running-game lock and cancellation without writing', async () => {
  const wrapper = mountWorlds(true);
  await flushPromises();
  expect(wrapper.get('.snapshot-confirm .confirmation-confirm').attributes('disabled')).toBeDefined();
  await wrapper.get('.snapshot-confirm .confirmation-cancel').trigger('click');
  expect(wrapper.find('.snapshot-confirm').exists()).toBe(false);
  await wrapper.setProps({ snapshotRestore: { ...target } });
  await flushPromises();
  expect(wrapper.find('.snapshot-confirm').exists()).toBe(true);
  expect(invoke.mock.calls.some(([command]) => command === 'restore_world_snapshot')).toBe(false);
  wrapper.unmount();
});

it('shows a missing snapshot without selecting another version or restoring', async () => {
  const wrapper = mountWorlds(false, 'b'.repeat(64));
  await flushPromises();
  expect(wrapper.text()).toContain('This world or saved version is no longer available.');
  expect(wrapper.find('.snapshot-confirm').exists()).toBe(false);
  expect(invoke.mock.calls.some(([command]) => command === 'restore_world_snapshot')).toBe(false);
  wrapper.unmount();
});

it('returns from the host confirmation to the originating plugin and game context', () => {
  const plugin = { view: 'tools' as const, location: { pluginId: 'world-dashboard', instanceId: 'game' } };
  const state = enterPage(enterPage(initialNavigation(), plugin), { view: 'details', gameId: 'game', snapshotRestore: target });
  expect(goToParent(state).current).toEqual(plugin);
});
