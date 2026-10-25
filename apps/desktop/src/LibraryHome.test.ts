// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { reactive, nextTick } from 'vue';
import { ContextMenuRoot, ContextMenuContent, DropdownMenuRoot } from 'reka-ui';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import LibraryHome from './LibraryHome.vue';
import GameCard from './GameCard.vue';
import GameCardMenu from './GameCardMenu.vue';
import AppIcon from './ui/AppIcon.vue';
import { homeGames, homeLaunchState, searchHomeGames, type HomeGame } from './library-home';
import { setLocale } from './i18n';
import { savedMarketItems, type SavedMarketItem } from './market-saved';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const profileLibrary = { active_id: 'active', profiles: [
  { id: 'active', kind: 'offline' as const, name: 'Active player', uuid: 'active-uuid', note: null },
  { id: 'pinned', kind: 'microsoft' as const, name: 'Pinned player', uuid: 'pinned-uuid', note: null },
] };
function game(id: number, extra: Partial<HomeGame> = {}): HomeGame {
  return { id: `game-${id}`, name: `Game ${id}`, version: '1.21.1', directory: `C:/private/game-${id}`, status: 'installed', error: null, progress: null, ...extra };
}
beforeEach(() => { setLocale('en-US'); localStorage.clear(); savedMarketItems.value = []; vi.mocked(invoke).mockResolvedValue([]); });
afterEach(() => { setLocale('zh-CN'); });

it('features the most recently launched installed game and preserves old order for ties', () => {
  const games = [game(0), game(1, { last_launched_at: 30 }), game(2, { last_launched_at: 50 }), game(3, { status: 'failed', last_launched_at: 60 }), game(4, { last_launched_at: 30 }), game(5)];
  const home = homeGames(games);
  expect(home.featured?.id).toBe('game-2');
  expect(home.recent.map(value => value.id)).toEqual(['game-3', 'game-1', 'game-4', 'game-0']);
  expect(homeGames([game(0, { status: 'installing' }), game(1), game(2)]).featured?.id).toBe('game-1');
  expect(games.map(value => value.id)).toEqual(['game-0', 'game-1', 'game-2', 'game-3', 'game-4', 'game-5']);
  expect(searchHomeGames([game(1, { pack_name: 'Pack name' })], 'PACK')).toHaveLength(1);
  expect(homeLaunchState(game(6, {launch_state:'verifying'}))).toBe('verifying');
  expect(homeLaunchState(game(7, {launch_state:'recovery_failed'}))).toBe('recovery_failed');
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
  expect(wrapper.emitted('select')).toEqual([['game-2'], ['game-2']]);
  expect(invoke).not.toHaveBeenCalled();
  wrapper.unmount();
});

it('searches the complete library while rendering at most fifty rows and disables running launch', async () => {
  const games = Array.from({ length: 61 }, (_, id) => game(id));
  games[60] = game(60, { launch_state: 'running' });
  const wrapper = mount(LibraryHome, { props: { games, profileLibrary, starting: [], retrying: [] } });
  await wrapper.get('.home-more-games').trigger('click');
  expect(wrapper.findAll('.game-card')).toHaveLength(50);
  await wrapper.get('input[type="search"]').setValue('Game 60');
  expect(wrapper.findAll('.game-card')).toHaveLength(1);
  expect(wrapper.text()).toContain('Running');
  const launch = wrapper.get('.library-game--row[aria-label="Game 60"] .library-game-play');
  expect(launch.attributes('disabled')).toBeDefined();
  expect(launch.attributes('aria-label')).toBe('Game 60 — Running');
  expect(launch.attributes('title')).toBe('Game 60 — Running');
  expect(launch.findComponent(AppIcon).props('name')).toBe('check');
  wrapper.unmount();
});

