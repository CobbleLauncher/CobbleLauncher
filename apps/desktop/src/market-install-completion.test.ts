// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import MarketPage from './MarketPage.vue';
import DiscoverMods from './DiscoverMods.vue';
import DiscoverPacks from './DiscoverPacks.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import CurseForgeInstallPlan from './CurseForgeInstallPlan.vue';
import { beginMarketInstall, endMarketInstall, modInstallResult, type MarketInstallResult } from './market-install-result';
import { rememberMarketWorkstate } from './market-workstate';
import { setLocale } from './i18n';
import type { Game, ManagedGameMod } from './games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const game: Game = { id: 'completion-game', name: 'My Fabric game', version: '1.21.1', directory: 'test', status: 'installed', error: null, progress: null, loader_kind: 'fabric', loader_version: '0.16' };
const hit = { project_id: 'one', slug: 'one', title: 'One', description: '', author: 'Author', downloads: 1 };
const version = { version_id: 'v2', name: 'Version 2', version_number: '2', version_type: 'release', date_published: '2026-10-01' };
const record: ManagedGameMod = {project_id: 'one', version_id: 'v2', filename: 'one.jar', enabled: true, pinned: false};
const modPlan = {plan_id: 'plan', project_id: 'one', version_id: 'v2', mods: [{project_id: 'one', version_id: 'v2', name: 'One', version_number: '2', required_by: null}]};
const result: MarketInstallResult = {gameId: game.id, kind: 'mod', provider: 'modrinth', projectIds: ['one'], versionIds: ['v2'], contentIds: ['one.jar']};
const picker = {emits: ['selected'], template: '<button data-test="choose-version" @click="$emit(\'selected\', {version_id: \'v2\', name: \'Version 2\'})">Choose version</button>'};
const stubs = {DiscoverMods: true, CurseForgeDiscover: true, DiscoverPacks: true, DiscoverPacksPage: true};
beforeEach(() => { vi.resetAllMocks(); setLocale('en-US'); localStorage.clear(); rememberMarketWorkstate('market:selection', undefined); rememberMarketWorkstate(`modrinth:mod:${game.id}`, undefined); rememberMarketWorkstate(`modrinth:resource_pack:${game.id}`, undefined); endMarketInstall(game.id); });
afterEach(() => { setLocale('zh-CN'); endMarketInstall(game.id); vi.useRealTimers(); });

it('resolves exact provider versions to canonical filenames without borrowing another version', async () => {
  vi.mocked(invoke).mockResolvedValue([{...record, filename: 'one.jar.disabled'}, {...record, project_id: 'other', filename: 'other.jar'}]);
  expect(await modInstallResult(game.id, 'modrinth', modPlan.mods)).toEqual(result);
  vi.mocked(invoke).mockRejectedValueOnce(new Error('read unavailable'));
  expect(await modInstallResult(game.id, 'curseforge', modPlan.mods)).toMatchObject({provider: 'curseforge', contentIds: [], projectIds: ['one']});
});

it('keeps both source mutation locks until each source finishes', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(MarketPage, {props: {games: [game], initialKind: 'mod', initialGameId: game.id}, global: {stubs}});
  wrapper.findComponent(DiscoverMods).vm.$emit('installing', true);
  wrapper.findComponent(CurseForgeDiscover).vm.$emit('busy', true);
  wrapper.findComponent(DiscoverMods).vm.$emit('installing', false);
  await flushPromises();
  expect(wrapper.emitted('busy')?.at(-1)).toEqual([true]);
  wrapper.findComponent(CurseForgeDiscover).vm.$emit('busy', false);
  expect(wrapper.emitted('busy')?.at(-1)).toEqual([false]);
  wrapper.unmount();
});

it('keeps installed IDs and target context on the explicit View content action', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(MarketPage, {props: {games: [game], initialKind: 'mod', initialGameId: game.id}, global: {stubs}});
  wrapper.findComponent(DiscoverMods).vm.$emit('installed', result);
  await flushPromises();
  expect(wrapper.emitted('installed')?.[0]).toEqual([game.id, result]);
  expect(wrapper.text()).toContain(game.name);
  expect(wrapper.emitted('locate')).toBeUndefined();
  await wrapper.get('.market-workflow > [role="status"] button').trigger('click');
  expect(wrapper.emitted('locate')?.[0]).toEqual([game.id, 'mod', result]);
  wrapper.unmount();
});

it('reads local provenance once per target instead of once per result card', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_managed_game_mods' ? [record] : []);
  const wrapper = mount(MarketPage, {props: {games: [game], initialKind: 'mod', initialGameId: game.id}, global: {stubs}});
  await flushPromises();
  expect(wrapper.findComponent(DiscoverMods).props('installedRecords')).toEqual([record]);
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'list_managed_game_mods')).toHaveLength(1);
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'list_game_curseforge_mods')).toHaveLength(1);
  wrapper.unmount();
});

