// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import AppIcon from './AppIcon.vue';
import AppIconButton from './AppIconButton.vue';

it('keeps decorative icons out of the accessibility tree while retaining SVG sizing', () => {
  const wrapper = mount(AppIcon, { props: { name: 'folder', size: 20 } });
  expect(wrapper.get('svg').attributes()).toMatchObject({ 'aria-hidden': 'true', focusable: 'false', width: '20', height: '20' });
  expect(wrapper.find('path').exists()).toBe(true);
  wrapper.unmount();
});

it('gives an icon-only action an accessible name without submitting its surrounding form', async () => {
  const clicked = vi.fn();
  const wrapper = mount(AppIconButton, { props: { icon: 'copy', label: 'Copy path' }, attrs: { onClick: clicked } });
  expect(wrapper.get('button').attributes()).toMatchObject({ type: 'button', 'aria-label': 'Copy path', title: 'Copy path' });
  await wrapper.get('button').trigger('click'); expect(clicked).toHaveBeenCalledOnce();
  expect(wrapper.get('svg').attributes('aria-hidden')).toBe('true');
  wrapper.unmount();
});

it('forwards native disabled and disclosure state to the action', () => {
  const wrapper = mount(AppIconButton, { props: { icon: 'chevron-down', label: 'More' }, attrs: { disabled: true, 'aria-expanded': false } });
  expect(wrapper.get('button').element.disabled).toBe(true);
  expect(wrapper.get('button').attributes('aria-expanded')).toBe('false');
  wrapper.unmount();
});
