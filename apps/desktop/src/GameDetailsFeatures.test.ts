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
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn()}));
const game: Game = {id: 'modded', name: 'Modded', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16', directory: 'private', status: 'installed', progress: null, error: null};
const profile = {id: 'player', name: 'Player', kind: 'offline' as const, uuid: 'uuid', note: null};
const baseStubs = {GameModsPanel: true, LocalPacks: true, GameWorlds: true, GameScreenshots: true, GameDiagnosticsPanel: true, GameAppearancePanel: true, InstanceExtensionSettings: true, KeybindingTransfer: true, GameRuntimeSettings: true};
const props = {game, starting: false, profile, library: {profiles: [profile], active_id: profile.id}};
beforeEach(() => {setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue([]);});
afterEach(() => setLocale('zh-CN'));
it('opens screenshots lazily for Vanilla without showing Mod config or a nested Back control', async () => {
  const wrapper = mount(GameDetailsPage, {props: {...props, game: {...game, loader_kind: null, loader_version: null}}, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  expect(wrapper.find('#game-configs-tab').exists()).toBe(false); expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe(game.id);
  expect(wrapper.get('#game-screenshots-tab').attributes('aria-selected')).toBe('true');
  expect(wrapper.find('.back-button').exists()).toBe(false); wrapper.unmount();
});
it('includes configuration drafts in tab, launch, and shared leave guards', async () => {
  let dirty = true; const confirmLeave = vi.fn(async () => !dirty);
  const Config = defineComponent({name: 'GameConfigsPanel', props: {game: {type: Object, required: true}, locked: Boolean}, setup(_props, {expose}) {expose({canLeave: () => !dirty, confirmLeave}); return () => h('div', 'Config editor');}});
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: Config}}});
  await wrapper.get('#game-configs-tab').trigger('click'); await flushPromises();
  expect(wrapper.vm.canLeave()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises();
  expect(wrapper.get('#game-configs-tab').attributes('aria-selected')).toBe('true'); expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false);
  await wrapper.get('.play-button').trigger('click'); await flushPromises(); expect(wrapper.emitted('play')).toBeUndefined();
  expect(await wrapper.vm.confirmLeave()).toBe(false); expect(confirmLeave).toHaveBeenCalledTimes(3);
  dirty = false; await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).exists()).toBe(true);
  await wrapper.get('.play-button').trigger('click'); await flushPromises(); expect(wrapper.emitted('play')).toEqual([[game.id]]); wrapper.unmount();
});
it('propagates config writes into task locks and provides a stopped-game lock to the panel', async () => {
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  await wrapper.get('#game-configs-tab').trigger('click'); await flushPromises(); const config = wrapper.findComponent({name: 'GameConfigsPanel'});
  config.vm.$emit('busy', true); await flushPromises();
  expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('[role=tab]').every(tab => tab.attributes('disabled') !== undefined)).toBe(true);
  expect(wrapper.emitted('contentBusy')?.at(-1)).toEqual([true]); expect(wrapper.vm.canLeave()).toBe(false);
  config.vm.$emit('busy', false); await flushPromises(); await wrapper.setProps({game: {...game, launch_state: 'running'}});
  expect(config.props('locked')).toBe(true); expect(wrapper.get('.play-button').attributes('disabled')).toBeDefined(); wrapper.unmount();
});
it('resets visited feature panels when the active instance changes', async () => {
  const wrapper = mount(GameDetailsPage, {props, global: {stubs: {...baseStubs, GameConfigsPanel: true}}});
  await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises(); expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe('modded');
  await wrapper.setProps({game: {...game, id: 'second', loader_kind: null, loader_version: null}}); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).exists()).toBe(false); expect(wrapper.get('#game-content-tab').attributes('aria-selected')).toBe('true'); expect(wrapper.find('#game-configs-tab').exists()).toBe(false);
  await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises(); expect(wrapper.findComponent(GameScreenshots).props('gameId')).toBe('second'); wrapper.unmount();
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
  await wrapper.get('#game-runtime-tab').trigger('click'); await flushPromises();
  await wrapper.get('.runtime-selection').setValue('custom runtime');
  await wrapper.get('#game-content-tab').trigger('click'); await flushPromises();
  expect(wrapper.get('.runtime-selection').isVisible()).toBe(false);
  await wrapper.get('#game-runtime-tab').trigger('click'); await flushPromises();
  expect((wrapper.get('.runtime-selection').element as HTMLInputElement).value).toBe('custom runtime');
  await wrapper.get('#game-screenshots-tab').trigger('click'); await flushPromises();
  expect(wrapper.findComponent(GameScreenshots).props('active')).toBe(true);
  await wrapper.setProps({active: false});
  expect(wrapper.findComponent(GameScreenshots).props('active')).toBe(false);
  wrapper.unmount();
});
