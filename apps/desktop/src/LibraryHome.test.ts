// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { reactive, nextTick } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import LibraryHome from './LibraryHome.vue';
import GameCard from './GameCard.vue';
import { homeGames, searchHomeGames, type HomeGame } from './library-home';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const profileLibrary = { active_id: 'active', profiles: [
  { id: 'active', kind: 'offline' as const, name: 'Active player', uuid: 'active-uuid', note: null },
  { id: 'pinned', kind: 'microsoft' as const, name: 'Pinned player', uuid: 'pinned-uuid', note: null },
] };
function game(id: number, extra: Partial<HomeGame> = {}): HomeGame {
  return { id: `game-${id}`, name: `Game ${id}`, version: '1.21.1', directory: `C:/private/game-${id}`, status: 'installed', error: null, progress: null, ...extra };
}
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockResolvedValue([]); });
afterEach(() => { setLocale('zh-CN'); });

it('features the most recently launched installed game and preserves old order for ties', () => {
  const games = [game(0), game(1, { last_launched_at: 30 }), game(2, { last_launched_at: 50 }), game(3, { status: 'failed', last_launched_at: 60 }), game(4, { last_launched_at: 30 }), game(5)];
  const home = homeGames(games);
  expect(home.featured?.id).toBe('game-2');
  expect(home.recent.map(value => value.id)).toEqual(['game-3', 'game-1', 'game-4', 'game-0']);
  expect(homeGames([game(0, { status: 'installing' }), game(1), game(2)]).featured?.id).toBe('game-1');
  expect(games.map(value => value.id)).toEqual(['game-0', 'game-1', 'game-2', 'game-3', 'game-4', 'game-5']);
  expect(searchHomeGames([game(1, { pack_name: 'Pack name' })], 'PACK')).toHaveLength(1);
});

it('renders one featured game and four distinct rows using the effective player', async () => {
  const games = Array.from({ length: 7 }, (_, id) => game(id));
  games[2] = game(2, { last_launched_at: 100, profile_id: 'pinned' });
  const wrapper = mount(LibraryHome, { props: { games, profileLibrary, starting: [], retrying: [] } });
  expect(wrapper.findAll('.game-card')).toHaveLength(5);
  expect(wrapper.get('.library-game--featured').attributes('aria-label')).toBe('Game 2');
  expect(wrapper.get('.library-game--featured').text()).toContain('Pinned player');
  expect(wrapper.findAll('.library-game--row').map(value => value.attributes('aria-label'))).not.toContain('Game 2');
  expect(wrapper.text()).not.toContain('C:/private');
  await wrapper.get('button[aria-label="Launch Game 2"]').trigger('click');
  await wrapper.get('button[aria-label="Manage Game 2"]').trigger('click');
  expect(wrapper.emitted('play')).toEqual([['game-2']]);
  expect(wrapper.emitted('manage')).toEqual([['game-2']]);
  expect(invoke).not.toHaveBeenCalled();
  wrapper.unmount();
});

