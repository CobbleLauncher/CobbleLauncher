// SPDX-License-Identifier: MIT
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import SettingsPage from './SettingsPage.vue';
import GameRuntimeSettings from './GameRuntimeSettings.vue';
import { setLocale } from './i18n';
import type { Game } from './games';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const preferences = { memory_mb: 4096, game_directory: 'C:\\Games', custom_game_directory: true };
function native() {
  vi.mocked(invoke).mockImplementation(async (command, rawArgs) => {
    const args = rawArgs as Record<string, unknown> | undefined;
    if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
    if (command === 'get_launcher_preferences') return preferences;
    if (command === 'get_download_tuning') return { max_concurrent: 12, per_origin: 4 };
    if (command === 'get_public_cache_stats') return { disk_bytes: 0 };
    if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
    if (command === 'list_java_runtimes' || command === 'list_game_java_runtimes') return { runtimes: [], requiredMajor: 21, recommendedId: null };
    if (command === 'set_default_game_directory') return { ...preferences, game_directory: args!.path };
    return preferences;
  });
}
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} }); });
afterEach(() => { vi.unstubAllGlobals(); vi.mocked(invoke).mockReset(); vi.mocked(writeText).mockReset(); setLocale('zh-CN'); });
it('defaults legacy concurrency settings to adaptive and saves manual mode with existing limits', async () => {
  setLocale('en-US'); native();
  const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } });
  await flushPromises();
  expect(wrapper.get<HTMLInputElement>('#adaptive-downloads').element.checked).toBe(true);
  expect(wrapper.vm.canLeave()).toBe(true);
  await wrapper.get('#adaptive-downloads').setValue(false);
  const concurrency = row(wrapper, 'Concurrency');
  await concurrency.findAll('button').find(button => button.text() === 'Apply')!.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_download_tuning', { tuning: { adaptive: false, max_concurrent: 12, per_origin: 4 } });
  expect(wrapper.vm.canLeave()).toBe(true);
  wrapper.unmount();
});
function row(wrapper: ReturnType<typeof mount>, label: string) { return wrapper.findAll('.settings-row').find(item => item.find('.settings-row-label').text() === label)!; }
it('stages download changes, cancels them locally, and removes plugin management from settings', async () => {
  setLocale('en-US'); native();
  const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } }); await flushPromises();
  expect(wrapper.findAll('[role="tab"]').some(tab => tab.text() === 'Extensions')).toBe(false);
  const source = row(wrapper, 'Download source');
  await source.get('[role="radio"][data-state="unchecked"]').trigger('click'); await flushPromises();
  expect(source.find('.settings-actions').exists()).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_download_settings')).toBe(false);
  await source.findAll('button').find(button => button.text() === 'Cancel')!.trigger('click');
  expect(source.find('.settings-actions').exists()).toBe(false);
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});
it('uses the typed directory setter only on Apply and copies the displayed normal-font path', async () => {
  setLocale('en-US'); native(); const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } }); await flushPromises();
  await wrapper.get('#default-game-directory').setValue('D:\\New games');
  expect(wrapper.vm.canLeave()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_default_game_directory')).toBe(false);
  await wrapper.get('button[aria-label="Copy path"]').trigger('click'); expect(writeText).toHaveBeenCalledWith('D:\\New games');
  await row(wrapper, 'New game directory').findAll('button').find(button => button.text() === 'Apply')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_default_game_directory', { path: 'D:\\New games' });
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});
it('offers Apply, Discard and Cancel before navigation and supports trusted extra category slots', async () => {
  setLocale('en-US'); native(); const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads', extraCategories: [{ id: 'example', label: 'Example settings' }] }, slots: { 'category-example': '<p>Extension settings</p>' } }); await flushPromises();
  await wrapper.get('#default-game-directory').setValue('D:\\Draft');
  const result = wrapper.vm.confirmLeave(); await flushPromises();
  const dialog = document.body.querySelector('[role="dialog"]')!;
  expect(dialog.textContent).toContain('Apply'); expect(dialog.textContent).toContain('Discard changes');
  const cancel = Array.from(dialog.querySelectorAll('button')).find(button => button.textContent === 'Keep editing')!; cancel.click(); await flushPromises(); expect(await result).toBe(false);
  expect(wrapper.get('#default-game-directory').element).toHaveProperty('value', 'D:\\Draft');
  const discard = wrapper.vm.confirmLeave(); await flushPromises(); Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent === 'Discard changes')!.click(); await flushPromises(); expect(await discard).toBe(true);
  await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Example settings')!.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(wrapper.text()).toContain('Extension settings'); wrapper.unmount();
});
it('stages per-game inheritance changes rather than saving on selection', async () => {
  setLocale('en-US'); native();
  const game: Game = { id: 'game', name: 'Game', version: '1.21.1', directory: 'test', status: 'installed', progress: null, error: null, runtime_overrides: { java: { mode: 'inherit' }, memory_mb: 8192 } };
  const wrapper = mount(GameRuntimeSettings, { props: { game } }); await flushPromises();
  await wrapper.get('#game-memory-mode').setValue('inherit');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_game_memory')).toBe(false);
  await row(wrapper, 'Maximum memory').findAll('button').find(button => button.text() === 'Apply')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_memory', { id: 'game', memoryMb: null }); wrapper.unmount();
});

it('awaits extension draft guards before changing category or leaving settings', async () => {
  setLocale('en-US'); native(); const beforeCategoryLeave = vi.fn().mockResolvedValue(false);
  const wrapper = mount(SettingsPage, { props: { beforeCategoryLeave } }); await flushPromises();
  await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Game defaults')!.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(beforeCategoryLeave).toHaveBeenCalledTimes(1);
  expect(wrapper.findAll('[role="tab"]').find(tab => tab.attributes('aria-selected') === 'true')!.text()).toBe('General');
  expect(await wrapper.vm.confirmLeave()).toBe(false);
  beforeCategoryLeave.mockResolvedValue(true);
  await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Game defaults')!.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(wrapper.findAll('[role="tab"]').find(tab => tab.attributes('aria-selected') === 'true')!.text()).toBe('Game defaults');
  wrapper.unmount();
});

it('guards per-game memory drafts with Cancel and Discard before instance navigation', async () => {
  setLocale('en-US'); native();
  const game: Game = { id: 'guarded-game', name: 'Guarded', version: '1.21.1', directory: 'test', status: 'installed', progress: null, error: null };
  const wrapper = mount(GameRuntimeSettings, { props: { game } }); await flushPromises();
  await wrapper.get('#game-memory-mode').setValue('custom');
  await wrapper.get('#game-memory').setValue('8192');
  const keep = wrapper.vm.confirmLeave(); await flushPromises();
  Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent === 'Keep editing')!.click(); await flushPromises();
  expect(await keep).toBe(false); expect(wrapper.vm.canLeave()).toBe(false);
  expect((wrapper.get('#game-memory').element as HTMLInputElement).value).toBe('8192');
  const discard = wrapper.vm.confirmLeave(); await flushPromises();
  Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent === 'Discard changes')!.click(); await flushPromises();
  expect(await discard).toBe(true); expect(wrapper.vm.canLeave()).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_game_memory')).toBe(false);
  wrapper.unmount();
});
