// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import App from './App.vue';
import { setLocale } from './i18n';

const taskEvents = vi.hoisted(() => ({ update: undefined as ((event: { payload: unknown }) => void) | undefined }));
vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => true,
  invoke: vi.fn(async (name: string) => {
    if (name === 'list_games') return [{ id: 'one', name: 'First game', version: '1.21.1', status: 'installed', directory: 'fixture' }];
    if (name === 'list_player_profiles') return { profiles: [], active_id: null };
    return name.startsWith('list_') ? [] : null;
  }),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => { if (event === 'task-updated') taskEvents.update = handler; return () => {}; }) }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {}) }) }));
vi.mock('./theme', () => ({ readTheme: () => 'light', applyTheme: vi.fn() }));
vi.mock('./GameDetailsPage.vue', () => ({ __esModule: true, default: {
  name: 'GameDetailsPage', props: ['active'], emits: ['market'],
  data: () => ({ search: '' }),
  methods: { confirmLeave: async () => true, refreshContent: async () => {} },
  template: '<div data-details><h1>First game</h1><input aria-label="Retained search" v-model="search"><button @click="$emit(\'market\', \'mod\')">Find content</button></div>',
} }));
vi.mock('./MarketPage.vue', () => ({ __esModule: true, default: { name: 'MarketPage', template: '<div data-market>Market</div>' } }));

it('retains game workspace input and restores its scroll when returning from the market', async () => {
  setLocale('en-US');
  let scroll = 0;
  const oldScroll = Object.getOwnPropertyDescriptor(window, 'scrollY');
  Object.defineProperty(window, 'scrollY', { configurable: true, get: () => scroll });
  window.scrollTo = vi.fn((_x: unknown, y: unknown) => { scroll = Number(y); }) as unknown as typeof window.scrollTo;
  const wrapper = mount(App, { global: { stubs: {
    AppNavigation: true, ProfileSwitcher: true, TasksPage: true,
    LibraryHome: { emits: ['manage'], template: '<button @click="$emit(\'manage\', \'one\')">Manage first</button>' },
  } } });
  try {
    await flushPromises();
    await wrapper.findAll('button').find(button => button.text() === 'Manage first')!.trigger('click');
    await vi.waitFor(() => expect(wrapper.find('[data-details]').exists()).toBe(true));
    await wrapper.get('input[aria-label="Retained search"]').setValue('Sodium');
    scroll = 640;
    await wrapper.findAll('button').find(button => button.text() === 'Find content')!.trigger('click');
    await vi.waitFor(() => expect(wrapper.find('[data-market]').exists()).toBe(true));
    expect(wrapper.get('[data-details]').isVisible()).toBe(false);
    await wrapper.get('.back-button').trigger('click');
    await flushPromises();
    expect((wrapper.get('[data-details]').element as HTMLElement).style.display).toBe('');
    expect((wrapper.get('input[aria-label="Retained search"]').element as HTMLInputElement).value).toBe('Sodium');
    expect(window.scrollTo).toHaveBeenLastCalledWith(0, 640);
  } finally {
    wrapper.unmount(); setLocale('zh-CN');
    if (oldScroll) Object.defineProperty(window, 'scrollY', oldScroll);
  }
});

it('keeps Mesh and Tasks in the titlebar when idle and only badges active tasks', async () => {
  setLocale('en-US');
  const wrapper = mount(App, { global: { stubs: {
    AppNavigation: true, ProfileSwitcher: true,
    TasksPage: { template: '<section data-tasks-history />' },
  } } });
  try {
    await flushPromises();
    expect(wrapper.find('.titlebar .tasks-button').exists()).toBe(true);
    expect(wrapper.find('.titlebar .task-count').exists()).toBe(false);
    expect(wrapper.find('.titlebar .mesh-status-button').exists()).toBe(true);
    taskEvents.update?.({ payload: {
      id: 'task-1', game_id: null, game_name: '', title: 'Download', kind: 'modpack_download',
      state: 'running', progress: null, error: null, can_retry: false, created_at: 1, updated_at: 1, has_result: false,
    } });
    await flushPromises();
    expect(wrapper.get('.titlebar .task-count').text()).toBe('1');
    await wrapper.get('.titlebar .tasks-button').trigger('click');
    expect(wrapper.get('.titlebar .tasks-button').attributes('aria-expanded')).toBe('true');
    expect(wrapper.find('[data-tasks-history]').exists()).toBe(true);
  } finally { wrapper.unmount(); setLocale('zh-CN'); taskEvents.update = undefined; }
});
