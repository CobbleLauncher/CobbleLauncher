// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import AutomaticWorldBackup from './AutomaticWorldBackup.vue';
import { setLocale } from './i18n';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
beforeEach(() => { setLocale('en-US'); invoke.mockReset(); });
it('offers an optional first-use choice and persists decline without enabling backups', async () => {
  invoke.mockResolvedValueOnce({ preference: 'undecided', last_backup: null, last_error: null });
  const wrapper = mount(AutomaticWorldBackup, { props: { gameId: 'abc' } });
  await flushPromises();
  expect(wrapper.text()).toContain('Back up changed worlds');
  invoke.mockResolvedValueOnce({ preference: 'disabled', last_backup: null, last_error: null });
  await wrapper.get('.text-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('set_automatic_world_backup', { gameId: 'abc', preference: 'disabled' });
  expect(wrapper.find('.text-button').exists()).toBe(false);
  expect(wrapper.get('[role="switch"]').attributes('data-state')).toBe('unchecked');
  wrapper.unmount();
});
it('keeps the persisted choice and shows the latest backup and failure', async () => {
  invoke.mockResolvedValue({ preference: 'enabled', last_backup: '2026-10-23T12:00:00Z', last_error: 'Disk full' });
  const wrapper = mount(AutomaticWorldBackup, { props: { gameId: 'abc' } });
  await flushPromises();
  expect(wrapper.text()).toContain('Last backup');
  expect(wrapper.get('[role="status"]').text()).toContain('Disk full');
  expect(wrapper.find('.text-button').exists()).toBe(false);
  wrapper.unmount();
});
