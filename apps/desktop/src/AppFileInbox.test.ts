// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import App from './App.vue';
import { setLocale } from './i18n';

const events = vi.hoisted(() => ({ onboardingCompleted: true, handlers: new Map<string, (event: { payload: unknown }) => void>(), reload: vi.fn(), leave: vi.fn(async () => true) }));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn(async (name: string, args?: { step?: string }) => {
  if (name === 'get_onboarding_state') return { step: 'game', consentVersion: '2026-10-24', completed: events.onboardingCompleted, deferred: false };
  if (name === 'save_onboarding_state') return { step: args?.step ?? 'game', consentVersion: '2026-10-24', completed: false, deferred: false };
  if (name === 'list_games') return [{ id: 'one', name: 'First game', version: '1.21.1', status: 'installed', directory: 'fixture' }];
  if (name === 'list_player_profiles') return { profiles: [], active_id: null };
  return name.startsWith('list_') ? [] : null;
}) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
  events.handlers.set(name, handler); return () => events.handlers.delete(name);
}) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {}) }) }));
vi.mock('./theme', () => ({ readTheme: () => 'light', applyTheme: vi.fn() }));
vi.mock('./ToolsPage.vue', () => ({ __esModule: true, default: {
  name: 'ToolsPage', props: ['location'],
  methods: { reload: () => events.reload(), confirmLeave: () => events.leave(), suspend: async () => {}, resume: () => {} },
  template: '<div data-inbox :data-plugin="location?.pluginId" :data-game="location?.instanceId" :data-backup="location?.configRestoreTarget?.backupId" :data-blueprint="location?.blueprintId" />',
} }));
vi.mock('./GameDetailsPage.vue', () => ({ __esModule: true, default: {
  name: 'GameDetailsPage', props: ['recoveryTarget'], methods: { confirmLeave: async () => true },
  template: '<div data-recovery :data-kind="recoveryTarget?.kind" />',
} }));
vi.mock('./FileInboxHostReview.vue', () => ({ __esModule: true, default: {
  name: 'FileInboxHostReview', props: ['request'], emits: ['result', 'finished'],
  methods: { confirmLeave: async () => false }, template: '<div data-host-review />',
} }));
vi.mock('./ImportPackPage.vue', () => ({ __esModule: true, default: {
  name: 'ImportPackPage', props: ['initialPreview'], template: '<div data-pack-import :data-preview="initialPreview?.preview_id" />',
} }));
vi.mock('./FirstRunPage.vue', () => ({ __esModule: true, default: { name: 'FirstRunPage', props: ['step'], emits: ['importLauncher'], template: '<div data-first-run :data-step="step" />' } }));
vi.mock('./ImportLauncherPage.vue', () => ({ __esModule: true, default: { name: 'ImportLauncherPage', emits: ['open'], template: '<div data-launcher-import />' } }));

afterEach(() => { events.onboardingCompleted = true; events.handlers.clear(); vi.clearAllMocks(); setLocale('zh-CN'); });

it('opens a general inbox for a main-window drop and refreshes it on the next drop', async () => {
  setLocale('en-US'); window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await flushPromises();
    const drop = events.handlers.get('plugin-file-inbox-drop')!;
    drop({ payload: { plugin_id: 'other', count: 1 } });
    await flushPromises();
    expect(wrapper.find('[data-inbox]').exists()).toBe(false);
    drop({ payload: { plugin_id: 'file-inbox', count: 1 } });
    await vi.waitFor(() => expect(wrapper.find('[data-inbox]').exists()).toBe(true));
    expect(wrapper.get('[data-inbox]').attributes('data-plugin')).toBe('file-inbox');
    expect(wrapper.get('[data-inbox]').attributes('data-game')).toBeUndefined();
    events.leave.mockResolvedValueOnce(false);
    drop({ payload: { plugin_id: 'file-inbox', count: 2 } });
    await flushPromises();
    expect(events.reload).not.toHaveBeenCalled();
    drop({ payload: { plugin_id: 'file-inbox', count: 2 } });
    await flushPromises();
    expect(events.reload).toHaveBeenCalledOnce();
  } finally { wrapper.unmount(); }
  expect(events.handlers.has('plugin-file-inbox-drop')).toBe(false);
  expect(events.handlers.has('plugin-file-inbox-notice')).toBe(false);
});

