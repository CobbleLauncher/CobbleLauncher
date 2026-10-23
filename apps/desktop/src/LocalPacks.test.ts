// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import LocalPacks from './LocalPacks.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn()}));
const pack = {file_name: 'texture.zip', name: 'Texture', description: 'Original fixture', size_bytes: 200};
beforeEach(() => {setLocale('en-US'); vi.mocked(invoke).mockImplementation(async command => command === 'list_game_packs' ? [pack] : null);});
afterEach(() => {vi.mocked(invoke).mockReset(); setLocale('zh-CN');});
it('uses an installed-only inventory without nested discovery and preserves picker cancellation and locks', async () => {
  const wrapper = mount(LocalPacks, {props: {gameId: 'game', kind: 'resource_pack', locked: false, installedOnly: true}}); await flushPromises();
  expect(wrapper.find('[role=tablist]').exists()).toBe(false); expect(wrapper.text()).not.toContain('Discover');
  const importer = wrapper.findAll('button').find(button => button.text() === 'Import resource packs')!;
  await importer.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('import_game_packs', {id: 'game', kind: 'resource_pack'}); expect(wrapper.text()).toContain('Texture');
  await wrapper.setProps({locked: true}); expect(importer.attributes('disabled')).toBeDefined(); expect(wrapper.get('[aria-label="Remove Texture"]').attributes('disabled')).toBeDefined(); wrapper.unmount();
});
it('keeps local ZIP removal behind explicit confirmation and sends the original identifiers', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_packs' ? [pack] : []);
  const wrapper = mount(LocalPacks, {props: {gameId: 'game', kind: 'resource_pack', locked: false, installedOnly: true}}); await flushPromises();
  await wrapper.get('[aria-label="Remove Texture"]').trigger('click');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'remove_game_pack')).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Confirm removal')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('remove_game_pack', {id: 'game', kind: 'resource_pack', fileName: 'texture.zip'}); expect(wrapper.emitted('busy')).toEqual([[true], [false]]); wrapper.unmount();
});
