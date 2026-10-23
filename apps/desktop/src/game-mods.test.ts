// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import GameModsPanel from './GameModsPanel.vue';
import GameDetailsPage from './GameDetailsPage.vue';
import type { Game, LocalMod } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const game: Game = { id: 'owned-game', name: 'Game', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null };
const initial: LocalMod[] = [
  { file_name: 'alpha.jar', name: 'Alpha', version: '1', authors: ['A'], loader: 'fabric', enabled: true, description: '' },
  { file_name: 'beta.jar.disabled', name: 'Beta', version: '1', authors: ['B'], loader: 'fabric', enabled: false, description: '' },
];
function mountMods() { setLocale('en-US'); return mount(GameModsPanel, { props: { game, locked: false, revision: 3 }, global: { stubs: { DiscoverMods: true, ModUpdates: true } } }); }
function button(wrapper: ReturnType<typeof mount>, label: string) { return wrapper.findAll('button').find(value => value.text() === label)!; }
it('keeps filtering and selected batch mutations scoped to visible Mods and reports busy until refresh completes', async () => {
  let mods = structuredClone(initial); let finish!: () => void;
  vi.mocked(invoke).mockImplementation((command, rawArgs) => {
    const args = rawArgs as Record<string, unknown>;
    if (command === 'list_game_mods') return Promise.resolve(structuredClone(mods));
    if (command === 'set_game_mod_enabled') return new Promise(resolve => { finish = () => { mods = mods.map(mod => mod.file_name === args.fileName ? { ...mod, enabled: false } : mod); resolve(mods); }; });
    return Promise.resolve(null);
  });
  const wrapper = mountMods(); await flushPromises();
  await wrapper.get('input[type="search"]').setValue('Alpha'); await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true);
  await button(wrapper, 'Disable selected').trigger('click');
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({ busy: true });
  expect(invoke).toHaveBeenCalledWith('set_game_mod_enabled', { id: game.id, fileName: 'alpha.jar', enabled: false });
  expect(wrapper.get('input[type="checkbox"]').attributes('disabled')).toBeDefined();
  finish(); await flushPromises();
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({ busy: false });
  expect(wrapper.get('[role="switch"]').attributes('aria-checked')).toBe('false'); wrapper.unmount();
});
it('requires batch removal confirmation and preserves the unprocessed selection after a partial native failure', async () => {
  let mods = structuredClone(initial);
  vi.mocked(invoke).mockImplementation(async (command, rawArgs) => {
    const args = rawArgs as Record<string, unknown>;
    if (command === 'list_game_mods') return structuredClone(mods);
    if (command === 'remove_game_mod') { if (args.fileName === 'beta.jar.disabled') throw new Error('Synthetic failure'); mods = mods.filter(mod => mod.file_name !== args.fileName); }
    return mods;
  });
  const wrapper = mountMods(); await flushPromises();
  await wrapper.get('input[aria-label="Select all visible Mods"]').setValue(true); await button(wrapper, 'Remove selected').trigger('click');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'remove_game_mod')).toBe(false);
  await button(wrapper, 'Confirm removal').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Could not process Beta: Error: Synthetic failure');
  expect(wrapper.findAll('.mod-row')).toHaveLength(1); expect(wrapper.get('.mod-row').text()).toContain('Beta');
  expect(wrapper.get('.mod-selection-label').text()).toContain('1 selected'); wrapper.unmount();
});
it('opens updates as a separate view and preserves activity and refresh contracts', async () => {
  vi.mocked(invoke).mockResolvedValue(initial);
  const wrapper = mountMods(); await flushPromises();
  const updates = wrapper.findComponent({name: 'ModUpdates'});
  expect(updates.props()).toMatchObject({game, revision: 3});
  expect(wrapper.find('[role=tablist]').exists()).toBe(false);
  expect(wrapper.findComponent({name: 'DiscoverMods'}).exists()).toBe(false);
  await button(wrapper, 'Updates').trigger('click'); expect(updates.isVisible()).toBe(true);
  updates.vm.$emit('recognizing', true); updates.vm.$emit('busy', true); await flushPromises();
  expect(wrapper.emitted('activity')?.at(-1)?.[0]).toMatchObject({busy: true, updating: true, recognizing: true});
  updates.vm.$emit('busy', false); updates.vm.$emit('recognizing', false); await flushPromises();
  await wrapper.vm.refreshContent(); wrapper.vm.showInstalled(); await flushPromises();
  expect(wrapper.find('.mods-view-bar').exists()).toBe(false); expect(wrapper.get('input[type=search]').isVisible()).toBe(true); wrapper.unmount();
});
it('ignores the old game response after switching instances', async () => {
  let finish!: (value: LocalMod[]) => void;
  vi.mocked(invoke).mockImplementation((_command, rawArgs) => (rawArgs as { id: string }).id === game.id ? new Promise(resolve => { finish = resolve; }) : Promise.resolve([initial[1]]));
  const wrapper = mountMods(); await wrapper.setProps({ game: { ...game, id: 'second-game' } }); await flushPromises();
  finish([initial[0]!]); await flushPromises();
  expect(wrapper.get('.mod-list').text()).toContain('Beta'); expect(wrapper.get('.mod-list').text()).not.toContain('Alpha'); wrapper.unmount();
});
it('propagates Mods installation activity into instance launch and navigation locks', async () => {
  setLocale('en-US');
  const wrapper = mount(GameDetailsPage, { props: { game, starting: false, profile: null, library: { profiles: [], active_id: null } }, global: { stubs: { GameModsPanel: true, ExtensionMenuItems: true, GameDiagnosticsPanel: true } } });
  const mods = wrapper.findComponent({ name: 'GameModsPanel' });
  mods.vm.$emit('activity', { busy: true, installing: true, updating: false, recognizing: false }); await flushPromises();
  expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined(); expect(wrapper.get('.play-button').text()).toBe('Installing Mods…');
  expect(wrapper.emitted('contentBusy')?.at(-1)).toEqual([true]); expect(wrapper.vm.canLeave()).toBe(false);
  mods.vm.$emit('activity', { busy: false, installing: false, updating: false, recognizing: false }); await flushPromises();
  expect(wrapper.emitted('contentBusy')?.at(-1)).toEqual([false]); wrapper.unmount();
});
it('shows Vanilla resource packs by default without Mod controls or requests', async () => {
  setLocale('en-US'); vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(GameDetailsPage, {props: {game: {...game, loader_kind: null, loader_version: null}, starting: false, profile: null, library: {profiles: [], active_id: null}}, global: {stubs: {GameModsPanel: true, LocalPacks: true, ExtensionMenuItems: true, GameDiagnosticsPanel: true}}});
  await flushPromises(); expect(wrapper.findComponent({name: 'GameModsPanel'}).exists()).toBe(false);
  expect(wrapper.find('#game-content-type').exists()).toBe(false); expect(wrapper.get('.details-content-title').text()).toBe('Resource packs');
  expect(wrapper.findComponent({name: 'LocalPacks'}).props('installedOnly')).toBe(true);
  expect(wrapper.findComponent({name: 'LocalPacks'}).props('kind')).toBe('resource_pack');
  expect(wrapper.text()).not.toContain('Add mods'); expect(wrapper.text()).not.toContain('Updates');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'list_game_mods')).toBe(false);
  expect(wrapper.get('.details-title h1').text()).toBe(game.name);
  expect(wrapper.get('.details-title .details-rename').attributes('aria-label')).toBe('Rename');
  await button(wrapper, 'Add resource packs').trigger('click'); expect(wrapper.emitted('market')?.[0]).toEqual(['resource_pack']); wrapper.unmount();
});
it('keeps Mesh available while running but locks launch and content writes', async () => {
  setLocale('en-US');
  const wrapper = mount(GameDetailsPage, {props: {game: {...game, launch_state: 'running'}, starting: false, profile: null, library: {profiles: [], active_id: null}}, global: {stubs: {GameModsPanel: true, LocalPacks: true, ExtensionMenuItems: true, GameDiagnosticsPanel: true}}});
  expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.details-content-toolbar button').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.details-mesh').attributes('disabled')).toBeUndefined();
  await wrapper.get('.details-mesh').trigger('click'); expect(wrapper.emitted('mesh')).toHaveLength(1); wrapper.unmount();
});
it('renders no standalone Vanilla Mods panel and does not enumerate Mods', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(GameModsPanel, {props: {game: {...game, loader_kind: null, loader_version: null}, locked: false}, global: {stubs: {ModUpdates: true}}}); await flushPromises();
  expect(wrapper.find('.game-mods-panel').exists()).toBe(false); expect(invoke).not.toHaveBeenCalled(); wrapper.unmount();
});
it('preserves runtime dirty guards when moving to worlds or content', async () => {
  setLocale('en-US'); const confirmLeave = vi.fn().mockResolvedValue(false);
  const Runtime = defineComponent({setup(_props, {expose}) {expose({canLeave: () => false, confirmLeave}); return () => h('div', 'Dirty runtime');}});
  const wrapper = mount(GameDetailsPage, {props: {game, starting: false, profile: null, library: {profiles: [], active_id: null}}, global: {stubs: {GameModsPanel: true, LocalPacks: true, ExtensionMenuItems: true, GameDiagnosticsPanel: true, GameRuntimeSettings: Runtime, GameAppearancePanel: true, InstanceExtensionSettings: true, GameWorlds: true}}});
  await wrapper.get('#game-runtime-tab').trigger('click'); await flushPromises();
  await wrapper.get('#game-worlds-tab').trigger('click'); await flushPromises();
  expect(confirmLeave).toHaveBeenCalledOnce(); expect(wrapper.get('#game-runtime-tab').attributes('aria-selected')).toBe('true');
  expect(wrapper.vm.canLeave()).toBe(false); expect(wrapper.findComponent({name: 'GameWorlds'}).exists()).toBe(false); wrapper.unmount();
});
