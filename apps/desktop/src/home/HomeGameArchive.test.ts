// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import HomeGameArchive from './HomeGameArchive.vue';
import type { HomeGame } from '../library-home';
import { setLocale } from '../i18n';

const games: HomeGame[] = Array.from({ length: 300 }, (_, id) => ({ id: `game-${id}`, name: `Game ${id}`, version: '1.21.1', directory: `C:/games/${id}`, status: 'installed', error: null, progress: null }));
const base = () => ({ games, selectedId: 'game-0', icons: new Map<string, string>() });
beforeEach(() => { setLocale('en-US'); });

it('bounds the rendered stack and visible resources while searching the entire library', async () => {
  const wrapper = mount(HomeGameArchive, { props: base() });
  try {
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(6);
    expect(wrapper.emitted('visible')?.[0]).toEqual([games.slice(0, 6).map(game => game.id)]);
    await wrapper.get('input[type="search"]').setValue('Game 299');
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(1);
    expect(wrapper.get('.home-archive-card').text()).toContain('Game 299');
    expect(wrapper.emitted('visible')?.at(-1)).toEqual([['game-299']]);
    expect(wrapper.emitted('select')).toBeUndefined();
    await wrapper.get('input[type="search"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('select')).toEqual([['game-299']]);
  } finally { wrapper.unmount(); }
});

it('uses arrows, Home, End and Enter across pages without committing focus changes', async () => {
  const wrapper = mount(HomeGameArchive, { attachTo: document.body, props: base() });
  try {
    await wrapper.get('input').trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get('[data-game-id="game-0"]').element);
    await wrapper.get('[data-game-id="game-0"]').trigger('keydown', { key: 'End' });
    await flushPromises();
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(6);
    expect(document.activeElement).toBe(wrapper.get('[data-game-id="game-299"]').element);
    expect(wrapper.emitted('select')).toBeUndefined();
    await wrapper.get('[data-game-id="game-299"]').trigger('keydown', { key: 'ArrowLeft' });
    await flushPromises();
    expect(document.activeElement).toBe(wrapper.get('[data-game-id="game-298"]').element);
    await wrapper.get('[data-game-id="game-298"]').trigger('keydown', { key: 'Home' });
    await flushPromises();
    await wrapper.get('[data-game-id="game-0"]').trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    await wrapper.get('[data-game-id="game-1"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('select')).toEqual([['game-1']]);
    expect(wrapper.emitted('play')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('opens on the selected game page and paginates with bounded resources', async () => {
  const wrapper = mount(HomeGameArchive, { props: { ...base(), selectedId: 'game-299' } });
  try {
    expect(wrapper.get('[aria-pressed="true"]').attributes('data-game-id')).toBe('game-299');
    expect(wrapper.get('button[aria-label="Next"]').attributes('disabled')).toBeDefined();
    await wrapper.get('button[aria-label="Previous"]').trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(6);
    expect(wrapper.get('.home-archive-card').attributes('data-game-id')).toBe('game-288');
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('reveals the deeper hovered card without changing the selected game', async () => {
  const wrapper = mount(HomeGameArchive, { props: base() });
  try {
    await wrapper.get('[data-game-id="game-4"]').trigger('pointermove', { pointerType: 'mouse' });
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-4');
    expect(wrapper.get('[aria-pressed="true"]').attributes('data-game-id')).toBe('game-0');
    await wrapper.get('.home-archive-stack').trigger('mouseleave');
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-0');
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('lets Escape cancel a search without selecting and leaves IME Escape alone', async () => {
  const wrapper = mount(HomeGameArchive, { props: base() });
  try {
    await wrapper.get('input').setValue('Game 299');
    await wrapper.get('input').trigger('keydown', { key: 'Escape', isComposing: true });
    expect(wrapper.emitted('close')).toBeUndefined();
    await wrapper.get('input').trigger('keydown', { key: 'Escape' });
    expect(wrapper.emitted('close')).toEqual([[]]);
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('reuses cached icons and covers and falls back locally after failures', async () => {
  const wrapper = mount(HomeGameArchive, { props: { ...base(), games: [{ ...games[0]!, icon_ref: 'icon', cover_ref: 'cover', appearance_preset: 'water' }], icons: new Map([['icon', 'data:image/png;base64,AA==']]), covers: new Map([['cover', 'data:image/png;base64,AQ==']]) } });
  try {
    expect(wrapper.findAll('img')).toHaveLength(2);
    await wrapper.get('.home-archive-glyph img').trigger('error');
    await wrapper.get('.home-archive-cover').trigger('error');
    expect(wrapper.findAll('img')).toHaveLength(0);
    expect(wrapper.get('.home-archive-card').attributes('data-preset')).toBe('water');
    expect(wrapper.find('.home-archive-glyph svg').exists()).toBe(true);
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('reports no results and keeps back and library actions available', async () => {
  const wrapper = mount(HomeGameArchive, { props: base() });
  try {
    await wrapper.get('input').setValue('no-such-game');
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(0);
    expect(wrapper.get('.home-archive-empty[role="status"]').text()).toBe('No games found');
    await wrapper.get('input').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('select')).toBeUndefined();
    await wrapper.get('.home-archive-back').trigger('click');
    await wrapper.get('.home-archive-manage-library').trigger('click');
    expect(wrapper.emitted('close')).toEqual([[]]);
    expect(wrapper.emitted('library')).toEqual([[]]);
  } finally { wrapper.unmount(); }
});


it('accumulates trackpad deltas and rate limits browsing without selecting a game', async () => {
  const clock = vi.spyOn(performance, 'now').mockReturnValue(300);
  const wrapper = mount(HomeGameArchive, { props: base() });
  try {
    const stack = wrapper.get('.home-archive-stack');
    await stack.trigger('wheel', { deltaY: 12 });
    await stack.trigger('wheel', { deltaY: 12 });
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-0');
    await stack.trigger('wheel', { deltaY: 12 });
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-1');
    await stack.trigger('wheel', { deltaY: 120 });
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-1');
    clock.mockReturnValue(500);
    await stack.trigger('wheel', { deltaY: -3, deltaMode: 1 });
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-0');
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); clock.mockRestore(); }
});

it('browses across page boundaries and confirms the preview beneath a stationary mouse', async () => {
  const wrapper = mount(HomeGameArchive, { attachTo: document.body, props: { ...base(), selectedId: 'game-5' } });
  try {
    await wrapper.get('.home-archive-stack').trigger('wheel', { deltaY: 100 });
    await flushPromises();
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-6');
    expect(wrapper.findAll('.home-archive-card')).toHaveLength(6);
    expect(wrapper.get('.home-archive-count').text()).toBe('7 / 300');
    expect(document.activeElement).toBe(wrapper.get('input').element);
    wrapper.get('[data-game-id="game-7"]').element.dispatchEvent(new MouseEvent('click', {detail: 1, bubbles: true}));
    expect(wrapper.emitted('select')).toEqual([['game-6']]);
    wrapper.get('[data-game-id="game-7"]').element.dispatchEvent(new MouseEvent('click', {detail: 0, bubbles: true}));
    expect(wrapper.emitted('select')?.at(-1)).toEqual(['game-7']);
  } finally { wrapper.unmount(); }
});

it('leaves browser zoom, reduced-motion list scrolling and end-of-library scrolling alone', async () => {
  const wrapper = mount(HomeGameArchive, { props: { ...base(), games: games.slice(0,2), selectedId: 'game-1' } });
  try {
    const stack = wrapper.get('.home-archive-stack').element;
    for (const options of [{deltaY: 100}, {deltaY: -100, ctrlKey: true}]) {
      const event = new WheelEvent('wheel', { ...options, bubbles: true, cancelable: true });
      stack.dispatchEvent(event); expect(event.defaultPrevented).toBe(false);
    }
    wrapper.element.setAttribute('data-motion', 'reduced');
    const event = new WheelEvent('wheel', {deltaY: -100, bubbles: true, cancelable: true});
    stack.dispatchEvent(event); expect(event.defaultPrevented).toBe(false);
    expect(wrapper.get('.is-highlighted').attributes('data-game-id')).toBe('game-1');
    expect(wrapper.emitted('select')).toBeUndefined();
  } finally { wrapper.unmount(); }
});

it('keeps the opened rear sheet selected across its exposed body and ignores layout-only pointer changes', async () => {
  const wrapper = mount(HomeGameArchive, { props: base() });
  const rear = wrapper.get('[data-game-id="game-4"]');
  const pointer = async (element: Element, x: number, y: number) => {
    const event = new MouseEvent('pointermove', { clientX: x, clientY: y, bubbles: true });
    Object.defineProperty(event, 'pointerType', { value: 'mouse' });
    element.dispatchEvent(event); await flushPromises();
  };
  try {
    await pointer(rear.element, 200, 100);
    expect(rear.classes()).toContain('is-highlighted');
    await pointer(wrapper.get('[data-game-id="game-0"]').element, 200, 100);
    expect(rear.classes()).toContain('is-highlighted');
    vi.spyOn(rear.get('.home-archive-face').element, 'getBoundingClientRect').mockReturnValue({ left: 100, top: 100, right: 400, bottom: 280, width: 300, height: 180 } as DOMRect);
    await pointer(wrapper.get('.home-archive-stack').element, 250, 180);
    expect(rear.classes()).toContain('is-highlighted');
    wrapper.get('.home-archive-stack').element.dispatchEvent(new MouseEvent('click', { clientX: 250, clientY: 180, bubbles: true, detail: 1 }));
    expect(wrapper.emitted('select')).toEqual([['game-4']]);
    wrapper.get('.home-archive-stack').element.dispatchEvent(new MouseEvent('click', { clientX: 450, clientY: 350, bubbles: true, detail: 1 }));
    expect(wrapper.emitted('select')).toHaveLength(1);
  } finally { wrapper.unmount(); }
});
