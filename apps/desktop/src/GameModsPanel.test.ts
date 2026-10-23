// SPDX-License-Identifier: MIT
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameModsPanel from './GameModsPanel.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { document.body.innerHTML = ''; vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
it('uses the existing removal confirmation from the mod context menu', async () => {
  setLocale('en-US');
  const mod = { file_name: 'example.jar', name: 'Example', version: '1', authors: [], loader: 'fabric', enabled: true, description: null };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mods' ? [mod] : []);
  const wrapper = mount(GameModsPanel, { props: { game: { id: 'game', name: 'Game', version: '1.21.1', loader_version: '0.16', directory: 'private', status: 'installed', error: null, progress: null }, locked: false }, attachTo: document.body, global: { stubs: { ModUpdates: true, ModDetailsPanel: true } } });
  await flushPromises();
  await wrapper.get('.mod-details-link').trigger('contextmenu', { button: 2, clientX: 80, clientY: 80 }); await flushPromises();
  const menu = new DOMWrapper(document.body.querySelector('[role="menu"]')!);
  expect(menu.text()).toContain('Disable Example');
  await menu.findAll('[role="menuitem"]').find(item => item.text() === 'Remove Example')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Confirm removal');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'remove_game_mod')).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Confirm removal')!.trigger('click');
  expect(invoke).toHaveBeenCalledWith('remove_game_mod', { id: 'game', fileName: 'example.jar' });
  wrapper.unmount();
});