it('prevents reinstalling a recorded exact version and keeps the version picker usable', async () => {
  vi.mocked(invoke).mockResolvedValue({hits: [hit], offset: 0, total_hits: 1});
  const wrapper = mount(DiscoverMods, {props: {game, locked: false, installedRecords: [record]}, global: {stubs: {ModVersionPicker: picker}}});
  await flushPromises(); await wrapper.get('[data-market-project="one"]').trigger('click');
  await wrapper.get('[data-test="choose-version"]').trigger('click'); await flushPromises();
  expect(wrapper.find('.primary-button').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'preview_game_mod_install')).toBe(false);
  expect(wrapper.find('[data-test="choose-version"]').attributes('disabled')).toBeUndefined();
  wrapper.unmount();
});

it('keeps a failed Mod install on the selected version, retries once and hides reinstall after success', async () => {
  let installs = 0;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'search_game_mods') return {hits: [hit], offset: 0, total_hits: 1};
    if (command === 'preview_game_mod_install') return modPlan;
    if (command === 'list_managed_game_mods') return [record];
    if (command === 'install_game_mod_plan' && ++installs === 1) throw new Error('offline');
    return null;
  });
  const wrapper = mount(DiscoverMods, {props: {game, locked: false}, global: {stubs: {ModVersionPicker: picker}}});
  await flushPromises(); await wrapper.get('input').setValue('keep query');
  await wrapper.get('[data-market-project="one"]').trigger('click'); await wrapper.get('[data-test="choose-version"]').trigger('click'); await flushPromises();
  await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  expect(wrapper.emitted('installed')).toBeUndefined();
  await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(installs).toBe(2); expect(wrapper.emitted('installed')?.[0]).toEqual([result]);
  expect(wrapper.find('.primary-button').exists()).toBe(false);
  await wrapper.get('.market-back-button').trigger('click');
  expect((wrapper.get('input').element as HTMLInputElement).value).toBe('keep query');
  wrapper.unmount();
});

it('blocks a duplicate native install from another mounted workflow', async () => {
  vi.mocked(invoke).mockResolvedValue({...modPlan, ready: true, missing: []});
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: game.id, projectId: 'one', fileId: 'v2', locked: false}});
  await flushPromises(); expect(beginMarketInstall(game.id)).toBe(true);
  await wrapper.get('.primary-button').trigger('click');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'install_curseforge_mod_plan')).toBe(false);
  endMarketInstall(game.id); wrapper.unmount();
});

it('does not prepare or apply an already-installed CurseForge file', async () => {
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: game.id, projectId: 'one', fileId: 'v2', locked: false, installedRecords: [record]}});
  await flushPromises(); expect(invoke).not.toHaveBeenCalled(); expect(wrapper.find('.primary-button').exists()).toBe(false); wrapper.unmount();
});

it('only reveals the newly added resource-pack filename and suppresses a second apply', async () => {
  const existing = {file_name: 'old.zip', name: 'Old', description: null, size_bytes: 1, pack_format: 1};
  const added = {...existing, file_name: 'new.zip', name: 'One'};
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'search_game_content_packs') return {hits: [hit], offset: 0, total_hits: 1};
    if (command === 'list_game_content_pack_versions') return [version];
    if (command === 'list_game_packs') return [existing];
    if (command === 'install_game_content_pack') return [existing, added];
    return null;
  });
  const wrapper = mount(DiscoverPacks, {props: {gameId: game.id, kind: 'resource_pack', locked: false, browseOnMount: true}});
  await flushPromises(); await wrapper.get('[data-market-project="one"]').trigger('click'); await flushPromises();
  await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(wrapper.emitted('installed')?.[0]).toEqual([[existing, added], {...result, kind: 'resource_pack', contentIds: ['new.zip']}]);
  expect(wrapper.find('.primary-button').exists()).toBe(false); wrapper.unmount();
});

it('retries CurseForge with the submitted query and filters when draft inputs changed', async () => {
  let searches = 0;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_curseforge_availability') return {available: true};
    if (command === 'search_curseforge' && ++searches === 2) throw new Error('offline');
    return {hits: [hit], offset: 0, total_hits: 1};
  });
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'modpack'}}); await flushPromises();
  await wrapper.get('input[type="search"]').setValue('submitted'); await wrapper.get('#cf-minecraft').setValue('1.21.1'); await wrapper.get('#cf-loader').setValue('fabric'); await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.get('input[type="search"]').setValue('draft'); await wrapper.get('#cf-minecraft').setValue('1.20.1');
  await wrapper.get('[role="alert"] button').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'search_curseforge').at(-1)?.[1]).toMatchObject({query: 'submitted', minecraftVersion: '1.21.1', loader: 'fabric'});
  expect((wrapper.get('input[type="search"]').element as HTMLInputElement).value).toBe('draft'); wrapper.unmount();
});

it('never flashes visual loading or an empty result for a fast search', async () => {
  vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
  vi.mocked(invoke).mockResolvedValue({hits: [hit], offset: 0, total_hits: 1});
  const wrapper = mount(DiscoverMods, {props: {game, locked: false}, global: {stubs: {ModVersionPicker: true}}});
  expect(wrapper.attributes('aria-busy')).toBe('true');
  expect(wrapper.find('.content-empty').exists()).toBe(false);
  await flushPromises(); await vi.advanceTimersByTimeAsync(200);
  expect(wrapper.find('.loading-indicator, .loading-skeleton').exists()).toBe(false);
  expect(wrapper.attributes('aria-busy')).toBe('false');
  expect(wrapper.find('[data-market-project="one"]').exists()).toBe(true);
  wrapper.unmount();
});
