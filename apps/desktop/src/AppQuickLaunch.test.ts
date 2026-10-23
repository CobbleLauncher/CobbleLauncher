// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import App from './App.vue';
import { setLocale } from './i18n';
import type { Game } from './games';

const eventHandlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    eventHandlers.set(name, handler);
    return () => eventHandlers.delete(name);
  }),
}));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {}) }) }));
vi.mock('./theme', () => ({ readTheme: () => 'light', applyTheme: vi.fn() }));

vi.mock('./GameDetailsPage.vue', () => ({ __esModule: true, default: {
  name: 'GameDetailsPage', props: ['game', 'active', 'library', 'profile', 'choosing'], emits: ['server-play'],
  methods: { confirmLeave: async () => true, refreshContent: async () => {} },
  template: '<section data-game-details><button data-server-play @click="$emit(\'server-play\', game.id, \'server-1\')">Join server</button></section>',
} }));
vi.mock('./ProfilesPage.vue', () => ({ __esModule: true, default: {
  name: 'ProfilesPage', props: ['choosing'], emits: ['select'],
  template: '<section data-profiles :data-choosing="choosing"><button data-select-profile @click="$emit(\'select\', \'profile-1\')">Select profile</button></section>',
} }));

const game: Game = { id: 'game-1', name: 'Game One', version: '1.21.1', directory: 'game-dir', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const profile = { id: 'profile-1', kind: 'offline', name: 'Player', uuid: 'uuid-1', note: null };
const profiles = { profiles: [profile], active_id: 'profile-1' };
const intent = { queue_id: 'queue-1', game_id: 'game-1', play: true };

function mountApp() {
  return mount(App, { global: { stubs: {
    AppNavigation: true,
    ProfileSwitcher: true,
    LibraryHome: { emits: ['manage'], template: '<button data-manage @click="$emit(\'manage\', \'game-1\')">Manage</button>' },
    TasksPage: true,
    AppIcon: true,
  } } });
}
function defaultInvoke(
  currentProfiles: typeof profiles | { profiles: []; active_id: null } = profiles,
  pending: typeof intent | null = intent,
  currentGame = game,
) {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_games') return [currentGame];
    if (command === 'list_player_profiles') return currentProfiles;
    if (command === 'select_player_profile') return profiles;
    if (command === 'set_game_profile') return { ...currentGame, profile_id: 'profile-1' };
    if (command === 'peek_pending_launch') return pending;
    if (command === 'acknowledge_pending_launch') return true;
    if (command === 'start_game' || command === 'start_game_server') return { ...game, launch_state: 'preparing' };
    if (command === 'list_tasks') return [];
    return null;
  });
}
afterEach(() => { vi.mocked(invoke).mockReset(); eventHandlers.clear(); setLocale('zh-CN'); });

it('waits for game and profile lists, then starts a cold queued play intent once', async () => {
  setLocale('en-US');
  let resolveGames!: (value: typeof game[]) => void;
  let resolveProfiles!: (value: typeof profiles) => void;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_games') return new Promise(resolve => { resolveGames = resolve; });
    if (command === 'list_player_profiles') return new Promise(resolve => { resolveProfiles = resolve; });
    if (command === 'peek_pending_launch') return intent;
    if (command === 'acknowledge_pending_launch') return true;
    if (command === 'start_game') return { ...game, launch_state: 'preparing' };
    if (command === 'list_tasks') return [];
    return null;
  });
  const wrapper = mountApp();
  try {
    await flushPromises();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'peek_pending_launch')).toBe(false);
    resolveGames([game]);
    resolveProfiles(profiles);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('start_game', { id: 'game-1' }));
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'start_game')).toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith('acknowledge_pending_launch', { queueId: 'queue-1' });
  } finally { wrapper.unmount(); }
});

it('continues a queued play after profile selection when the game has no effective profile', async () => {
  setLocale('en-US');
  defaultInvoke(profiles, intent, { ...game, profile_id: 'missing-profile' });
  const wrapper = mountApp();
  try {
    await vi.waitFor(() => expect(wrapper.find('[data-profiles]').exists()).toBe(true));
    expect(wrapper.get('[data-profiles]').attributes('data-choosing')).toBe('true');
    await wrapper.get('[data-select-profile]').trigger('click');
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('start_game', { id: 'game-1' }));
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'start_game')).toHaveLength(1);
  } finally { wrapper.unmount(); }
});

it('retains the server id while resuming a server play after profile selection', async () => {
  setLocale('en-US');
  defaultInvoke(profiles, null, { ...game, profile_id: 'missing-profile' });
  const wrapper = mountApp();
  try {
    await flushPromises();
    await wrapper.get('[data-manage]').trigger('click');
    await vi.waitFor(() => expect(wrapper.find('[data-game-details]').exists()).toBe(true));
    await wrapper.get('[data-server-play]').trigger('click');
    await vi.waitFor(() => expect(wrapper.find('[data-profiles]').exists()).toBe(true));
    await wrapper.get('[data-select-profile]').trigger('click');
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('start_game_server', { id: 'game-1', serverId: 'server-1' }));
  } finally { wrapper.unmount(); }
});