it('shares one context host across fifty cards and creates menus only when opened', async () => {
  const wrapper = mount(LibraryHome, { props: { games: Array.from({ length: 61 }, (_, id) => game(id)), profileLibrary, starting: [], retrying: [] }, attachTo: document.body });
  try {
    await wrapper.get('.home-more-games').trigger('click');
    expect(wrapper.findAllComponents(ContextMenuRoot)).toHaveLength(1);
    expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(0);
    expect(wrapper.findAllComponents(GameCardMenu).every(menu => !menu.findComponent(DropdownMenuRoot).exists())).toBe(true);
    const row = wrapper.findAll('.library-game--row').find(row => row.attributes('aria-label') === 'Game 24')!;
    await row.get('.library-game-open').trigger('keydown', { key: 'F10', shiftKey: true }); await flushPromises();
    expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(1);
    expect(document.querySelector('[role="menu"]')?.textContent).toContain('Manage Game 24');
    await wrapper.get('input[type="search"]').setValue('Game 24'); await flushPromises();
    expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(0);
    await wrapper.get('.game-menu-trigger').trigger('keydown', { key: 'Enter' }); await flushPromises();
    expect(wrapper.findAllComponents(GameCardMenu).filter(menu => menu.findComponent(DropdownMenuRoot).exists())).toHaveLength(1);
    const action = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(item => item.textContent?.trim() === 'Manage Game 24')!;
    action.click(); await flushPromises();
    expect(wrapper.emitted('manage')).toEqual([['game-24']]);
    expect(wrapper.emitted('play')).toBeUndefined();
  } finally { wrapper.unmount(); }
});
it('keeps session recovery states visible and retries verification instead of launching', async () => {
  const wrapper = mount(LibraryHome, { props: { games: [game(0, {launch_state:'recovery_failed'}), game(1, {launch_state:'verifying'})], profileLibrary, starting: [], retrying: [] } });
  expect(wrapper.get('.library-game-state[role="alert"]').text()).toBe('Could not verify whether the game is still open.');
  const checking = wrapper.findAll('.library-game--row').find(row => row.attributes('aria-label') === 'Game 1')!;
  expect(checking.get('.library-game-state[role="status"]').text()).toBe('Checking whether the game is still open…');
  const checkingButton = checking.get('.library-game-play');
  expect(checkingButton.attributes('disabled')).toBeDefined();
  expect(checkingButton.attributes('aria-label')).toBe('Checking whether Game 1 is still open');
  expect(checkingButton.findComponent(AppIcon).props('name')).toBe('loader-circle');
  expect(checkingButton.findComponent(AppIcon).classes()).toContain('ui-busy-icon');
  await wrapper.get('.library-game-retry').trigger('click');
  expect(wrapper.emitted('retrySessionRecovery')).toEqual([['game-0']]);
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

it('keeps pinned games in Other games and opens saved market collections from home', async () => {
  savedMarketItems.value = [
    { collection: 'favorite', provider: 'modrinth', kind: 'modpack', project_id: 'favorite-one', title: 'Favorite pack', hit: { project_id: 'favorite-one' } } as SavedMarketItem,
    { collection: 'later', provider: 'modrinth', kind: 'modpack', project_id: 'later-one', title: 'Later pack', hit: { project_id: 'later-one' } } as SavedMarketItem,
  ];
  const wrapper = mount(LibraryHome, { props: { games: [game(0, { last_launched_at: 100 }), game(1, { pinned: true }), game(2)], profileLibrary, starting: [], retrying: [] } });
  expect(wrapper.get('.home-recent .library-game--row').attributes('aria-label')).toBe('Game 1');
  expect(wrapper.findAll('.home-saved-market-actions button')).toHaveLength(2);
  await wrapper.findAll('.home-saved-market-actions button')[0]!.trigger('click');
  expect(wrapper.emitted('savedMarket')).toEqual([['favorite']]);
  wrapper.unmount();
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

it('moves the existing featured cover crop without changing the image', async () => {
  const cover_ref = 'c'.repeat(64);
  vi.mocked(invoke).mockResolvedValue([{ id: 'game-0', cover_ref, data_url: 'data:image/png;base64,AA==' }]);
  const wrapper = mount(LibraryHome, { props: { games: [game(0, { cover_ref })], profileLibrary, starting: [], retrying: [] } });
  await flushPromises();
  expect(wrapper.find('.library-cover-focus').exists()).toBe(false);
  wrapper.getComponent(GameCardMenu).vm.$emit('adjustCover'); await nextTick();
  const sliders = wrapper.findAll('.library-cover-focus input[type="range"]');
  await sliders[0]!.setValue('30');
  await sliders[1]!.setValue('75');
  expect(wrapper.get('.library-game-backdrop').attributes('style')).toContain('30% 75%');
  expect(wrapper.get('.library-game-backdrop').attributes('src')).toBe('data:image/png;base64,AA==');
  expect(JSON.parse(localStorage.getItem('cobble.cover-focus.v1')!)['game-0']).toEqual({ coverRef: cover_ref, x: 30, y: 75 });
  await wrapper.findAll('.library-cover-focus button').find(button => button.text() === 'Done')!.trigger('click');
  expect(wrapper.find('.library-cover-focus').exists()).toBe(false);
  wrapper.unmount();
});

it('opens details from a card surface while keeping launch and nested controls separate', async () => {
  const wrapper = mount(GameCard, {props: {game: game(0), profile: profileLibrary.profiles[0]!, starting: false, retrying: false}});
  await wrapper.get('.library-game-version').trigger('click');
  expect(wrapper.emitted('manage')).toEqual([['game-0']]);
  expect(wrapper.emitted('play')).toBeUndefined();
  await wrapper.get('.library-game-play').trigger('click');
  expect(wrapper.emitted('play')).toEqual([['game-0']]);
  expect(wrapper.emitted('manage')).toHaveLength(1);
  await wrapper.get('.library-game-open').trigger('click');
  expect(wrapper.emitted('manage')).toHaveLength(2);
  wrapper.unmount();
});

it('exposes cover positioning in the existing menu without a persistent card editor', async () => {
  const wrapper = mount(GameCard, {attachTo: document.body, props: {game: game(0, {cover_ref: 'cover'}), coverUrl: 'data:image/png;base64,AA==', profile: profileLibrary.profiles[0]!, starting: false, retrying: false}});
  expect(wrapper.find('.library-cover-focus').exists()).toBe(false);
  await wrapper.get('.game-menu-trigger').trigger('keydown', {key: 'Enter'}); await flushPromises();
  const action = [...document.querySelectorAll<HTMLElement>('[role=menuitem]')].find(item => item.textContent?.trim() === 'Adjust cover')!;
  expect(action).toBeDefined(); action.click(); await flushPromises();
  expect(wrapper.find('.library-cover-focus').exists()).toBe(true);
  expect(wrapper.emitted('manage')).toBeUndefined(); expect(wrapper.emitted('play')).toBeUndefined();
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
    expect(labels()).toEqual(['Manage Game 0', 'Create desktop shortcut', 'Unpin game', 'Choose cover…', 'Remove cover']);
    (document.querySelector('[role="menuitem"]') as HTMLElement).click();
    await flushPromises();
    expect(wrapper.emitted('manage')).toEqual([['game-0']]);
    expect(wrapper.emitted('play')).toBeUndefined();
    await wrapper.get('.game-menu-trigger').trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(labels()).toEqual(['Manage Game 0', 'Create desktop shortcut', 'Unpin game', 'Choose cover…', 'Remove cover']);
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

it('applies batch groups and tags, archives games, and restores them from archived results', async () => {
  const wrapper = mount(LibraryHome, { props: { games: [game(0), game(1)], profileLibrary, starting: [], retrying: [] } });
  await wrapper.setProps({ editHome: true });
  await wrapper.get('.home-organize').trigger('click');
  await wrapper.get('input[aria-label="Select Game 0"]').setValue(true);
  await wrapper.get('input[aria-label="Group"]').setValue('Friends');
  await wrapper.get('input[aria-label="Tags"]').setValue('Survival, Co-op');
  await wrapper.get('.home-batch .secondary-button').trigger('click');
  expect(JSON.parse(localStorage.getItem('cobble.library.organization.v1')!)['game-0']).toEqual({ group: 'Friends', tags: ['Survival', 'Co-op'], archived: false });
  await wrapper.get('input[aria-label="Select Game 0"]').setValue(true);
  await wrapper.get('input[aria-label="Tags"]').setValue('Building');
  await wrapper.get('.home-batch .secondary-button').trigger('click');
  expect(JSON.parse(localStorage.getItem('cobble.library.organization.v1')!)['game-0']).toEqual({ group: 'Friends', tags: ['Building'], archived: false });
  await wrapper.get('input[aria-label="Select Game 0"]').setValue(true);
  await wrapper.findAll('.home-batch .secondary-button')[1]!.trigger('click');
  expect(wrapper.findAll('.game-card')).toHaveLength(1);
  await wrapper.get('.home-filters input[type="checkbox"]').setValue(true);
  expect(wrapper.findAll('.game-card')).toHaveLength(1);
  expect(wrapper.get('.game-card').attributes('aria-label')).toBe('Game 0');
  wrapper.unmount();
});

it('reorders home sections through accessible controls and runs only an available saved quick action', async () => {
  localStorage.setItem('cobble.home.layout.v1', JSON.stringify({ order: ['other', 'quick', 'favorites'], hidden: [], quickIds: ['navigation.tools', 'plugin:missing:open'] }));
  const run = vi.fn();
  const wrapper = mount(LibraryHome, { props: { games: [game(0), game(1)], profileLibrary, starting: [], retrying: [], quickActions: [{ id: 'navigation.tools', label: 'Open tools', run }] } });
  expect(wrapper.findAll('.home-quick-actions button.secondary-button')).toHaveLength(1);
  await wrapper.get('.home-quick-actions button.secondary-button').trigger('click');
  expect(run).toHaveBeenCalledOnce();
  await wrapper.setProps({ editHome: true });
  await wrapper.get('button[aria-label="Move Other games down"]').trigger('click');
  expect(JSON.parse(localStorage.getItem('cobble.home.layout.v1')!).order).toEqual(['quick', 'other', 'favorites']);
  await wrapper.get('button[aria-label="Move Open tools up"]').trigger('click');
  expect(wrapper.get('button[aria-label="Move Open tools up"]').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('bounds the quick-action editor for a large registry and searches the rest', async () => {
  const quickActions = Array.from({ length: 1000 }, (_, index) => ({ id: `tool-${index}`, label: `Action ${index}`, run: vi.fn() }));
  const wrapper = mount(LibraryHome, { props: { games: [game(0)], profileLibrary, starting: [], retrying: [], quickActions } });
  await wrapper.setProps({ editHome: true });
  expect(wrapper.findAll('.home-quick-candidate')).toHaveLength(20);
  await wrapper.get('input[aria-label="Find quick action"]').setValue('Action 999');
  expect(wrapper.findAll('.home-quick-candidate')).toHaveLength(1);
  expect(wrapper.get('.home-quick-candidate').text()).toContain('Action 999');
  wrapper.unmount();
});

it('keeps unavailable quick IDs while loading and lets the user release their slots', async () => {
  localStorage.setItem('cobble.home.layout.v1', JSON.stringify({ order: ['other', 'quick', 'favorites'], hidden: [], quickIds: ['navigation.tools', 'plugin:missing:1', 'plugin:missing:2', 'plugin:missing:3', 'plugin:missing:4', 'plugin:missing:5'] }));
  const quickActions = [{ id: 'navigation.tools', label: 'Tools', run: vi.fn() }, { id: 'navigation.library', label: 'Games', run: vi.fn() }];
  const wrapper = mount(LibraryHome, { props: { games: [game(0)], profileLibrary, starting: [], retrying: [], quickActions, quickActionsLoading: true } });
  await wrapper.setProps({ editHome: true });
  expect(wrapper.find('.home-quick-remove').exists()).toBe(false);
  expect(wrapper.get('[role="switch"][aria-label="Games"]').attributes('data-disabled')).toBeDefined();
  await wrapper.setProps({ quickActionsLoading: false });
  await wrapper.get('.home-quick-remove').trigger('click');
  expect(JSON.parse(localStorage.getItem('cobble.home.layout.v1')!).quickIds).toEqual(['navigation.tools']);
  await wrapper.get('[role="switch"][aria-label="Games"]').trigger('click');
  expect(JSON.parse(localStorage.getItem('cobble.home.layout.v1')!).quickIds).toEqual(['navigation.tools', 'navigation.library']);
  wrapper.unmount();
});

it('guards quick dispatch and reports a failed action', async () => {
  localStorage.setItem('cobble.home.layout.v1', JSON.stringify({ order: ['quick', 'other', 'favorites'], hidden: [], quickIds: ['navigation.tools'] }));
  let reject!: (reason: Error) => void;
  const run = vi.fn(() => new Promise<void>((_, no) => { reject = no; }));
  const wrapper = mount(LibraryHome, { props: { games: [game(0)], profileLibrary, starting: [], retrying: [], quickActions: [{ id: 'navigation.tools', label: 'Open tools', run }] } });
  await wrapper.get('.home-quick-actions .secondary-button').trigger('click');
  expect(wrapper.get('.home-quick-actions .secondary-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.home-quick-actions .secondary-button').trigger('click');
  expect(run).toHaveBeenCalledOnce();
  reject(new Error('private details'));
  await flushPromises();
  expect(wrapper.get('.home-quick-error').text()).toBe('Could not open action');
  expect(wrapper.text()).not.toContain('private details');
  wrapper.unmount();
});
