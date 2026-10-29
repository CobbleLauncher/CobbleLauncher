// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { defineComponent, h, ref } from 'vue';
import { afterEach, expect, it, vi } from 'vitest';
import { useArchiveMotion } from './use-archive-motion';

afterEach(() => { vi.unstubAllGlobals(); });
function fixture() {
  const callbacks = new Map<number, FrameRequestCallback>();
  let id = 0, time = 0;
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { callbacks.set(++id, callback); return id; });
  vi.stubGlobal('cancelAnimationFrame', (key: number) => callbacks.delete(key));
  const focus = ref(2);
  const wrapper = mount(defineComponent({ setup() {
    const stack = ref<HTMLElement>();
    const motion = useArchiveMotion(stack, focus);
    return () => h('div', { ref: stack, onPointermove: motion.move, onPointerleave: motion.leave },
      Array.from({ length: 5 }, () => h('button', { class: 'home-archive-card' })));
  } }));
  vi.spyOn(wrapper.element, 'getBoundingClientRect').mockReturnValue({ left: 0, top: 0, width: 400, height: 300 } as DOMRect);
  const advance = () => { const batch = [...callbacks.values()]; callbacks.clear(); time += 16; batch.forEach(callback => callback(time)); };
  const settle = () => { for (let i = 0; callbacks.size && i < 100; i++) advance(); expect(callbacks.size).toBe(0); };
  const value = (key: string) => parseFloat(wrapper.element.style.getPropertyValue(key));
  const move = async (clientX: number, clientY: number, pointerType = 'mouse') => {
    const event = new MouseEvent('pointermove', { clientX, clientY, bubbles: true });
    Object.defineProperty(event, 'pointerType', { value: pointerType });
    wrapper.element.dispatchEvent(event); await wrapper.vm.$nextTick();
  };
  return { wrapper, focus, callbacks, advance, settle, value, move };
}

it('eases the whole stack toward the pointer, opens neighboring gaps and stops when settled', async () => {
  const f = fixture();
  try {
    await f.move(360, 30, 'mouse');
    expect(f.callbacks.size).toBe(1);
    f.advance();
    expect(f.value('--drift-x')).toBeGreaterThan(0);
    expect(f.value('--drift-x')).toBeLessThan(5.6);
    f.settle();
    expect(f.value('--drift-x')).toBeCloseTo(5.6);
    expect(f.value('--drift-y')).toBeCloseTo(-4);
    const gaps = f.wrapper.findAll('button').map(card => parseFloat(card.element.style.getPropertyValue('--fan-offset')));
    expect(gaps[1]).toBeLessThan(0);
    expect(gaps[3]).toBeGreaterThan(0);
    expect(Number(f.wrapper.findAll('button')[0]!.element.style.getPropertyValue('--front-open'))).toBeGreaterThan(.8);
    expect(Number(f.wrapper.findAll('button')[2]!.element.style.getPropertyValue('--front-open'))).toBe(0);
    expect(gaps.every((gap, index) => index === 0 || gap > gaps[index - 1]!)).toBe(true);
    await f.move(200, 150, 'mouse');
    f.settle();
    expect(f.value('--drift-x')).toBe(0);
    expect(parseFloat(f.wrapper.findAll('button')[2]!.element.style.getPropertyValue('--fan-offset'))).toBe(0);
    await f.wrapper.trigger('pointerleave'); f.settle();
    expect(f.wrapper.findAll('button').every(card => Number(card.element.style.getPropertyValue('--fan-offset')) === 0)).toBe(true);
    expect(f.wrapper.findAll('button').every(card => Number(card.element.style.getPropertyValue('--front-open')) === 0)).toBe(true);
  } finally { f.wrapper.unmount(); }
});

it('moves the spacing field continuously between previews without a hover reset or idle loop', async () => {
  const f = fixture();
  try {
    await f.move(200, 150, 'mouse');
    f.settle();
    const card = f.wrapper.findAll('button')[2]!.element;
    f.focus.value = 4; await f.wrapper.vm.$nextTick();
    expect(Number(card.style.getPropertyValue('--fan-offset'))).toBe(0);
    f.advance(); const intermediate = Number(card.style.getPropertyValue('--fan-offset'));
    f.settle();
    expect(intermediate).toBeLessThan(0);
    expect(intermediate).toBeGreaterThan(Number(card.style.getPropertyValue('--fan-offset')));
    await f.move(250, 200, 'mouse');
    f.wrapper.unmount(); expect(f.callbacks.size).toBe(0);
  } finally { if (f.wrapper.exists()) f.wrapper.unmount(); }
});

it('does not animate touch or reduced-motion interaction', async () => {
  const f = fixture();
  try {
    await f.move(360, 30, 'touch');
    expect(f.callbacks.size).toBe(0);
    f.wrapper.element.setAttribute('data-motion', 'reduced');
    await f.move(360, 30, 'mouse');
    expect(f.callbacks.size).toBe(0);
    expect(f.value('--drift-x')).toBe(0);
  } finally { f.wrapper.unmount(); }
});
