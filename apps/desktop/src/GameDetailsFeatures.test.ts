// SPDX-License-Identifier: MIT
/* eslint-disable vue/one-component-per-file -- test doubles exercise exposed leave guards */
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameDetailsPage from './GameDetailsPage.vue';
import GamePackUpdatePanel from './GamePackUpdatePanel.vue';
import GameScreenshots from './GameScreenshots.vue';
import type { Game } from './games';
import { setLocale } from './i18n';
import SelectField from './ui/SelectField.vue';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn()}));
const game: Game = {id: 'modded', name: 'Modded', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16', directory: 'private', status: 'installed', progress: null, error: null};
const profile = {id: 'player', name: 'Player', kind: 'offline' as const, uuid: 'uuid', note: null};
const baseStubs = {GameModsPanel: true, LocalPacks: true, GameWorlds: true, GameScreenshots: true, GameDiagnosticsPanel: true, GameAppearancePanel: true, InstanceExtensionSettings: true, KeybindingTransfer: true, GameRuntimeSettings: true};
const props = {game, starting: false, profile, library: {profiles: [profile], active_id: profile.id}};
beforeEach(() => {setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue([]);});
afterEach(() => setLocale('zh-CN'));
it('shows the selected instance icon and preserves its preset if the image fails', async () => {
  const dataUrl = 'data:image/png;base64,aWNvbg==';
  vi.mocked(invoke).mockImplementation(async command => command === 'get_game_icons' ? [{ id: game.id, icon_ref: 'custom-icon', data_url: dataUrl }] : []);
  const wrapper = mount(GameDetailsPage, { props: { ...props, game: { ...game, icon_ref: 'custom-icon', appearance_preset: 'water' } }, global: { stubs: { ...baseStubs, GameConfigsPanel: true } } });
  await flushPromises();
  expect(wrapper.get('.details-game-mark img').attributes('src')).toBe(dataUrl);
  expect(wrapper.get('.details-game-mark').attributes('data-preset')).toBe('water');
  await wrapper.get('.details-game-mark img').trigger('error');
  expect(wrapper.find('.details-game-mark svg').exists()).toBe(true);
  wrapper.unmount();
});
it('selects the existing runtime page for a recovery request without changing settings', async () => {
  const wrapper = mount(GameDetailsPage, { props: { ...props, active: true, recoveryTarget: { kind: 'runtimeSettings' } }, global: { stubs: { ...baseStubs, GameConfigsPanel: true } } });
  await flushPromises(); expect(wrapper.get('#game-runtime-tab').attributes('aria-selected')).toBe('true');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command.startsWith('set_game_'))).toBe(false); wrapper.unmount();
});
it('opens screenshots lazily for Vanilla without showing Mod config or a nested Back control', async () => {
  const wrapper = mount(GameDetailsPage, {props: {...props, game: {...game, loader_kind: null, loader_version: null}}, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  expect(wrapper.find('#game-configs-tab').exists()).toBe(false); expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe(game.id);
  expect(wrapper.get('#game-screenshots-tab').attributes('aria-selected')).toBe('true');
  expect(wrapper.find('.back-button').exists()).toBe(false); wrapper.unmount();
});
it('includes configuration drafts in tab, launch, and shared leave guards', async () => {
  let dirty = true; const confirmLeave = vi.fn(async () => !dirty);
  const Config = defineComponent({name: 'GameConfigsPanel', props: {game: {type: Object, required: true}, locked: Boolean}, setup(_props, {expose}) {expose({canLeave: () => !dirty, confirmLeave}); return () => h('div', 'Config editor');}});
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: Config}}});
  await wrapper.get('#game-configs-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.vm.canLeave()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.get('#game-configs-tab').attributes('aria-selected')).toBe('true'); expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false);
  await wrapper.get('.play-button').trigger('click'); await flushPromises(); expect(wrapper.emitted('play')).toBeUndefined();
  expect(await wrapper.vm.confirmLeave()).toBe(false); expect(confirmLeave).toHaveBeenCalledTimes(3);
  dirty = false; await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).exists()).toBe(true);
  await wrapper.get('.play-button').trigger('click'); await flushPromises(); expect(wrapper.emitted('play')).toEqual([[game.id]]); wrapper.unmount();
});
it('propagates config writes into task locks and provides a stopped-game lock to the panel', async () => {
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  await wrapper.get('#game-configs-tab').trigger('keydown', {key: 'Enter'}); await flushPromises(); const config = wrapper.findComponent({name: 'GameConfigsPanel'});
  config.vm.$emit('busy', true); await flushPromises();
  expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('[role=tab]').every(tab => tab.attributes('disabled') !== undefined)).toBe(true);
  expect(wrapper.emitted('contentBusy')?.at(-1)).toEqual([true]); expect(wrapper.vm.canLeave()).toBe(false);
  config.vm.$emit('busy', false); await flushPromises(); await wrapper.setProps({game: {...game, launch_state: 'running'}});
  expect(config.props('locked')).toBe(true); expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined(); wrapper.unmount();
});
it('shows recovery verification without describing the instance as running and blocks launch', async () => {
  const wrapper = mount(GameDetailsPage, {props:{...props, game:{...game, launch_state:'verifying'}}, global:{stubs:{...baseStubs, GameConfigsPanel:true}}});
  expect(wrapper.get('.launch-session-status').text()).toBe('Checking whether the game is still open…');
  expect(wrapper.get('.play-button').text()).toContain('Checking whether the game is still open…');
  expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined();
  expect(wrapper.find('#game-worlds-tab').exists()).toBe(true);
  await wrapper.get('.play-button').trigger('click'); expect(wrapper.emitted('play')).toBeUndefined();
  wrapper.unmount();
});
it('offers a session check retry while keeping game details available after recovery failure', async () => {
  const wrapper = mount(GameDetailsPage, {props:{...props, game:{...game, launch_state:'recovery_failed'}}, global:{stubs:{...baseStubs, GameConfigsPanel:true}}});
  expect(wrapper.get('.launch-session-status').text()).toBe('Could not verify whether the game is still open.');
  expect(wrapper.get('.play-button').text()).toContain('Retry check');
  await wrapper.get('.play-button').trigger('click');
  expect(wrapper.emitted('retrySessionRecovery')).toEqual([[game.id]]);
  expect(wrapper.find('#game-worlds-tab').exists()).toBe(true);
  wrapper.unmount();
});
it('resets visited feature panels when the active instance changes', async () => {
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises(); expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe('modded');
  await wrapper.setProps({game: {...game, id: 'second', loader_kind: null, loader_version: null}}); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false); expect(wrapper.get('#game-content-tab').attributes('aria-selected')).toBe('true'); expect(wrapper.find('#game-configs-tab').exists()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises(); expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe('second'); wrapper.unmount();
});


