// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CfPackInstallPage from './CfPackInstallPage.vue';
import CurseForgePackPreparation from './CurseForgePackPreparation.vue';
import ModUpdates from './ModUpdates.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import CurseForgeInstallPlan from './CurseForgeInstallPlan.vue';
import { setLocale } from './i18n';
import { rememberMarketWorkstate } from './market-workstate';
import type { CurseForgePlan, CurseForgePackPreview } from './curseforge';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn(), isTauri: () => true}));
const hit = {project_id: '42', title: 'Test mod', author: 'Author', description: 'Description', slug: 'test', downloads: 10, website_url: null, allow_distribution: true, available: true};
const file = {project_id: '42', file_id: '84', name: 'Release', file_name: 'test.jar', release_type: 'release', date_published: '2026-10-22', size_bytes: 100, game_versions: ['1.21.1'], dependencies: [], download_availability: 'available'};
const plan: CurseForgePlan = {plan_id: 'plan', project_id: '42', version_id: '84', mods: [{project_id: '42', version_id: '84', name: 'Test mod', version_number: 'Release', required_by: null}], missing: [{project_id: '43', file_id: '85', name: 'Dependency', file_name: 'dependency.jar', size_bytes: 20, website_url: null, reason: 'manual'}], ready: false};
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); rememberMarketWorkstate('curseforge:modpack:global', undefined); rememberMarketWorkstate('curseforge:mod:game', undefined); });
afterEach(() => { setLocale('zh-CN'); });
it('searches compatible mods and loads deduplicated pages using backend offsets', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_curseforge_availability' ? {available: true} : command === 'search_curseforge' ? {hits: [hit], offset: 0, total_hits: 80} : {files: [file], offset: 0, total_hits: 1});
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'mod', game: {id: 'game', name: 'Target', version: '1.21.1', directory: 'private', status: 'installed', loader_kind: 'fabric', loader_version: '0.16', error: null, progress: null}}, global: {stubs: {CurseForgeInstallPlan: true}}});
  await flushPromises(); await wrapper.get('input[type=search]').setValue('test'); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('search_curseforge', {query: 'test', kind: 'mod', minecraftVersion: '1.21.1', loader: 'fabric', offset: 0});
  await wrapper.findAll('button').find(button => button.text() === 'Load more')!.trigger('click'); await flushPromises();
  expect(wrapper.findAll('.mod-row')).toHaveLength(1);
  await wrapper.get('button[data-market-project]').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_curseforge_files', {projectId: '42', minecraftVersion: '1.21.1', loader: 'fabric', offset: 0});
  expect(wrapper.text()).toContain('test.jar'); expect(wrapper.emitted('installed')).toBeUndefined(); wrapper.unmount();
});
it('requires Minecraft before a loader filter and reports unavailable builds without querying', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_curseforge_availability' ? {available: true} : {hits: [], offset: 0, total_hits: 0});
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'modpack'}}); await flushPromises();
  await wrapper.get('#cf-loader').setValue('forge'); await wrapper.get('form').trigger('submit');
  expect(wrapper.get('[role=alert]').text()).toContain('Choose a Minecraft version');
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'search_curseforge')).toHaveLength(1); wrapper.unmount();
  vi.mocked(invoke).mockResolvedValue({available: false});
  const unavailable = mount(CurseForgeDiscover, {props: {kind: 'modpack'}}); await flushPromises();
  expect(unavailable.text()).toContain('unavailable in this build'); expect(unavailable.find('form').exists()).toBe(false); unavailable.unmount();
});
it('keeps install disabled until manual dependencies are verified and only then emits installed', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'preview_curseforge_mod_install' ? plan : command === 'select_curseforge_manual_file' ? {...plan, ready: true, missing: []} : {installed: 2, reused: 0});
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: 'game', projectId: '42', fileId: '84', locked: false}}); await flushPromises();
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.cf-missing button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('select_curseforge_manual_file', {id: 'game', planId: 'plan', projectId: '43', fileId: '85'});
  expect(wrapper.emitted('installed')).toBeUndefined(); await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('install_curseforge_mod_plan', {id: 'game', planId: 'plan'}); expect(wrapper.emitted('installed')).toHaveLength(1); wrapper.unmount();
});
it('does not mark failed or cancelled manual completion as installed and respects running locks', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'preview_curseforge_mod_install' ? plan : null);
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: 'game', projectId: '42', fileId: '84', locked: false}}); await flushPromises();
  await wrapper.get('.cf-missing button').trigger('click'); await flushPromises(); expect(wrapper.find('.cf-missing').exists()).toBe(true); expect(wrapper.emitted('installed')).toBeUndefined();
  await wrapper.setProps({locked: true}); expect(wrapper.get('.cf-missing button').attributes('disabled')).toBeDefined(); wrapper.unmount();
});
it('keeps a failed install retryable and never emits a completion event', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'preview_curseforge_mod_install') return {...plan, ready: true, missing: []};
    throw new Error('synthetic failure');
  });
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: 'game', projectId: '42', fileId: '84', locked: false}}); await flushPromises();
  await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('Installation did not complete');
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeUndefined();
  expect(wrapper.emitted('installed')).toBeUndefined(); wrapper.unmount();
});
it('explains a rejected manual file without exposing the native error', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'preview_curseforge_mod_install') return plan;
    throw 'manual_file_mismatch';
  });
  const wrapper = mount(CurseForgeInstallPlan, {props: {gameId: 'game', projectId: '42', fileId: '84', locked: false}}); await flushPromises();
  await wrapper.get('.cf-missing button').trigger('click'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('does not match the selected version');
  expect(wrapper.get('[role=alert]').text()).not.toContain('manual_file_mismatch');
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined(); wrapper.unmount();
});

