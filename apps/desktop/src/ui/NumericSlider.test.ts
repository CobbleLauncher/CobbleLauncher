// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import NumericSlider from './NumericSlider.vue';

const wrappers: ReturnType<typeof mount>[] = [];
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} }); });
afterEach(() => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); vi.unstubAllGlobals(); });

it('clamps numeric entry to bounds and snaps to its step', async () => {
  const wrapper = mount(NumericSlider, { props: { label: 'Interface scale', modelValue: 100, min: 90, max: 125, step: 5, defaultValue: 100, unit: '%' } });
  wrappers.push(wrapper);
  const input = wrapper.get('input');
  expect(input.attributes()).toMatchObject({ 'aria-label': 'Interface scale', min: '90', max: '125', step: '5' });
  expect(wrapper.get('[data-default-value]').attributes('data-default-value')).toBe('100');
  await input.setValue('999');
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([125]);
  await input.setValue('92');
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([90]);
  await input.setValue('103');
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([105]);
});

it('retains the current value on empty input and supports keyboard slider changes', async () => {
  const wrapper = mount(NumericSlider, { attachTo: document.body, props: { label: 'Text size', modelValue: 14, min: 13, max: 18 } });
  wrappers.push(wrapper);
  const input = wrapper.get('input');
  await input.setValue('');
  expect((input.element as HTMLInputElement).value).toBe('14');
  expect(wrapper.emitted('update:modelValue')).toBeUndefined();
  const slider = wrapper.get('[role="slider"]');
  expect(slider.attributes()).toMatchObject({ 'aria-label': 'Text size', 'aria-valuenow': '14' });
  await slider.trigger('keydown', { key: 'ArrowRight' });
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([15]);
  await wrapper.setProps({ disabled: true });
  expect(input.attributes('disabled')).toBeDefined();
  const count = wrapper.emitted('update:modelValue')?.length;
  await slider.trigger('keydown', { key: 'ArrowRight' });
  expect(wrapper.emitted('update:modelValue')?.length).toBe(count);
});

it('uses semantic value text for discrete settings without exposing meaningless numbers', () => {
  const wrapper = mount(NumericSlider, { props: { label: 'Density', modelValue: 1, min: 0, max: 2, valueText: 'Standard' } });
  wrappers.push(wrapper);
  expect(wrapper.find('input').exists()).toBe(false);
  expect(wrapper.get('[role="slider"]').attributes('aria-valuetext')).toBe('Standard');
  expect(wrapper.text()).toBe('Standard');
});
