// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import WorldDatapacks from './WorldDatapacks.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const world = { folder_name: 'Exact folder', name: 'My world', minecraft_version: null, game_mode: null, hardcore: false, last_played: null, metadata_error: false };
const pack = { file_name: '<original>.zip', name: 'Original', description: null, size_bytes: 10, pack_format: 48 };
it('scopes reads, confirmed removal and recovery to the selected world, with retry after failure', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue({ installed: [pack], removed: [pack] });
  const wrapper = mount(WorldDatapacks, { props: { gameId: 'game-id', world, locked: false } }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_world_datapacks', { id: 'game-id', folderName: 'Exact folder' });
  const button = (text: string) => wrapper.findAll('button').find(b => b.text() === text)!;
  await button('Remove file').trigger('click');
  expect(invoke).not.toHaveBeenCalledWith('move_world_datapack', expect.anything());
  vi.mocked(invoke).mockRejectedValueOnce('disk busy');
  await wrapper.get('.world-replace-confirm .secondary-button').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toBe('disk busy');
  expect(wrapper.find('.world-replace-confirm').exists()).toBe(true);
  await wrapper.get('.world-replace-confirm .secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('move_world_datapack', { id: 'game-id', folderName: 'Exact folder', fileName: pack.file_name, recover: false });
  await button('Recover file').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('move_world_datapack', { id: 'game-id', folderName: 'Exact folder', fileName: pack.file_name, recover: true });
  expect(wrapper.find('original').exists()).toBe(false);
  wrapper.unmount();
});
it('allows read refresh while the game is locked but prevents file mutations', async () => {
  setLocale('en-US'); vi.mocked(invoke).mockResolvedValue({ installed: [pack], removed: [pack] });
  const wrapper = mount(WorldDatapacks, { props: { gameId: 'game-id', world, locked: true } }); await flushPromises();
  for (const label of ['Import ZIPs', 'Remove file', 'Recover file']) expect(wrapper.findAll('button').find(b => b.text() === label)!.attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('button').find(b => b.text() === 'Refresh')!.attributes('disabled')).toBeUndefined(); wrapper.unmount();
});
