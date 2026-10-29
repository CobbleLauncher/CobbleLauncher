// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import HomeDashboard from './HomeDashboard.vue';
import GameCard from '../GameCard.vue';
import type { HomeGame } from '../library-home';
import { setLocale } from '../i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue([]) }));
vi.mock('./HomeContextPanel.vue', () => ({ default: { template: '<div />' } }));
const games: HomeGame[] = Array.from({ length: 6 }, (_, id) => ({ id: `game-${id}`, name: `Game ${id}`, version: '1.21.1', directory: `C:/games/${id}`, status: 'installed', error: null, progress: null }));
const profile = { id: 'player', kind: 'offline' as const, name: 'Player', uuid: 'player-id', note: null };
const base = () => ({ game: games[0]!, games, profile, active: true, starting: false, retrying: false, icons: new Map<string, string>(), commands: [] });
beforeEach(() => { setLocale('en-US'); localStorage.clear(); });

it('reaches the complete recent strip with arrow keys and keeps one tab stop', async () => {
  const wrapper = mount(HomeDashboard, { attachTo: document.body, props: base() });
  try {
    const buttons = wrapper.findAll('.home-game-switcher button');
    expect(buttons).toHaveLength(6);
    expect(buttons.map(button => button.attributes('tabindex'))).toEqual(['0', '-1', '-1', '-1', '-1', '-1']);
    await buttons[0]!.trigger('keydown', { key: 'End' });
    expect(wrapper.emitted('select')).toEqual([['game-5']]);
    expect(document.activeElement).toBe(buttons[5]!.element);
    await wrapper.setProps({ game: games[5]! });
    expect(buttons[5]!.attributes('aria-pressed')).toBe('true');
    expect(buttons[5]!.attributes('tabindex')).toBe('0');
    await buttons[5]!.trigger('keydown', { key: 'ArrowLeft' });
    await buttons[4]!.trigger('keydown', { key: 'Home' });
    expect(wrapper.emitted('select')).toEqual([['game-5'], ['game-4'], ['game-0']]);
    expect(wrapper.emitted('play')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('converts vertical wheel movement only while the strip can consume it', () => {
  const wrapper = mount(HomeDashboard, { props: base() });
  try {
    const strip = wrapper.get('.home-game-switcher').element as HTMLElement;
    Object.defineProperties(strip, { clientWidth: { value: 240 }, scrollWidth: { value: 600 } });
    const wheel = (deltaY: number, extra: WheelEventInit = {}) => {
      const event = new WheelEvent('wheel', { deltaY, bubbles: true, cancelable: true, ...extra });
      strip.dispatchEvent(event); return event;
    };
    expect(wheel(120).defaultPrevented).toBe(true);
    expect(strip.scrollLeft).toBe(120);
    expect(wheel(500).defaultPrevented).toBe(true);
    expect(strip.scrollLeft).toBe(360);
    expect(wheel(120).defaultPrevented).toBe(false);
    expect(wheel(-120, { ctrlKey: true }).defaultPrevented).toBe(false);
    expect(wheel(-120, { deltaX: 180 }).defaultPrevented).toBe(false);
    expect(wheel(-3, { deltaMode: 1 }).defaultPrevented).toBe(true);
    expect(strip.scrollLeft).toBe(288);
  } finally { wrapper.unmount(); }
});

it('reveals the selected recent game when returning to the dashboard', async () => {
  const wrapper = mount(HomeDashboard, { props: { ...base(), game: games[5]! } });
  try {
    const reveal = vi.fn();
    wrapper.get('.home-game-switcher button[aria-pressed="true"]').element.scrollIntoView = reveal;
    await flushPromises();
    expect(reveal).toHaveBeenCalledWith({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  } finally { wrapper.unmount(); }
});

it('keeps the live launch button while title and cover transition to a different game', async () => {
  const wrapper = mount(HomeDashboard, { props: { ...base(), coverUrl: 'data:image/png;base64,AA==' }, global: { stubs: { transition: false } } });
  try {
    const launch = wrapper.get('.library-game-play').element;
    await wrapper.setProps({ game: games[5]!, coverUrl: 'data:image/png;base64,AQ==' });
    expect(wrapper.get('.library-game-play').element).toBe(launch);
    expect(wrapper.get('.library-game-play').attributes('aria-label')).toBe('Launch Game 5');
    expect(wrapper.get('.library-game-open').attributes('aria-label')).toBe('Manage Game 5');
    await wrapper.get('.library-game-play').trigger('click');
    await wrapper.get('.library-game-open').trigger('click');
    expect(wrapper.emitted('play')).toEqual([['game-5']]);
    expect(wrapper.emitted('manage')).toEqual([['game-5']]);
    expect(wrapper.findAllComponents(GameCard)).toHaveLength(1);
    expect(wrapper.findAll('.home-launch-art-layer button')).toHaveLength(0);
    await wrapper.setProps({ game: { ...games[4]!, launch_state: 'running' } });
    expect(wrapper.get('.library-game-play').element).toBe(launch);
    expect(wrapper.get('.library-game-play').attributes('disabled')).toBeDefined();
  } finally { wrapper.unmount(); }
});

it('closes the old game menu when selection changes', async () => {
  const wrapper = mount(HomeDashboard, { attachTo: document.body, props: base() });
  try {
    await wrapper.get('.game-menu-trigger').trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(document.querySelector('[role="menu"]')?.textContent).toContain('Manage Game 0');
    await wrapper.setProps({ game: games[1]! });
    await flushPromises();
    expect(document.querySelector('[role="menu"]')).toBeNull();
    await wrapper.get('.game-menu-trigger').trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(document.querySelector('[role="menu"]')?.textContent).toContain('Manage Game 1');
    expect(wrapper.emitted('play')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('shows actual custom icons and falls back to the selected preset after an image failure', async () => {
  const wrapper = mount(HomeDashboard, { props: { ...base(), games: [{ ...games[0]!, icon_ref: 'icon', appearance_preset: 'water' }, games[1]!], icons: new Map([['icon', 'data:image/png;base64,AA==']]) } });
  try {
    const icon = wrapper.get('.home-game-switcher .home-instance-icon');
    expect(icon.get('img').attributes('src')).toBe('data:image/png;base64,AA==');
    await icon.get('img').trigger('error');
    expect(icon.find('img').exists()).toBe(false);
    expect(icon.attributes('data-preset')).toBe('water');
    expect(icon.find('svg .block-top').exists()).toBe(true);
  } finally { wrapper.unmount(); }
});

it('opens and cancels the internal selector without replacing the launch card or selection', async () => {
  const wrapper = mount(HomeDashboard, { attachTo: document.body, props: base() });
  try {
    const launch = wrapper.get('.library-game-play').element;
    const trigger = wrapper.get('.home-more-games');
    await trigger.trigger('click');
    expect(wrapper.find('.home-game-archive').exists()).toBe(true);
    expect(wrapper.get('.home-launch-front').attributes('inert')).toBeDefined();
    expect(wrapper.get('.library-game-play').element).toBe(launch);
    await wrapper.get('.home-game-archive input').setValue('Game 5');
    await wrapper.get('.home-game-archive input').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(wrapper.find('.home-game-archive').exists()).toBe(false);
    expect(wrapper.get('.home-launch-front').attributes('inert')).toBeUndefined();
    expect(document.activeElement).toBe(trigger.element);
    expect(wrapper.emitted('select')).toBeUndefined();
    expect(wrapper.emitted('play')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('commits archive selection once, closes, and keeps library organization separate', async () => {
  const extra = { ...games[0]!, id: 'game-12', name: 'Game 12' };
  const wrapper = mount(HomeDashboard, { props: { ...base(), archiveGames: [...games, extra] } });
  try {
    await wrapper.get('.home-more-games').trigger('click');
    await wrapper.get('.home-game-archive input').setValue('Game 12');
    await wrapper.get('.home-archive-card').trigger('click');
    expect(wrapper.find('.home-game-archive').exists()).toBe(false);
    expect(wrapper.emitted('select')).toEqual([['game-12']]);
    expect(wrapper.emitted('play')).toBeUndefined();
    await wrapper.get('.home-manage-library').trigger('click');
    expect(wrapper.emitted('library')).toEqual([[]]);
  } finally { wrapper.unmount(); }
});