it('opens the host review and routes its prepared archive without reopening a picker', async () => {
  setLocale('en-US'); window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await flushPromises();
    events.handlers.get('plugin-file-inbox-open')!({ payload: { handoff_id: 'a'.repeat(32), plugin_id: 'file-inbox' } });
    await vi.waitFor(() => expect(wrapper.find('[data-host-review]').exists()).toBe(true));
    const review = wrapper.findComponent({ name: 'FileInboxHostReview' });
    review.vm.$emit('result', { kind: 'mrpack', preview: { id: 'prepared' } });
    review.vm.$emit('finished');
    await vi.waitFor(() => expect(wrapper.find('[data-pack-import]').exists()).toBe(true));
    expect(wrapper.find('[data-host-review]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
  expect(events.handlers.has('plugin-file-inbox-open')).toBe(false);
});

it('routes validated recovery events to existing workspaces without applying a restore', async () => {
  setLocale('en-US'); window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await flushPromises();
    const recover = events.handlers.get('plugin-open-game-recovery')!;
    recover({ payload: { game_id: 'one', plugin_id: 'crash-doctor', target: { kind: 'runtimeSettings' } } });
    await vi.waitFor(() => expect(wrapper.find('[data-recovery]').exists()).toBe(true));
    expect(wrapper.get('[data-recovery]').attributes('data-kind')).toBe('runtimeSettings');
    recover({ payload: { game_id: 'one', plugin_id: 'crash-doctor', target: { kind: 'configRestore', configId: 'a'.repeat(64), backupId: '100-backup' } } });
    await vi.waitFor(() => expect(wrapper.find('[data-inbox]').exists()).toBe(true));
    expect(wrapper.get('[data-inbox]').attributes('data-plugin')).toBe('config-doctor');
    expect(wrapper.get('[data-inbox]').attributes('data-game')).toBe('one');
    expect(wrapper.get('[data-inbox]').attributes('data-backup')).toBe('100-backup');
    events.handlers.get('plugin-game-open-request')!({ payload: { gameId: 'one' } });
    await flushPromises();
    expect(wrapper.find('[data-inbox]').exists()).toBe(false);
    expect(wrapper.get('[data-recovery]').attributes('data-kind')).toBeUndefined();
  } finally { wrapper.unmount(); }
  expect(events.handlers.has('plugin-open-game-recovery')).toBe(false);
  expect(events.handlers.has('plugin-game-open-request')).toBe(false);
});


it('opens a generated library item through the common leave guard', async () => {
  window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await flushPromises();
    const open = events.handlers.get('plugin-generated-blueprint-open')!;
    open({ payload: { blueprint_id: '../invalid', plugin_id: 'geometry-builder' } });
    await flushPromises();
    expect(wrapper.find('[data-inbox]').exists()).toBe(false);
    events.handlers.get('plugin-file-inbox-drop')!({ payload: { plugin_id: 'file-inbox', count: 1 } });
    await vi.waitFor(() => expect(wrapper.find('[data-inbox]').exists()).toBe(true));
    events.leave.mockResolvedValueOnce(false);
    open({ payload: { blueprint_id: 'generated-one', plugin_id: 'geometry-builder' } });
    await flushPromises();
    expect(wrapper.get('[data-inbox]').attributes('data-blueprint')).toBeUndefined();
    open({ payload: { blueprint_id: 'generated-one', plugin_id: 'geometry-builder' } });
    await vi.waitFor(() => expect(wrapper.get('[data-inbox]').attributes('data-blueprint')).toBe('generated-one'));
  } finally { wrapper.unmount(); }
  expect(events.handlers.has('plugin-generated-blueprint-open')).toBe(false);
});


it('opens a native dropped modpack directly in installation review', async () => {
  window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await flushPromises();
    events.handlers.get('native-pack-preview')!({ payload: { kind: 'mrpack', preview: { preview_id: 'native-drop' } } });
    await vi.waitFor(() => expect(wrapper.find('[data-pack-import]').exists()).toBe(true));
    expect(wrapper.get('[data-pack-import]').attributes('data-preview')).toBe('native-drop');
    expect(wrapper.find('[data-inbox]').exists()).toBe(false);
    expect(wrapper.find('[data-host-review]').exists()).toBe(false);
  } finally { wrapper.unmount(); }
  expect(events.handlers.has('native-pack-preview')).toBe(false);
});


it('returns a completed launcher import to the next onboarding step', async () => {
  events.onboardingCompleted = false; window.scrollTo = vi.fn();
  const wrapper = mount(App, { global: { stubs: { AppNavigation: true, LibraryHome: true, ProfileSwitcher: true, MeshStatusButton: true } } });
  try {
    await vi.waitFor(() => expect(wrapper.find('[data-first-run]').exists()).toBe(true));
    wrapper.findComponent({ name: 'FirstRunPage' }).vm.$emit('importLauncher');
    await vi.waitFor(() => expect(wrapper.find('[data-launcher-import]').exists()).toBe(true));
    wrapper.findComponent({ name: 'ImportLauncherPage' }).vm.$emit('open', {id:'imported',name:'Imported',version:'1.21.1',status:'installed',directory:'fixture'});
    await vi.waitFor(() => expect(wrapper.get('[data-first-run]').attributes('data-step')).toBe('mesh'));
  } finally { wrapper.unmount(); }
});
