// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import MirrorAddressEditor from './MirrorAddressEditor.vue';
it.each(['http://mirror.example.org/', 'https://user:password@mirror.example.org/', 'https://mirror.example.org/?key=value', 'not a URL'])('keeps invalid mirror %s in the editor and blocks use', async value => {
  const wrapper = mount(MirrorAddressEditor, { props: { modelValue: value, disabled: false } });
  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('use')).toBeUndefined();
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('true');
  expect(wrapper.get('input').element.value).toBe(value);
  expect(wrapper.get('form').attributes('novalidate')).toBeDefined(); wrapper.unmount();
});
it('allows correction and emits a trimmed mirror without persisting it itself', async () => {
  const wrapper = mount(MirrorAddressEditor, { props: { modelValue: 'bad', disabled: false } });
  await wrapper.get('form').trigger('submit');
  await wrapper.setProps({ modelValue: ' https://mirror.example.org/ ' });
  expect(wrapper.get('input').attributes('aria-invalid')).toBe('false');
  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('use')).toEqual([['https://mirror.example.org/']]); wrapper.unmount();
});