it('blocks launch and shared navigation while the managed pack update comparison is pending',async()=>{
 let complete!:(value:null)=>void;
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?{supported:true,reason:null,provider:'mrpack',name:'Pack',installed_version:'v1',online_versions:false,identity_relation:'unknown'}:command==='preview_game_pack_update'?new Promise<null>(resolve=>{complete=resolve;}):[]);
 const wrapper=mount(GameDetailsPage,{props:{...props,game:{...game,pack_name:'Pack',pack_version:'v1'}},global:{stubs:baseStubs}});await flushPromises();
 const panel=wrapper.getComponent(GamePackUpdatePanel);expect(panel.vm.supported).toBe(true);const pending=panel.vm.choose();await flushPromises();
 expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined();expect(wrapper.vm.canLeave()).toBe(false);expect(await wrapper.vm.confirmLeave()).toBe(false);
 await wrapper.get('.play-button').trigger('click');expect(wrapper.emitted('play')).toBeUndefined();
 complete(null);await pending;await flushPromises();expect(wrapper.get('.play-button').attributes('disabled')).toBeUndefined();expect(wrapper.vm.canLeave()).toBe(true);wrapper.unmount();
});

it('retains visited runtime controls when switching tabs and pauses hidden screenshot tools', async () => {
  const Runtime = defineComponent({setup(_props, {expose}) {
    expose({canLeave: () => true, confirmLeave: async () => true});
    return () => h('input', {class: 'runtime-selection'});
  }});
  const wrapper = mount(GameDetailsPage, {props: {...props, active: true}, global: {stubs: {...baseStubs, GameRuntimeSettings: Runtime, KeybindingTransfer: Runtime, InstanceExtensionSettings: Runtime, GameConfigsPanel: true}}});
  await wrapper.get('#game-runtime-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  await wrapper.get('.runtime-selection').setValue('custom runtime');
  await wrapper.get('#game-content-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.get('.runtime-selection').isVisible()).toBe(false);
  await wrapper.get('#game-runtime-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect((wrapper.get('.runtime-selection').element as HTMLInputElement).value).toBe('custom runtime');
  await wrapper.get('#game-screenshots-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).props('active')).toBe(true);
  await wrapper.setProps({active: false});
  expect(wrapper.findComponent(GameScreenshots).props('active')).toBe(false);
  wrapper.unmount();
});

it('freezes rename, profile and runtime write controls during a live session', async () => {
  const wrapper = mount(GameDetailsPage, {props:{...props, game:{...game, launch_state:'running'}}, global:{stubs:{...baseStubs, GameConfigsPanel:true}}});
  expect(wrapper.get('.details-rename').attributes('disabled')).toBeDefined();
  await wrapper.get('#game-runtime-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.get('#game-player-profile').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.runtime-write-controls').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.runtime-write-controls').attributes('inert')).toBeDefined();
  await wrapper.get('#game-player-profile').trigger('change'); expect(vi.mocked(invoke).mock.calls.some(call => call[0] === 'set_game_profile')).toBe(false);
  wrapper.unmount();
});

it('uses Reka tab navigation while retaining a single selected panel', async () => {
  const wrapper = mount(GameDetailsPage, {props, attachTo: document.body, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  await flushPromises();
  const content = wrapper.get('#game-content-tab');
  (content.element as HTMLElement).focus();
  await content.trigger('keydown', {key: 'ArrowRight'}); await flushPromises();
  expect(document.activeElement).toBe(wrapper.get('#game-worlds-tab').element);
  expect(content.attributes('aria-selected')).toBe('true');
  await wrapper.get('#game-worlds-tab').trigger('keydown', {key: 'Enter'}); await flushPromises();
  expect(wrapper.get('#game-worlds-tab').attributes('aria-selected')).toBe('true');
  expect(wrapper.get('#game-content-panel').isVisible()).toBe(false);
  expect(wrapper.get('#game-worlds-panel').attributes('aria-labelledby')).toBe('game-worlds-tab');
  wrapper.unmount();
});

it('changes the player directly in the header and preserves the live-session write lock', async () => {
  const other = {...profile, id: 'other', name: 'Other player'};
  vi.mocked(invoke).mockImplementation(async command => command === 'set_game_profile' ? {...game, profile_id: other.id} : []);
  const wrapper = mount(GameDetailsPage, {props: {...props, library: {profiles: [profile, other], active_id: profile.id}}, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  const picker = wrapper.findAllComponents(SelectField).find(field => field.props('id') === 'game-header-profile')!;
  picker.vm.$emit('update:modelValue', other.id); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_profile', {id: game.id, profileId: other.id});
  expect(wrapper.emitted('profileChanged')?.[0]?.[0]).toMatchObject({profile_id: other.id});
  expect(wrapper.get('#game-content-tab').attributes('aria-selected')).toBe('true');
  await wrapper.setProps({game: {...game, launch_state: 'running'}});
  vi.mocked(invoke).mockClear(); picker.vm.$emit('update:modelValue', '__current__'); await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('set_game_profile', expect.anything());
  wrapper.unmount();
});
