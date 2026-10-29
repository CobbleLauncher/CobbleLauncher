// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import SearchField from './SearchField.vue';
it('clears on Escape, then returns focus on a second Escape', async () => {
  const opener = document.createElement('button'); document.body.append(opener); opener.focus();
  const wrapper = mount(SearchField, { attachTo: document.body, props: { label: 'Search', modelValue: 'word' } });
  const input = wrapper.get('input'); input.element.focus();
  await input.trigger('keydown', { key: 'Escape' });
  expect(wrapper.emitted('update:modelValue')).toEqual([['']]);
  expect(document.activeElement).toBe(input.element);
  await wrapper.setProps({ modelValue: '' }); await input.trigger('keydown', { key: 'Escape' });
  expect(document.activeElement).toBe(opener); wrapper.unmount(); opener.remove();
});
it('leaves Escape to an enclosing dialog and Enter to the input method', async () => {
  const dialog = document.createElement('div'); dialog.setAttribute('role', 'dialog'); document.body.append(dialog);
  const wrapper = mount(SearchField, { attachTo: dialog, props: { label: 'Search', modelValue: 'word' } });
  const input = wrapper.get('input');
  await input.trigger('keydown', { key: 'Escape' }); expect(wrapper.emitted('update:modelValue')).toBeUndefined();
  await input.trigger('compositionstart'); await input.setValue('输入');
  await input.trigger('keydown', { key: 'Enter', isComposing: true });
  expect(wrapper.emitted('submit')).toBeUndefined(); expect(wrapper.emitted('update:modelValue')).toBeUndefined();
  await input.trigger('compositionend'); expect(wrapper.emitted('update:modelValue')).toEqual([['输入']]);
  wrapper.unmount(); dialog.remove();
});

it('forwards search attributes to the input and emits typing and submit events', async () => {
  const wrapper = mount(SearchField, { props: { label: 'Search Mods', modelValue: '' }, attrs: { class: 'mods-search', 'data-list-search': 'mods', 'aria-describedby': 'mod-count' } });
  expect(wrapper.classes()).toContain('mods-search');
  const input = wrapper.get('input');
  expect(input.attributes()).toMatchObject({ type: 'search', 'aria-label': 'Search Mods', 'aria-describedby': 'mod-count', 'data-list-search': 'mods' });
  await input.setValue('Sodium');
  expect(wrapper.emitted('update:modelValue')).toEqual([['Sodium']]);
  await input.trigger('keydown', { key: 'Enter' });
  expect(wrapper.emitted('submit')).toHaveLength(1);
  wrapper.unmount();
});

it('offers one explicitly labeled clear action and retains input focus', async () => {
  const wrapper = mount(SearchField, { attachTo: document.body, props: { label: 'Search tools', modelValue: 'world', clearLabel: 'Clear search' } });
  const input = wrapper.get('input');
  expect(input.attributes('data-custom-clear')).toBe('true');
  await wrapper.get('button[aria-label="Clear search"]').trigger('click');
  expect(wrapper.emitted('update:modelValue')).toEqual([['']]);
  expect(document.activeElement).toBe(input.element);
  await wrapper.setProps({ modelValue: '' });
  expect(wrapper.find('button').exists()).toBe(false);
  wrapper.unmount();
});
