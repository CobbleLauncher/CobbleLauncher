// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import SearchField from './SearchField.vue';

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
