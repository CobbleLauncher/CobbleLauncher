// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import TransferWorldScene from './TransferWorldScene.vue';

let inView: (entries: { isIntersecting: boolean }[]) => void;
let hidden = false, reduced = false;
const motionListeners = new Set<() => void>();
const disconnect = vi.fn();
const wrappers: ReturnType<typeof mount>[] = [];
beforeEach(() => {
  hidden = false; reduced = false; motionListeners.clear(); disconnect.mockClear();
  vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden);
  vi.stubGlobal('matchMedia', () => ({ get matches() { return reduced; }, addEventListener: (_: string, listener: () => void) => motionListeners.add(listener), removeEventListener: (_: string, listener: () => void) => motionListeners.delete(listener) }));
  vi.stubGlobal('IntersectionObserver', class { constructor(callback: typeof inView) { inView = callback; } observe() {} disconnect() { disconnect(); } });
});
afterEach(() => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function scene(progress: number | null, state: 'working' | 'waiting' | 'failed' | 'complete' | 'cancelled' = 'working') {
  const wrapper = mount(TransferWorldScene, { props: { progress, state, sourceLabel: 'Old drive', targetLabel: 'New drive' } }); wrappers.push(wrapper); return wrapper;
}
const count = (wrapper: ReturnType<typeof mount>, world: string) => Number(wrapper.get(`[data-world="${world}"]`).attributes('data-built-blocks'));

it('moves the actual construction amount from left to right without inventing a percentage', async () => {
  const wrapper = scene(0), total = count(wrapper, 'source');
  expect(total).toBeGreaterThan(30); expect(count(wrapper, 'target')).toBe(0);
  await wrapper.setProps({ progress: .5 });
  expect(count(wrapper, 'target')).toBe(Math.floor(total / 2));
  expect(count(wrapper, 'source') + count(wrapper, 'target')).toBe(total);
  await wrapper.setProps({ progress: 1 });
  expect(count(wrapper, 'source')).toBe(0); expect(count(wrapper, 'target')).toBe(total);
  expect(wrapper.attributes('data-animating')).toBe('false');
  expect(wrapper.text()).toBe('Old driveNew drive'); expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
});

it('keeps unknown work unquantified, clamps bad bounds, and honors authoritative completion', async () => {
  const wrapper = scene(null), total = count(wrapper, 'source');
  expect(wrapper.attributes('data-progress-known')).toBe('false'); expect(count(wrapper, 'target')).toBe(0);
  await wrapper.setProps({ progress: Number.NaN }); expect(wrapper.attributes('data-progress-known')).toBe('false');
  await wrapper.setProps({ progress: -1 }); expect(count(wrapper, 'source')).toBe(total);
  await wrapper.setProps({ progress: 2 }); expect(count(wrapper, 'target')).toBe(total);
  await wrapper.setProps({ progress: null, state: 'complete' }); expect(count(wrapper, 'target')).toBe(total);
});

it('freezes waiting, failed and cancelled scenes at their real progress', async () => {
  const wrapper = scene(.4), transferred = count(wrapper, 'target');
  for (const state of ['waiting', 'failed', 'cancelled'] as const) {
    await wrapper.setProps({ state });
    expect(wrapper.attributes('data-animating')).toBe('false');
    expect(wrapper.find('.transfer-particles').exists()).toBe(false);
    expect(count(wrapper, 'target')).toBe(transferred);
  }
});

it('pauses CSS flight offscreen, while hidden and under reduced motion, then releases observers', async () => {
  const wrapper = scene(.25); await nextTick(); expect(wrapper.attributes('data-animating')).toBe('true');
  inView([{ isIntersecting: false }]); await nextTick(); expect(wrapper.attributes('data-animating')).toBe('false');
  inView([{ isIntersecting: true }]); hidden = true; document.dispatchEvent(new Event('visibilitychange')); await nextTick(); expect(wrapper.attributes('data-animating')).toBe('false');
  hidden = false; document.dispatchEvent(new Event('visibilitychange')); reduced = true; motionListeners.forEach(listener => listener()); await nextTick(); expect(wrapper.attributes('data-animating')).toBe('false');
  reduced = false; motionListeners.forEach(listener => listener()); await nextTick(); expect(wrapper.attributes('data-animating')).toBe('true');
  wrapper.unmount(); wrappers.pop(); expect(disconnect).toHaveBeenCalledOnce(); expect(motionListeners.size).toBe(0);
});
