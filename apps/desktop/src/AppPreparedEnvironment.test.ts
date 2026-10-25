// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import App from './App.vue';
import { setLocale } from './i18n';

const events = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}));
const games = vi.hoisted(() => ({
  source: { id: 'source-game', name: 'Source world', version: '1.21.1', status: 'installed', directory: 'fixture', launch_state: 'running' },
}));

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => true,
  invoke: vi.fn(async (command: string) => {
    if (command === 'list_games') return [games.source];
    if (command === 'list_player_profiles') return { profiles: [], active_id: null };
    if (command === 'list_tasks') return [];
    return command.startsWith('list_') ? [] : null;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    events.handlers.set(event, handler);
    return () => events.handlers.delete(event);
  }),
}));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {}) }) }));
vi.mock('./theme', () => ({ readTheme: () => 'light', applyTheme: vi.fn() }));
vi.mock('./GameRunningPage.vue', () => ({ __esModule: true, default: {
  name: 'GameRunningPage', props: ['game'], emits: ['share'],
  template: '<div data-running><button @click="$emit(\'share\')">Open Mesh group</button></div>',
} }));
vi.mock('./mesh-group/MeshGroupPanel.vue', () => ({ __esModule: true, default: {
  name: 'MeshGroupPanel', props: ['gameId', 'initialInvitation'], emits: ['prepared', 'created'],
  methods: { confirmLeave: async () => true },
  template: '<div data-mesh-group :data-game-id="gameId" :data-invitation="initialInvitation"><button @click="$emit(\'prepared\', { gameId: \'prepared-game\', invitation: \'mesh-invite-owned\' })">Remember prepared invite</button><button @click="$emit(\'created\')">Show task</button></div>',
} }));

it('retains a prepared invitation through installation and joins with the installed game identity', async () => {
  setLocale('en-US');
  const originalScrollTo = window.scrollTo;
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  const wrapper = mount(App, { global: { stubs: {
    AppNavigation: true, ProfileSwitcher: true, TasksPage: true,
    LibraryHome: { emits: ['manage'], template: '<button @click="$emit(\'manage\', \'source-game\')">Manage source game</button>' },
  } } });
  try {
    await flushPromises();
    await wrapper.findAll('button').find(button => button.text() === 'Manage source game')!.trigger('click');
    await flushPromises();
    await wrapper.get('[data-running] button').trigger('click');
    await flushPromises();
    const meshPanel = wrapper.findComponent({ name: 'MeshGroupPanel' });
    expect(meshPanel.exists()).toBe(true);
    meshPanel.vm.$emit('prepared', { gameId: 'prepared-game', invitation: 'mesh-invite-owned' });
    meshPanel.vm.$emit('created');
    await flushPromises();
    await vi.waitFor(() => expect(wrapper.findComponent({ name: 'PreparedGameJoin' }).exists()).toBe(true));
    expect(wrapper.find('.tasks-workspace').exists()).toBe(true);
    expect(wrapper.findAll('button').some(button => button.text() === 'Join with prepared game')).toBe(false);

    const taskEvent = events.handlers.get('task-updated');
    const gameEvent = events.handlers.get('game-install');
    expect(taskEvent).toBeDefined();
    expect(gameEvent).toBeDefined();
    taskEvent?.({ payload: { id: 'prepared-task', game_id: 'prepared-game', game_name: 'Prepared world', title: 'Create world', kind: 'game_install', state: 'succeeded', progress: null, error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: true } });
    gameEvent?.({ payload: { id: 'prepared-game', name: 'Prepared world', version: '1.21.1', status: 'installed', directory: 'fixture/prepared' } });
    await flushPromises();
    const join = wrapper.findAll('button').find(button => button.text() === 'Join with prepared game');
    expect(join, `UI after install: ${wrapper.text()}`).toBeDefined();
    await join!.trigger('click');
    await flushPromises();
    expect(wrapper.get('[data-mesh-group]').attributes('data-game-id')).toBe('prepared-game');
    expect(wrapper.get('[data-mesh-group]').attributes('data-invitation')).toBe('mesh-invite-owned');
    const { invoke } = await import('@tauri-apps/api/core');
    expect(invoke).not.toHaveBeenCalledWith('start_game', expect.anything());
  } finally {
    wrapper.unmount(); window.scrollTo = originalScrollTo; setLocale('zh-CN'); events.handlers.clear();
  }
});
