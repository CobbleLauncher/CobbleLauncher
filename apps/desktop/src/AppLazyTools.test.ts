// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import App from './App.vue';
import { setLocale } from './i18n';

const loaded = vi.hoisted(() => ({ count: 0, suspend: vi.fn<() => Promise<void>>().mockResolvedValue(undefined), resume: vi.fn() }));
vi.mock('./ToolsPage.vue', () => {
  loaded.count++;
  return { __esModule: true, default: { name: 'ToolsPage', props: ['games', 'initialGameId', 'location', 'suspended'], methods: { suspend: () => loaded.suspend(), resume: () => loaded.resume(), confirmLeave: async () => true }, template: '<div data-testid="loaded-tools" />' } };
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {}) }) }));
vi.mock('./theme', () => ({ readTheme: () => 'light', applyTheme: vi.fn() }));

it('keeps the Tools module outside initial library rendering and loads it once on explicit navigation', async () => {
  setLocale('en-US'); window.scrollTo = vi.fn();
  vi.mocked(invoke).mockImplementation(async name => name === 'list_player_profiles' ? { profiles: [], active_id: null } : name.startsWith('list_') ? [] : null);
  const wrapper = mount(App, { global: { stubs: {
    AppNavigation: { emits: ['navigate'], template: '<nav><button @click="$emit(\'navigate\', \'tools\')">Open tools</button><button @click="$emit(\'navigate\', \'library\')">Open library</button></nav>' },
    LibraryHome: true, ProfileSwitcher: true, CreateGamePage: true, ImportPackPage: true, ImportLauncherPage: true,
    CfPackInstallPage: true, SettingsPage: true, PluginsPanel: true, ExtensionSettingsPanel: true,
    GameDetailsPage: true, GameActionsPage: true, MeshPage: true, ProfilesPage: true, AddProfilePage: true,
  } } });
  try {
    await flushPromises();
    expect(loaded.count).toBe(0);
    expect(wrapper.find('[data-testid="loaded-tools"]').exists()).toBe(false);
    expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'list_plugins' || name === 'list_extension_contributions')).toBe(false);
    await wrapper.findAll('button').find(button => button.text() === 'Open tools')!.trigger('click');
    await vi.waitFor(() => expect(loaded.count).toBe(1)); await flushPromises();
    expect(wrapper.find('[data-testid="loaded-tools"]').exists()).toBe(true);
    let finishSuspend!: () => void;
    loaded.suspend.mockImplementationOnce(() => new Promise<void>(resolve => { finishSuspend = resolve; }));
    const tasksButton = wrapper.find('button.tasks-button[aria-expanded="false"]');
    await tasksButton.trigger('click');
    await flushPromises();
    expect(loaded.suspend).toHaveBeenCalledOnce();
    expect(tasksButton.attributes('aria-expanded')).toBe('false');
    finishSuspend();
    await flushPromises();
    expect(tasksButton.attributes('aria-expanded')).toBe('true');
    await tasksButton.trigger('click');
    await flushPromises();
    expect(loaded.resume).toHaveBeenCalled();

    await wrapper.findAll('button').find(button => button.text() === 'Open library')!.trigger('click'); await flushPromises();
    expect(wrapper.find('[data-testid="loaded-tools"]').exists()).toBe(false);
    await wrapper.findAll('button').find(button => button.text() === 'Open tools')!.trigger('click'); await flushPromises();
    expect(loaded.count).toBe(1);
    expect(wrapper.find('[data-testid="loaded-tools"]').exists()).toBe(true);
  } finally { wrapper.unmount(); setLocale('zh-CN'); vi.mocked(invoke).mockReset(); }
});
