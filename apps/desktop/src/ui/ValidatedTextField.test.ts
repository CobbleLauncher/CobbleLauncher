// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import ValidatedTextField from './ValidatedTextField.vue';
const props = { id: 'name', modelValue: '', label: 'Player name', hint: '3–16 characters', validator: (value: string) => value.length < 3 ? 'Use at least 3 characters' : null };
afterEach(() => { document.body.innerHTML = ''; vi.unstubAllGlobals(); });
it('does not scold an untouched or empty blurred field; links the hint to its input', async () => {
  const wrapper = mount(ValidatedTextField, { props });
  expect(wrapper.get('input').attributes('aria-describedby')).toBe('name-feedback');
  await wrapper.get('input').trigger('blur');
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('false');
  expect(wrapper.get('#name-feedback').text()).toBe(props.hint); wrapper.unmount();
});
it('validates on nonempty blur, then clears the error as soon as the input is corrected', async () => {
  const wrapper = mount(ValidatedTextField, { props: { ...props, modelValue: 'Al' } });
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('false');
  await wrapper.get('input').trigger('blur');
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('true');
  expect(wrapper.get('#name-feedback').text()).toBe('Use at least 3 characters');
  await wrapper.setProps({ modelValue: 'Alex' });
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('false');
  expect(wrapper.get('#name-feedback').text()).toBe(props.hint); wrapper.unmount();
});
it('focuses failed submissions and repeats a single local animation, without losing the text', async () => {
  const wrapper = mount(ValidatedTextField, { props, attachTo: document.body });
  const cancel = vi.fn(); const animate = vi.fn(() => ({ cancel }));
  wrapper.get('input').element.animate = animate as unknown as typeof Element.prototype.animate;
  expect(wrapper.vm.validate()).toBe(false);
  expect(document.activeElement).toBe(wrapper.get('input').element);
  expect(animate).toHaveBeenCalledTimes(1);
  wrapper.vm.validate(); expect(cancel).toHaveBeenCalledTimes(1);
  expect(wrapper.emitted('update:modelValue')).toBeUndefined(); wrapper.unmount();
});
it.each(['system', 'application'])('respects %s reduced motion while keeping focus and errors', async mode => {
  const wrapper = mount(ValidatedTextField, { props, attachTo: document.body });
  if (mode === 'application') wrapper.element.setAttribute('data-motion', 'reduced');
  else vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true })));
  const animate = vi.fn(); wrapper.get('input').element.animate = animate;
  expect(wrapper.vm.validate()).toBe(false);
  await wrapper.vm.$nextTick();
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('true');
  expect(document.activeElement).toBe(wrapper.get('input').element);
  expect(animate).not.toHaveBeenCalled(); wrapper.unmount();
});
it('does not validate or submit partial IME composition', async () => {
  const wrapper = mount(ValidatedTextField, { props });
  await wrapper.get('input').trigger('compositionstart');
  expect(wrapper.vm.validate()).toBe(false);
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('false');
  await wrapper.get('input').trigger('compositionend');
  expect(wrapper.vm.validate()).toBe(false);
  await wrapper.vm.$nextTick(); expect(wrapper.get('input').attributes('aria-invalid')).toBe('true'); wrapper.unmount();
});
it('shows length near the limit without silently truncating pasted text', async () => {
  const wrapper = mount(ValidatedTextField, { props: { ...props, limit: 16, modelValue: 'abcdefghijklmnooo' } });
  expect(wrapper.get('input').attributes('maxlength')).toBeUndefined();
  expect(wrapper.get('.validated-field-count').text()).toBe('17 / 16'); wrapper.unmount();
});
