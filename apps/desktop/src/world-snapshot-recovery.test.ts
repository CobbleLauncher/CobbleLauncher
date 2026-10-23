// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import WorldSnapshotsPanel from './WorldSnapshotsPanel.vue';
import type { LocalWorld } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const world: LocalWorld = { folder_name: 'World', name: 'My world', minecraft_version: null, game_mode: null, hardcore: false, last_played: null, metadata_error: false };
function mountPanel() { setLocale('en-US'); return mount(WorldSnapshotsPanel, { props: { gameId: 'owned-game', world, locked: false } }); }
it('exports through a native dialog using only game id and reports only completion', async () => {
  vi.mocked(invoke).mockResolvedValueOnce([]).mockResolvedValueOnce(false).mockResolvedValueOnce(true);
  const wrapper = mountPanel(); await flushPromises();
  const actions = wrapper.findAll('.snapshot-recovery button');
  expect(wrapper.get('.snapshot-recovery').text()).toContain('Keep this key private');
  await actions[0]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('export_world_snapshot_recovery', { gameId: 'owned-game' });
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  await actions[0]!.trigger('click'); await flushPromises();
  expect(wrapper.get('[role="status"]').text()).toBe('Recovery key exported');
  expect(wrapper.emitted('busy')).toEqual([[true], [false], [true], [false]]);
  wrapper.unmount();
});
it('allows importing after keyring/list failure then refreshes history without handling key material', async () => {
  vi.mocked(invoke).mockRejectedValueOnce('Key unavailable').mockResolvedValueOnce(true).mockResolvedValueOnce([]);
  const wrapper = mountPanel(); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Key unavailable');
  await wrapper.findAll('.snapshot-recovery button')[1]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenNthCalledWith(2, 'import_world_snapshot_recovery', { gameId: 'owned-game' });
  expect(invoke).toHaveBeenNthCalledWith(3, 'list_world_snapshots', { gameId: 'owned-game', folderName: 'World' });
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(wrapper.get('[role="status"]').text()).toBe('Recovery key imported');
  wrapper.unmount();
});
it('surfaces recovery failure without success and keeps mutations disabled while dialog is pending', async () => {
  let reject!: (reason: string) => void;
  vi.mocked(invoke).mockResolvedValueOnce([]).mockImplementationOnce(() => new Promise((_resolve, rejectPromise) => { reject = rejectPromise; }));
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.findAll('.snapshot-recovery button')[1]!.trigger('click');
  expect(wrapper.get('form button').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('.snapshot-recovery button')[0]!.attributes('disabled')).toBeDefined();
  reject('Invalid recovery key'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Invalid recovery key');
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  wrapper.unmount();
});
