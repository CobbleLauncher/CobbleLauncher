// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CreateGamePage from './CreateGamePage.vue';
import VersionSelector from './VersionSelector.vue';
import MinecraftVersionTiles from './MinecraftVersionTiles.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn(), isTauri: () => true}));
const versions = Array.from({length: 60}, (_, index) => ({id: index === 0 ? '1.21.1' : `1.20.${index}`, kind: 'release'}));
beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_download_settings') return {source: 'official', mirror: ''};
    if (command === 'default_game_directory') return 'C:/Games';
    if (command === 'list_game_versions') return versions;
    if (command === 'list_fabric_versions') return [{version: '0.17-beta', stable: false}, {version: '0.16.9', stable: true}];
    if (command === 'create_game') return {id: 'new', status: 'installing'};
    return null;
  });
});
afterEach(() => setLocale('zh-CN'));
it('defaults to a release when a snapshot is newer and never refetches the manifest for tile selection', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_download_settings' ? {source:'official',mirror:''} : command === 'default_game_directory' ? 'C:/Games' : command === 'list_game_versions' ? [{id:'26w01a',kind:'snapshot'},...versions] : null);
  const wrapper = mount(CreateGamePage); await flushPromises();
  expect(wrapper.findComponent(MinecraftVersionTiles).props('modelValue')).toBe('1.21.1');
  const calls = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'list_game_versions').length;
  wrapper.findComponent(MinecraftVersionTiles).vm.$emit('update:modelValue','1.20.1'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'list_game_versions')).toHaveLength(calls);
  wrapper.unmount();
});
it('preserves manual names and resumes generated names when cleared, with recommended loader defaults', async () => {
  const wrapper = mount(CreateGamePage); await flushPromises();
  expect(wrapper.get('#game-name').element).toHaveProperty('value', 'Minecraft 1.21.1');
  expect(wrapper.findComponent(MinecraftVersionTiles).props('versions')).toHaveLength(60);
  await wrapper.get('#game-name').setValue('My game');
  wrapper.findComponent(MinecraftVersionTiles).vm.$emit('update:modelValue', '1.20.1'); await flushPromises();
  expect(wrapper.get('#game-name').element).toHaveProperty('value', 'My game');
  await wrapper.get('#game-name').setValue('');
  expect(wrapper.get('#game-name').element).toHaveProperty('value', 'Minecraft 1.20.1');
  await wrapper.findAll('[role=radio]').find(button => button.text() === 'Fabric')!.trigger('click'); await flushPromises();
  expect(wrapper.findComponent(VersionSelector).props('modelValue')).toBe('0.16.9');
  expect(wrapper.get('#game-name').element).toHaveProperty('value', 'Minecraft 1.20.1 Fabric 0.16.9');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_game', {name: 'Minecraft 1.20.1 Fabric 0.16.9', automaticName: true, version: '1.20.1', loaderKind: 'fabric', loaderVersion: '0.16.9', source: {source: 'official', mirror: ''}, useDefaultDirectory: true});
  expect(wrapper.emitted('created')).toHaveLength(1); wrapper.unmount();
});
it('keeps user-edited names and saved mirror preferences when changing the source', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_download_settings' ? {source: 'custom', mirror: 'https://mirror.example.com/'} : command === 'list_game_versions' ? versions : command === 'default_game_directory' ? 'C:/Games' : {id: 'new'});
  const wrapper = mount(CreateGamePage); await flushPromises();
  await wrapper.get('.create-install-options').trigger('click');
  expect(wrapper.get('#create-mirror').attributes('readonly')).toBeDefined();
  await wrapper.get('#game-name').setValue('My named game');
  wrapper.findComponent(MinecraftVersionTiles).vm.$emit('update:modelValue', '1.20.1'); await flushPromises();
  await wrapper.findAll('[role=radio]').find(button => button.text() === 'BMCLAPI')!.trigger('click'); await flushPromises();
  expect(wrapper.findComponent(MinecraftVersionTiles).props('modelValue')).toBe('1.20.1');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_versions', {source: {source: 'bmclapi', mirror: 'https://mirror.example.com/'}});
  expect(invoke).toHaveBeenCalledWith('create_game', expect.objectContaining({name: 'My named game', automaticName: false, version:'1.20.1', source: {source: 'bmclapi', mirror: 'https://mirror.example.com/'}})); wrapper.unmount();
});
it('uses paired copy and localized safe errors while preserving the installer warning', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_download_settings' ? {source: 'official', mirror: ''} : command === 'list_game_versions' ? versions : command === 'list_loader_versions' ? [] : 'C:/Games');
  const wrapper = mount(CreateGamePage); await flushPromises();
  expect(wrapper.find('.create-description').exists()).toBe(false);
  await wrapper.findAll('[role=radio]').find(button => button.text() === 'Forge')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('official installer'); expect(wrapper.get('[role=alert]').text()).toContain('No Forge versions');
  setLocale('zh-CN'); await flushPromises(); expect(wrapper.text()).toContain('游戏名称'); expect(wrapper.text()).toContain('官方安装器'); wrapper.unmount();
});
