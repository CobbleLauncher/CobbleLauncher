// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { setLocale } from '../i18n';
import SettingsSaveBar from './SettingsSaveBar.vue';
beforeEach(() => { setLocale('en-US'); vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); setLocale('zh-CN'); });
it('acknowledges only a confirmed save, then quietly disappears', async () => {
  const wrapper = mount(SettingsSaveBar, { props: { dirty: true, savedRevision: 0 } });
  await wrapper.setProps({ dirty: false }); expect(wrapper.find('.settings-save-bar').exists()).toBe(false);
  await wrapper.setProps({ savedRevision: 1 }); expect(wrapper.text()).toBe('Saved'); expect(wrapper.find('button').exists()).toBe(false);
  await vi.advanceTimersByTimeAsync(2400); expect(wrapper.find('.settings-save-bar').exists()).toBe(false); wrapper.unmount();
});
it('never hides new drafts or offers duplicate save while pending', async () => {
  const wrapper = mount(SettingsSaveBar, { props: { dirty: false, savedRevision: 0 } });
  await wrapper.setProps({ savedRevision: 1 }); await wrapper.setProps({ dirty: true, busy: true });
  await vi.advanceTimersByTimeAsync(3000);
  expect(wrapper.text()).toContain('Applying'); expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined();
  await wrapper.setProps({ busy: false, error: 'Disk unavailable' });
  expect(wrapper.get('[role="alert"]').text()).toBe('Disk unavailable'); expect(wrapper.find('.primary-button').exists()).toBe(true); wrapper.unmount();
});
