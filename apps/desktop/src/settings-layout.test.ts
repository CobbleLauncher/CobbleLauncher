// SPDX-License-Identifier: MIT
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import SettingsPage from './SettingsPage.vue';
import GameRuntimeSettings from './GameRuntimeSettings.vue';
import { locale, setLocale } from './i18n';
import SelectField from './ui/SelectField.vue';
import { applyAppearance, defaultAppearance, readAppearance } from './theme';
import { readHomeLayout } from './home-layout';
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
    if (command === 'launcher_update_status') return { version: '0.1.0', background_check: false, channel: 'stable', configured: false, can_install: false, available_version: null };
    if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
    if (command === 'list_java_runtimes' || command === 'list_game_java_runtimes') return { runtimes: [], requiredMajor: 21, recommendedId: null };
    if (command === 'set_default_game_directory') return { ...preferences, game_directory: args!.path };
    if (command === 'set_default_memory') return { ...preferences, memory_mb: args!.memoryMb };
    return preferences;
  });
}
beforeEach(() => { localStorage.clear(); applyAppearance(defaultAppearance); vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} }); });
afterEach(() => { vi.unstubAllGlobals(); vi.mocked(invoke).mockReset(); vi.mocked(writeText).mockReset(); setLocale('zh-CN'); });
it('defaults legacy concurrency settings to adaptive and saves manual mode with existing limits', async () => {
  setLocale('en-US'); native();
  const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } });
  await flushPromises();
  expect(wrapper.get('#adaptive-downloads').attributes('data-state')).toBe('checked');
  expect(wrapper.vm.canLeave()).toBe(true);
  await wrapper.get('#adaptive-downloads').trigger('click');
  const concurrency = row(wrapper, 'Concurrency');
  expect(concurrency.find('.settings-actions').exists()).toBe(false);
  await wrapper.get('.settings-save-bar .primary-button').trigger('click');
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
  expect(wrapper.findAll('.settings-save-bar')).toHaveLength(1);
  expect(source.find('.settings-actions').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_download_settings')).toBe(false);
  await wrapper.get('.settings-save-bar .text-button').trigger('click');
  expect(source.find('.settings-actions').exists()).toBe(false);
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});
it('uses the typed directory setter only on Apply and copies the displayed normal-font path', async () => {
  setLocale('en-US'); native(); const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } }); await flushPromises();
  await wrapper.get('#default-game-directory').setValue('D:\\New games');
  expect(wrapper.vm.canLeave()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_default_game_directory')).toBe(false);
  await wrapper.get('button[aria-label="Copy path"]').trigger('click'); expect(writeText).toHaveBeenCalledWith('D:\\New games');
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_default_game_directory', { path: 'D:\\New games' });
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});
it('offers Apply, Discard and Cancel before navigation and supports trusted extra category slots', async () => {
  setLocale('en-US'); native(); const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads', extraCategories: [{ id: 'example', label: 'Example settings' }] }, slots: { 'category-example': '<p>Extension settings</p>' } }); await flushPromises();
  await wrapper.get('#default-game-directory').setValue('D:\\Draft');
  const result = wrapper.vm.confirmLeave(); await flushPromises();
  const dialog = document.body.querySelector('[role="dialog"]')!;
  expect(dialog.textContent).toContain('Save settings'); expect(dialog.textContent).toContain('Discard changes');
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
  wrapper.findAllComponents(SelectField).find(field => field.props('id') === 'game-memory-mode')!.vm.$emit('update:modelValue', 'inherit'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_game_memory')).toBe(false);
  await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_memory', { id: 'game', memoryMb: null }); wrapper.unmount();
});

it('awaits extension draft guards before changing category or leaving settings', async () => {
  setLocale('en-US'); native(); const beforeCategoryLeave = vi.fn().mockResolvedValue(false);
  const wrapper = mount(SettingsPage, { props: { beforeCategoryLeave } }); await flushPromises();
  await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Game defaults')!.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(beforeCategoryLeave).toHaveBeenCalledTimes(1);
  expect(wrapper.findAll('[role="tab"]').find(tab => tab.attributes('aria-selected') === 'true')!.text()).toBe('Appearance');
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
  wrapper.findAllComponents(SelectField).find(field => field.props('id') === 'game-memory-mode')!.vm.$emit('update:modelValue', 'custom'); await flushPromises();
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

it('retains appearance drafts across categories and saves or discards through one settings bar', async () => {
  setLocale('en-US'); native();
  const wrapper = mount(SettingsPage); await flushPromises();
  const choose = async (name: string) => { await wrapper.findAll('[role="tab"]').find(tab => tab.text() === name)!.trigger('keydown', { key: 'Enter' }); await flushPromises(); };
  const active = () => wrapper.findAll('[role="tab"]').find(tab => tab.attributes('aria-selected') === 'true')!.text();
  await wrapper.get('input[name="appearance-accent"][value="blue"]').setValue();
  expect(wrapper.vm.canLeave()).toBe(false);
  await choose('Game defaults');
  expect(active()).toBe('Game defaults');
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(readAppearance().accent).toBe('green');
  await choose('Appearance');
  expect(wrapper.get('.appearance-preview-canvas').attributes('data-accent')).toBe('blue');
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(readAppearance().accent).toBe('blue');
  expect(wrapper.find('.settings-save-bar').exists()).toBe(false);
  await wrapper.get('input[name="appearance-accent"][value="purple"]').setValue();
  await choose('Downloads & storage');
  await wrapper.get('.settings-save-bar .text-button').trigger('click'); await flushPromises();
  await choose('Appearance');
  expect(wrapper.get('.appearance-preview-canvas').attributes('data-accent')).toBe('blue');
  expect(wrapper.find('.appearance-save').exists()).toBe(false);
  expect(wrapper.vm.canLeave()).toBe(true);
  wrapper.unmount();
});

it('stages language and home layout together and supports external category requests', async () => {
  setLocale('en-US'); native(); const wrapper = mount(SettingsPage); await flushPromises();
  wrapper.findAllComponents(SelectField).find(field => field.props('id') === 'language')!.vm.$emit('update:modelValue', 'zh-CN');
  await flushPromises(); expect(locale.value).toBe('en-US');
  await wrapper.setProps({ initialCategory: 'home-layout' }); await flushPromises();
  await wrapper.get('[data-section="other"] .home-layout-drag').trigger('keydown', { key: 'ArrowDown' });
  expect(readHomeLayout().order).toEqual(['other', 'quick', 'favorites']);
  expect(wrapper.emitted('editHome')).toBeUndefined();
  await wrapper.vm.openCategory('downloads'); await flushPromises();
  expect(wrapper.findAll('.settings-save-bar')).toHaveLength(1);
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(readHomeLayout().order).toEqual(['quick', 'other', 'favorites']);
  expect(locale.value).toBe('zh-CN');
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});

it('uses readonly directory pick/reset candidates and persists only from the settings bar', async () => {
  setLocale('en-US'); native(); const previous = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'pick_default_game_directory_candidate') return { path: 'D:\\Picked games' };
    if (command === 'get_standard_game_directory') return { path: 'C:\\Standard games' };
    if (command === 'reset_default_game_directory') return { ...preferences, game_directory: 'C:\\Standard games', custom_game_directory: false };
    return previous(command, args);
  });
  const wrapper = mount(SettingsPage, { props: { initialCategory: 'downloads' } }); await flushPromises();
  await wrapper.get('button[aria-label="Browse"]').trigger('click'); await flushPromises();
  expect(wrapper.get('#default-game-directory').element).toHaveProperty('value', 'D:\\Picked games');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'set_default_game_directory')).toBe(false);
  await wrapper.get('.settings-save-bar .text-button').trigger('click'); await flushPromises();
  await wrapper.get('button[aria-label="Reset to default"]').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'get_standard_game_directory')).toBe(false);
  document.querySelector<HTMLButtonElement>('.confirmation-confirm')!.click(); await flushPromises();
  expect(wrapper.get('#default-game-directory').element).toHaveProperty('value', 'C:\\Standard games');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'reset_default_game_directory')).toBe(false);
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('reset_default_game_directory', undefined);
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});