it('searches the complete library while rendering at most fifty rows and disables running launch', async () => {
  const games = Array.from({ length: 61 }, (_, id) => game(id));
  games[60] = game(60, { launch_state: 'running' });
  const wrapper = mount(LibraryHome, { props: { games, profileLibrary, starting: [], retrying: [] } });
  await wrapper.get('.home-view-toggle').trigger('click');
  expect(wrapper.findAll('.game-card')).toHaveLength(50);
  await wrapper.get('input[type="search"]').setValue('Game 60');
  expect(wrapper.findAll('.game-card')).toHaveLength(1);
  expect(wrapper.text()).toContain('Running');
  expect(wrapper.get('button[aria-label="Launch Game 60"]').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('keeps empty creation primary and install failures compact without leaking filesystem errors', async () => {
  const empty = mount(LibraryHome, { props: { games: [], profileLibrary, starting: [], retrying: [] } });
  await empty.get('.home-empty .primary-button').trigger('click');
  expect(empty.emitted('create')).toEqual([[]]);
  empty.unmount();
  const card = mount(GameCard, { props: { game: game(1, { status: 'failed', error: 'File C:/private/world failed' }), profile: null, starting: false, retrying: false } });
  expect(card.get('[role="alert"]').text()).toBe('Installation incomplete');
  expect(card.text()).not.toContain('C:/private');
  await card.get('.library-game-retry').trigger('click');
  expect(card.emitted('retry')).toEqual([['game-1']]);
  await card.setProps({ game: game(1, { status: 'installing', progress: { phase: 'assets', completed: 2, total: 10, reused: 0 } }) });
  expect(card.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('20');
  card.unmount();
});

it('requests only visible custom icons in one batch and falls back after image failure', async () => {
  const icon_ref = 'a'.repeat(64);
  vi.mocked(invoke).mockResolvedValue([{ id: 'game-0', icon_ref, data_url: 'data:image/png;base64,AA==' }]);
  const wrapper = mount(LibraryHome, { props: { games: [game(0, { icon_ref })], profileLibrary, starting: [], retrying: [] } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_game_icons', { ids: ['game-0'] });
  expect(wrapper.get('.library-game-cover img').attributes('src')).toBe('data:image/png;base64,AA==');
  await wrapper.get('.library-game-cover img').trigger('error');
  expect(wrapper.find('.library-game-cover img').exists()).toBe(false);
  expect(wrapper.find('.library-game-cover svg').exists()).toBe(true);
  wrapper.unmount();
});

it('keeps the last launched game featured while pins prioritize only the other rows', () => {
  const result = homeGames([game(0, { last_launched_at: 300 }), game(1, { last_launched_at: 200 }), game(2, { pinned: true })]);
  expect(result.featured?.id).toBe('game-0');
  expect(result.recent.map(value => value.id)).toEqual(['game-2', 'game-1']);
});

it('loads only the featured cover and preserves the game when a cover fails', async () => {
  const cover_ref = 'c'.repeat(64);
  vi.mocked(invoke).mockResolvedValue([{ id: 'game-0', cover_ref, data_url: 'data:image/png;base64,AA==' }]);
  const wrapper = mount(LibraryHome, { props: { games: [game(0, { cover_ref, last_launched_at: 20 }), game(1, { cover_ref: 'd'.repeat(64) })], profileLibrary, starting: [], retrying: [] } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_game_covers', { ids: ['game-0'] });
  expect(wrapper.findAll('.library-game-backdrop')).toHaveLength(1);
  await wrapper.get('.library-game-backdrop').trigger('error');
  expect(wrapper.find('.library-game-backdrop').exists()).toBe(false);
  expect(wrapper.get('.library-game--featured').text()).toContain('Game 0');
  wrapper.unmount();
});

it('does not let a local pin result hide subsequent native launch status', async () => {
  const games = reactive([game(0)]);
  const wrapper = mount(LibraryHome, { props: { games, profileLibrary, starting: [], retrying: [] } });
  wrapper.getComponent(GameCard).vm.$emit('appearance', game(0, { pinned: true }));
  await nextTick();
  games[0] = game(0, { launch_state: 'running' });
  await nextTick();
  expect(wrapper.get('button[aria-label="Launch Game 0"]').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.library-game--featured').text()).toContain('Running');
  wrapper.unmount();
});

it('shares game actions between right click and the ellipsis without launching', async () => {
  const wrapper = mount(GameCard, { attachTo: document.body, props: { game: game(0, { pinned: true, cover_ref: 'cover' }), profile: profileLibrary.profiles[0]!, starting: false, retrying: false } });
  try {
    expect(wrapper.find('.library-game-manage').exists()).toBe(false);
    expect(wrapper.get('.library-game-pinned').attributes('aria-label')).toBe('Pinned');
    expect(wrapper.get('.library-game-pinned').attributes('title')).toBe('Pinned');
    expect(wrapper.get('.library-game-pinned').text()).toBe('');
    await wrapper.get('article').trigger('contextmenu', { button: 2, clientX: 100, clientY: 80 });
    await flushPromises();
    const labels = () => Array.from(document.querySelectorAll('[role="menuitem"]')).map(item => item.textContent?.trim());
    expect(labels()).toEqual(['Manage Game 0', 'Unpin game', 'Choose cover…', 'Remove cover']);
    (document.querySelector('[role="menuitem"]') as HTMLElement).click();
    await flushPromises();
    expect(wrapper.emitted('manage')).toEqual([['game-0']]);
    expect(wrapper.emitted('play')).toBeUndefined();
    await wrapper.get('.game-menu-trigger').trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(labels()).toEqual(['Manage Game 0', 'Unpin game', 'Choose cover…', 'Remove cover']);
  } finally { wrapper.unmount(); }
});

it('opens the game context menu from the keyboard without activating play', async () => {
  const wrapper = mount(GameCard, { attachTo: document.body, props: { game: game(0), profile: profileLibrary.profiles[0]!, starting: false, retrying: false } });
  try {
    await wrapper.get('.library-game-open').trigger('keydown', { key: 'F10', shiftKey: true });
    await flushPromises();
    expect(document.querySelector('[role="menu"]')).not.toBeNull();
    expect(wrapper.emitted('play')).toBeUndefined();
    expect(wrapper.find('button button').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});
