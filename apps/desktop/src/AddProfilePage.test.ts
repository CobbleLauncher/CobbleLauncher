// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import AddProfilePage from './AddProfilePage.vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
beforeEach(() => { vi.useFakeTimers(); vi.mocked(writeText).mockReset(); vi.mocked(invoke).mockImplementation(async (command) => command === 'begin_microsoft_login' ? { flow_id: 'flow', user_code: 'ABCD1234', expires_in: 600, interval: 5 } : null); });
afterEach(() => { vi.useRealTimers(); });
it('shows an actionable field error and only creates a corrected offline name', async () => {
  const wrapper = mount(AddProfilePage, { props: { library: { profiles: [], active_id: null }, busy: false, error: '', ready: true } });
  await wrapper.get('#profile-name').setValue('Alex 玩家');
  await wrapper.get('#profile-name').trigger('blur');
  expect(wrapper.get('#profile-name-feedback').text()).toContain('不含空格');
  await wrapper.get('form').trigger('submit'); expect(wrapper.emitted('add')).toBeUndefined();
  await wrapper.get('#profile-name').setValue('Alex_Builder');
  expect(wrapper.get('#profile-name').attributes('aria-invalid')).toBe('false');
  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('add')).toEqual([['Alex_Builder', null]]); wrapper.unmount();
});
it('copies the new code before opening the browser and allows another copy', async () => {
  const order: string[] = [];
  vi.mocked(writeText).mockImplementation(async () => { order.push('copy'); });
  vi.mocked(invoke).mockImplementation(async (command) => { if (command === 'begin_microsoft_login') return { flow_id: 'flow', user_code: 'ABCD1234', expires_in: 600, interval: 5 }; if (command === 'open_microsoft_login') order.push('open'); return null; });
  const wrapper = mount(AddProfilePage, { props: { library: { profiles: [], active_id: null }, busy: false, error: '', ready: true } });
  await wrapper.findAll('button').find(button => button.text() === 'Microsoft 登录')!.trigger('click'); await flushPromises();
  expect(order).toEqual(['copy', 'open']); expect(writeText).toHaveBeenCalledWith('ABCD1234');
  await wrapper.get('[aria-label="复制登录代码"]').trigger('click'); await flushPromises(); expect(writeText).toHaveBeenCalledTimes(2);
  expect(wrapper.text()).toContain('已复制'); wrapper.unmount();
});
it('keeps login working when clipboard access fails', async () => {
  vi.mocked(writeText).mockRejectedValue(new Error('unavailable'));
  const wrapper = mount(AddProfilePage, { props: { library: { profiles: [], active_id: null }, busy: false, error: '', ready: true } });
  await wrapper.findAll('button').find(button => button.text() === 'Microsoft 登录')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_microsoft_login', { flowId: 'flow' }); expect(wrapper.text()).toContain('复制失败，请手动复制'); wrapper.unmount();
});