it('checks both providers from one action and keeps CurseForge version IDs in its own plan', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'check_game_mod_updates') return {updates: [], managed_count: 0};
    if (command === 'list_game_curseforge_mods') return [{project_id: '42', version_id: '83', filename: 'old.jar', enabled: false}];
    if (command === 'check_game_curseforge_updates') return [{project_id: '42', name: 'Test mod', current_version: 'Old', version_number: 'New', version_id: '84', filename: 'test.jar', enabled: false}];
    if (command === 'preview_game_curseforge_updates') return {plan_id: 'update-plan', changes: [{project_id: '42', name: 'Test mod', from_version: 'Old', to_version: 'New', filename: 'test.jar', enabled: false}], ready: true, missing: []};
    return {installed: 1, reused: 0};
  });
  const wrapper = mount(ModUpdates, {props: {game: {id: 'game', name: 'Target', version: '1.21.1', directory: 'private', status: 'installed', loader_kind: 'fabric', loader_version: '0.16', error: null, progress: null}, locked: false}});
  await wrapper.findAll('button').find(button => button.text() === 'Check updates')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('check_game_mod_updates', {id: 'game'});
  expect(invoke).toHaveBeenCalledWith('check_game_curseforge_updates', {id: 'game'});
  expect(wrapper.text()).not.toContain('No updates found');
  await wrapper.findAll('button').find(button => button.text() === 'Update')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_game_curseforge_updates', {id: 'game', projectIds: ['42']});
  await wrapper.get('.cf-updates .primary-button').trigger('click'); await flushPromises();
  expect(wrapper.emitted('updated')).toHaveLength(1);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'preview_game_mod_updates')).toBe(false);
  expect(wrapper.findAll('button').find(button => button.text() === 'Check again')!.attributes('disabled')).toBeUndefined(); wrapper.unmount();
});
const pack: CurseForgePackPreview = {preview_id: 'pack', name: 'My pack', version_id: '1.0', minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16', required_files: 1, optional_files: [{id: '43:85', project_id: '43', file_id: '85'}], selected_optional_files: [], overrides: 2, plan_id: null, files: [], missing: [], ready: false};
it('prepares selected optional pack files and only continues after required manual files match', async () => {
  const prepared = {...pack, plan_id: 'plan', selected_optional_files: ['43:85'], files: [{project_id: '42', file_id: '84', name: 'File', file_name: 'test.jar', destination: 'mods/test.jar', size_bytes: 100, required_by: null}], missing: plan.missing};
  vi.mocked(invoke).mockImplementation(async command => command === 'prepare_curseforge_pack' ? prepared : {...prepared, missing: [], ready: true});
  const wrapper = mount(CurseForgePackPreparation, {props: {preview: pack}});
  await wrapper.get('input[type=checkbox]').setValue(true); await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('prepare_curseforge_pack', {previewId: 'pack', optionalFiles: ['43:85']});
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined(); expect(wrapper.emitted('selected')).toBeUndefined();
  await wrapper.get('.cf-missing .secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('select_curseforge_pack_manual_file', {previewId: 'pack', planId: 'plan', projectId: '43', fileId: '85'});
  await wrapper.get('.primary-button').trigger('click'); expect(wrapper.emitted('selected')?.[0]?.[0]).toMatchObject({ready: true, plan_id: 'plan'}); wrapper.unmount();
});
it('creates a ready CurseForge pack with saved source preferences and a native-authorized directory', async () => {
  const ready = {...pack, plan_id: 'plan', ready: true};
  const game = {id: 'new', name: 'New game', status: 'installing'};
  vi.mocked(invoke).mockImplementation(async command => command === 'get_download_settings' ? {source: 'official', mirror: ''} : command === 'pick_game_directory' ? 'D:/Games' : game);
  const wrapper = mount(CfPackInstallPage, {props: {initialPreview: ready}}); await flushPromises();
  await wrapper.get('#cf-game-name').setValue(' New game '); await wrapper.get('.folder-button').trigger('click'); await flushPromises();
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_curseforge_pack_game', {name: 'New game', previewId: 'pack', planId: 'plan', source: {source: 'official', mirror: ''}, useDefaultDirectory: false});
  expect(wrapper.emitted('created')?.[0]?.[0]).toEqual(game);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'create_mrpack_game')).toBe(false); wrapper.unmount();
});
it('pins an online pack selection to the selected official project and file', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_curseforge_availability' ? {available: true} : command === 'search_curseforge' ? {hits: [hit], offset: 0, total_hits: 1} : command === 'list_curseforge_files' ? {files: [file], offset: 0, total_hits: 1} : pack);
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'modpack'}}); await flushPromises(); await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.get('button[data-market-project]').trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Prepare this version')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_curseforge_online_pack', {projectId: '42', fileId: '84'}); expect(wrapper.emitted('selectedCfPack')).toBeUndefined();
  expect(wrapper.findComponent(CurseForgePackPreparation).exists()).toBe(true); wrapper.unmount();
});
it.each(['mod', 'pack'] as const)('accepts an exact locally supplied unavailable file in the %s plan without an automatic download', async kind => {
  const unavailable = {...plan.missing[0]!, reason: 'unavailable' as const};
  const modPlan = {...plan, missing: [unavailable]};
  const packPlan = {...pack, plan_id: 'plan', missing: [unavailable]};
  vi.mocked(invoke).mockImplementation(async command => command === 'preview_curseforge_mod_install' ? modPlan : command === 'select_curseforge_manual_file' ? {...modPlan, missing: [], ready: true} : {...packPlan, missing: [], ready: true});
  const wrapper = kind === 'mod' ? mount(CurseForgeInstallPlan, {props: {gameId: 'game', projectId: '42', fileId: '84', locked: false}}) : mount(CurseForgePackPreparation, {props: {preview: packPlan}});
  await flushPromises(); expect(wrapper.text()).toContain('Archived or unavailable');
  const button = wrapper.findAll('button').find(button => button.text() === 'Choose local file')!;
  await button.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith(kind === 'mod' ? 'select_curseforge_manual_file' : 'select_curseforge_pack_manual_file', {...(kind === 'mod' ? {id: 'game'} : {previewId: 'pack'}), planId: 'plan', projectId: unavailable.project_id, fileId: unavailable.file_id});
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeUndefined();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command.includes('download'))).toBe(false); wrapper.unmount();
});
it('routes an unavailable root pack to native verified local ZIP selection', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_curseforge_availability' ? {available: true} : command === 'search_curseforge' ? {hits: [hit], offset: 0, total_hits: 1} : command === 'list_curseforge_files' ? {files: [{...file, download_availability: 'unavailable'}], offset: 0, total_hits: 1} : pack);
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'modpack'}}); await flushPromises(); await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.get('button[data-market-project]').trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Choose local ZIP')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_curseforge_online_pack', {projectId: '42', fileId: '84'});
  expect(wrapper.emitted('selectedCfPack')).toBeUndefined(); wrapper.unmount();
});