it('retains only failed native drafts after a partial save and retries without repeating saved Java', async () => {
  setLocale('en-US'); native(); const previous = vi.mocked(invoke).getMockImplementation()!;
  const runtime = { id: 'candidate-21', major: 21, version: '21.0.1', vendor: 'Test', arch: 'x64', executable: 'C:\\Java21\\bin\\javaw.exe', recommended: true };
  let rejectMemory = true;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'pick_java_candidate') return runtime;
    if (command === 'select_java_runtime') return { mode: 'custom', executable: runtime.executable, runtime };
    if (command === 'set_default_memory' && rejectMemory) throw Error('write failed');
    return previous(command, args);
  });
  const wrapper = mount(SettingsPage, { props: { initialCategory: 'runtime' } }); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Choose Java executable')!.trigger('click'); await flushPromises();
  await wrapper.get('#default-memory').setValue('8192');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'select_java_runtime')).toBe(false);
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('select_java_runtime', { runtimeId: 'candidate-21' });
  expect(wrapper.get('.settings-save-bar [role="alert"]').text()).toContain('Could not save');
  expect(wrapper.vm.canLeave()).toBe(false);
  rejectMemory = false;
  await wrapper.get('.settings-save-bar .primary-button').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'select_java_runtime')).toHaveLength(1);
  expect(wrapper.vm.canLeave()).toBe(true); wrapper.unmount();
});
