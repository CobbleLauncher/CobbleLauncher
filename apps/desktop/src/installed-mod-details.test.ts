// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { defineComponent } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameModsPanel from './GameModsPanel.vue';
import ModUpdates from './ModUpdates.vue';
import type { Game, LocalMod, ModVersion } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const game: Game = { id: 'game-a', name: 'Fixture', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16', directory: 'synthetic', status: 'installed', error: null, progress: null };
const local: LocalMod = { file_name: 'map.jar.disabled', name: 'Forest Map', version: '2', authors: ['Original author'], loader: 'fabric', enabled: false, description: 'Original fixture description' };
const older: ModVersion = { version_id: 'v1', name: 'Map 1', version_number: '1', version_type: 'release', date_published: '2026-09-01' };
const current: ModVersion = { ...older, version_id: 'v2', name: 'Map 2', version_number: '2' };
const Selector = defineComponent({ props: { modelValue: { type: String, required: true }, options: { type: Array, required: true }, disabled: Boolean }, emits: ['update:modelValue'], template: '<select :value="modelValue" :disabled="disabled" @change="$emit(\'update:modelValue\', $event.target.value)"><option v-for="option in options" :key="option.value" :value="option.value">{{option.label}}</option></select>' });
function button(wrapper: ReturnType<typeof mount>, text: string) { return wrapper.findAll('button').find(value => value.text() === text)!; }
function mockCommands() {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'list_game_mods') return [local];
    if (command === 'list_managed_game_mods') return [{ project_id: 'forest-map', version_id: 'v2', filename: 'map.jar', enabled: false, name: 'Forest Map' }];
    if (command === 'list_game_mod_versions') return [current, older];
    if (command === 'preview_game_mod_version_changes') return { plan_id: 'reviewed-plan', changes: [
      { project_id: 'forest-map', name: 'Forest Map', from_version: '2', to_version: '1', filename: 'map.jar', enabled: false },
      { project_id: 'api', name: 'Required API', from_version: null, to_version: '3', filename: 'api.jar', enabled: true },
    ] };
    if (command === 'apply_game_mod_updates') return { updated: 1, added: 1 };
    return null;
  });
}

it('opens an installed disabled mod directly, reviews a downgrade and dependencies before explicit apply', async () => {
  setLocale('en-US'); mockCommands();
  const wrapper = mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { VersionSelector: Selector, CurseForgeUpdates: true } } });
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'list_game_mod_versions')).toBe(false);
  await wrapper.get('button[aria-label="Details for Forest Map"]').trigger('click');
  expect(wrapper.get('.installed-mod-details').text()).toContain('Original author');
  expect(wrapper.get('.installed-mod-details').text()).toContain('map.jar.disabled');
  await button(wrapper, 'Versions and dependencies').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_mod_versions', { id: 'game-a', projectId: 'forest-map' });
  expect((wrapper.get('select').element as HTMLSelectElement).value).toBe('v2');
  expect(button(wrapper, 'Review version changes').attributes('disabled')).toBeDefined();
  await wrapper.get('select').setValue('v1');
  await button(wrapper, 'Review version changes').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_game_mod_version_changes', { id: 'game-a', selections: [{ project_id: 'forest-map', version_id: 'v1' }] });
  expect(wrapper.get('[aria-label="Version changes"]').text()).toContain('Required API');
  expect(wrapper.get('[aria-label="Version changes"]').text()).toContain('Keep disabled');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'apply_game_mod_updates')).toBe(false);
  await button(wrapper, 'Confirm changes').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('apply_game_mod_updates', { id: 'game-a', planId: 'reviewed-plan' });
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({ busy: false });
  wrapper.unmount();
});

it('does not turn an unrecognized local file into an arbitrary provider project', async () => {
  setLocale('en-US'); mockCommands();
  vi.mocked(invoke).mockImplementation(async command => ['list_managed_game_mods', 'list_game_curseforge_mods'].includes(command) ? [] : null);
  const wrapper = mount(ModUpdates, { props: { game, locked: true }, global: { stubs: { CurseForgeUpdates: true } } });
  await wrapper.vm.openInstalledVersion(local.file_name); await flushPromises();
  expect(wrapper.text()).toContain('Recognize this local file');
  expect(button(wrapper, 'Recognize local mods').attributes('disabled')).toBeDefined();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'list_game_mod_versions')).toBe(false);
  wrapper.unmount();
});

it('discards an old installed-file lookup after changing the target game', async () => {
  setLocale('en-US'); let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'list_managed_game_mods' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(null));
  const wrapper = mount(ModUpdates, { props: { game, locked: false }, global: { stubs: { CurseForgeUpdates: true } } });
  const pending = wrapper.vm.openInstalledVersion(local.file_name);
  await wrapper.setProps({ game: { ...game, id: 'game-b' } });
  finish([{ project_id: 'stale', version_id: 'old', filename: 'map.jar', name: 'Stale', enabled: false }]);
  await pending; await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'list_game_mod_versions')).toBe(false);
  expect(wrapper.find('.managed-mod-versions').isVisible()).toBe(false);
  wrapper.unmount();
});

it('keeps a CurseForge-managed file in its existing provider workflow', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_managed_game_mods') return [];
    if (command === 'list_game_curseforge_mods') return [{ filename: 'map.jar', project_id: '123', version_id: '456', enabled: false }];
    return null;
  });
  const wrapper = mount(ModUpdates, { props: { game, locked: false }, global: { stubs: { CurseForgeUpdates: true } } });
  await wrapper.vm.openInstalledVersion(local.file_name); await flushPromises();
  expect(wrapper.text()).toContain('This file is managed by CurseForge');
  expect(button(wrapper, 'Check CurseForge updates').exists()).toBe(true);
  expect(wrapper.findAll('button').some(value => value.text() === 'Recognize local mods')).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'list_game_mod_versions')).toBe(false);
  wrapper.unmount();
});
