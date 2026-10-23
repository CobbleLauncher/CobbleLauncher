// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameModsPanel from './GameModsPanel.vue';
import type { Game, LocalMod } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const game: Game = { id: 'filter-game', name: 'Fixture', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16', directory: 'synthetic', status: 'installed', error: null, progress: null };
const mods: LocalMod[] = [
  { file_name: 'alpha.jar', name: 'Alpha', version: '1', authors: ['Author'], loader: 'fabric', enabled: true, description: '' },
  { file_name: 'beta.jar.disabled', name: 'Beta', version: '2', authors: ['Author'], loader: 'fabric', enabled: false, description: '' },
  { file_name: 'local.jar', name: 'Local', version: '3', authors: [], loader: null, enabled: true, description: '' },
];
function mountPanel() { setLocale('en-US'); return mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { ModUpdates: true } } }); }
function mockSources() {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_game_mods') return structuredClone(mods);
    if (command === 'list_managed_game_mods') return [{ filename: 'alpha.jar' }];
    if (command === 'list_game_curseforge_mods') return [{ filename: 'beta.jar' }];
    return null;
  });
}
function button(wrapper: ReturnType<typeof mountPanel>, label: string) { return wrapper.findAll('button').find(value => value.text() === label)!; }

it('combines state, lazy provenance and search filters with honest visible counts', async () => {
  mockSources(); const wrapper = mountPanel(); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(['list_game_mods']);
  expect(wrapper.get('select[aria-label="Filter Mod state"]').text()).toContain('Enabled (2)');
  await wrapper.get('select[aria-label="Filter Mod state"]').setValue('disabled');
  expect(wrapper.get('.mod-list').text()).toContain('Beta');
  expect(wrapper.get('.mods-count').text()).toBe('1 of 3 installed');
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('curseforge'); await flushPromises();
  expect(wrapper.get('.mod-list').text()).toContain('Beta');
  expect(invoke).toHaveBeenCalledWith('list_game_curseforge_mods', { id: game.id });
  await wrapper.get('input[type="search"]').setValue('not found');
  expect(wrapper.get('.mods-count').text()).toBe('0 of 3 installed');
  expect(wrapper.text()).toContain('No matching Mods.');
  wrapper.unmount();
});

it('clears old selections when changing filters and local files exclude recorded providers', async () => {
  mockSources(); const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true);
  expect(wrapper.text()).toContain('3 selected');
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('local'); await flushPromises();
  expect(wrapper.get('.mod-list').text()).toContain('Local');
  expect(wrapper.get('.mod-list').text()).not.toContain('Alpha');
  expect(wrapper.get('.mod-selection-label').text()).toBe('Select Mods');
  wrapper.unmount();
});

it('does not mislabel a failed provenance read as local files', async () => {
  mockSources(); let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => {
    if (command === 'list_game_mods') return Promise.resolve(structuredClone(mods));
    if (command === 'list_managed_game_mods') return new Promise(resolve => { finish = resolve; });
    return Promise.reject(new Error('synthetic read failure'));
  });
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('local'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Could not read Mod source records.');
  expect(wrapper.find('.mod-list').exists()).toBe(false);
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('all');
  finish([{filename:'local.jar'}]); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(wrapper.findAll('.mod-row')).toHaveLength(3);
  wrapper.unmount();
});

it('ignores a successful source lookup after switching back to all sources', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => {
    if (command === 'list_game_mods') return Promise.resolve(structuredClone(mods));
    if (command === 'list_managed_game_mods') return new Promise(resolve => { finish = resolve; });
    return Promise.resolve([]);
  });
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('modrinth');
  expect(wrapper.text()).toContain('Reading Mod sources…');
  await wrapper.get('select[aria-label="Filter Mod source"]').setValue('all');
  finish([{filename:'alpha.jar'}]); await flushPromises();
  expect(wrapper.findAll('.mod-row')).toHaveLength(3);
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  expect(wrapper.get('.mods-count').text()).toBe('3 installed');
  wrapper.unmount();
});

it('stops the sequential bulk operation before dispatching another file when the game becomes locked', async () => {
  mockSources(); let finish!: () => void;
  vi.mocked(invoke).mockImplementation(command => {
    if (command === 'list_game_mods') return Promise.resolve(structuredClone(mods));
    return new Promise(resolve => { finish = () => resolve(null); });
  });
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true);
  await button(wrapper, 'Disable selected').trigger('click');
  await wrapper.setProps({ locked: true }); finish(); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_game_mod_enabled')).toHaveLength(1);
  expect(wrapper.get('[role="alert"]').text()).toContain('Close the game');
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({ busy: false });
  wrapper.unmount();
});

it('never dispatches the old batch into another game after an in-flight mutation', async () => {
  mockSources(); let finish!: () => void;
  vi.mocked(invoke).mockImplementation(command => command === 'list_game_mods' ? Promise.resolve(structuredClone(mods)) : new Promise(resolve => { finish = () => resolve(null); }));
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true);
  await button(wrapper, 'Disable selected').trigger('click');
  await wrapper.setProps({ game: { ...game, id: 'other-game' } }); await flushPromises();
  finish(); await flushPromises();
  const mutations = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_game_mod_enabled');
  expect(mutations).toHaveLength(1);
  expect(mutations[0]?.[1]).toMatchObject({ id: game.id, fileName: 'alpha.jar' });
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  wrapper.unmount();
});

it('releases its busy state when a refresh invalidates an in-flight batch', async () => {
  let finish!: () => void;
  vi.mocked(invoke).mockImplementation(command => command === 'list_game_mods' ? Promise.resolve(structuredClone(mods)) : new Promise(resolve => { finish = () => resolve(null); }));
  const wrapper = mountPanel(); await flushPromises();
  await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true);
  await button(wrapper, 'Disable selected').trigger('click');
  await wrapper.vm.refreshContent(); finish(); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_game_mod_enabled')).toHaveLength(1);
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({ busy: false });
  wrapper.unmount();
});
