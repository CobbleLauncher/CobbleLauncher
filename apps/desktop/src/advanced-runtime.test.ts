// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import AdvancedRuntimeSettings from './AdvancedRuntimeSettings.vue';
import type { Game } from './games';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const game: Game = { id: 'forest', name: 'Forest', version: '1.21.1', directory: 'test', status: 'installed', progress: null, error: null };
describe('advanced runtime settings', () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });
  it('submits inheritance explicitly without replacing defaults with empty values', async () => {
    vi.mocked(invoke).mockResolvedValue(game);
    const wrapper = mount(AdvancedRuntimeSettings, { props: { game } });
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith('set_game_advanced_runtime', { id: 'forest', windowSize: null, jvmArgs: null, launchBehavior: null });
    expect(wrapper.emitted('updated')?.[0]).toEqual([game]);
  });
  it('keeps JVM lines as individual arguments and saves optional dimensions', async () => {
    const preferences = { memory_mb: 2048, game_directory: 'test', custom_game_directory: false };
    vi.mocked(invoke).mockResolvedValue(preferences);
    const wrapper = mount(AdvancedRuntimeSettings, { props: { preferences } });
    await flushPromises();
    await wrapper.get('input[type="number"]').setValue('1280');
    await wrapper.get('textarea').setValue('-Dlabel=two words\n\n-XX:+UseG1GC');
    await wrapper.get('select').setValue('minimize');
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith('set_default_advanced_runtime', { windowSize: { width: 1280, height: null }, jvmArgs: ['-Dlabel=two words', '-XX:+UseG1GC'], launchBehavior: 'minimize' });
  });
  it('rejects invalid dimensions before invoking the host', async () => {
    const wrapper = mount(AdvancedRuntimeSettings, { props: { preferences: { memory_mb: 2048, game_directory: 'test', custom_game_directory: false } } });
    await flushPromises();
    await wrapper.get('input[type="number"]').setValue('1');
    await wrapper.get('form').trigger('submit');
    expect(invoke).not.toHaveBeenCalled();
    expect(wrapper.get('[role="alert"]').text()).toContain('320–16384');
  });
});

it('keeps advanced drafts when unrelated global memory preferences refresh and cancels back to saved values', async () => {
  const preferences = { memory_mb: 2048, game_directory: 'test', custom_game_directory: false };
  const wrapper = mount(AdvancedRuntimeSettings, { props: { preferences } }); await flushPromises();
  await wrapper.get('textarea').setValue('-Dkeep=draft');
  await wrapper.setProps({ preferences: { ...preferences, memory_mb: 4096 } });
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('-Dkeep=draft');
  wrapper.vm.cancel(); await flushPromises();
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('');
  expect(wrapper.find('.settings-actions').exists()).toBe(false);
});
