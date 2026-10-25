// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import MeshStatusButton from './MeshStatusButton.vue';
import MeshContactsButton from './MeshContactsButton.vue';
import { setLocale } from './i18n';
import type { MeshNetworkSnapshot } from './mesh';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
const snapshot = (phase: MeshNetworkSnapshot['phase'], generation: number, enabled = true): MeshNetworkSnapshot => ({
  enabled, phase, generation, attempt: 0, retry_at: null, profile: null, services: [], last_error: null,
  relay_available: phase === 'online', session_route: null, checks: [],
});
let event: ((value: { payload: MeshNetworkSnapshot }) => void) | undefined;
const unlisten = vi.fn();
beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
  unlisten.mockReset();
  vi.mocked(listen).mockImplementation(async (_name, handler) => { event = handler as typeof event; return unlisten; });
});
afterEach(() => { event = undefined; setLocale('zh-CN'); document.body.innerHTML = ''; });
async function selectMenuItem(label: string) {
  await vi.waitFor(() => expect([...document.querySelectorAll('[role="menuitem"]')].some(item => item.textContent?.includes(label))).toBe(true));
  document.querySelectorAll<HTMLElement>('[role="menuitem"]').forEach(item => { if (item.textContent?.includes(label)) item.click(); });
}

it('reads the cached snapshot once and follows newer native events without polling', async () => {
  vi.mocked(invoke).mockResolvedValue(snapshot('online', 2));
  const wrapper = mount(MeshStatusButton, { props: { active: true } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('mesh_network_snapshot');
  expect(wrapper.get('button').text()).toBe('Online');
  expect(wrapper.get('button').attributes('aria-current')).toBe('page');
  event?.({ payload: snapshot('reconnecting', 3) });
  await flushPromises();
  expect(wrapper.get('button').text()).toBe('Reconnecting');
  expect(wrapper.emitted('changed')).toHaveLength(1);
  event?.({ payload: snapshot('offline', 1, false) });
  await flushPromises();
  expect(wrapper.get('button').text()).toBe('Reconnecting');
  expect(wrapper.emitted('changed')).toHaveLength(1);
  wrapper.unmount();
  expect(unlisten).toHaveBeenCalledOnce();
});

it('enables the contacts entry from the cached online snapshot and forwards its destinations', async () => {
  vi.mocked(invoke).mockResolvedValue(snapshot('online', 2));
  const wrapper = mount(MeshStatusButton);
  await flushPromises();
  const contacts = wrapper.findComponent(MeshContactsButton);
  expect(contacts.props('available')).toBe(true);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('mesh_network_snapshot');
  contacts.vm.$emit('contact', 'exact-peer-id');
  contacts.vm.$emit('join');
  contacts.vm.$emit('open');
  expect(wrapper.emitted('contact')).toEqual([['exact-peer-id']]);
  expect(wrapper.emitted('join')).toEqual([[]]);
  expect(wrapper.emitted('contacts')).toEqual([[]]);
  expect(invoke).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});

it('keeps a same-generation online event when an older cached read finishes later', async () => {
  let finishRead!: (value: MeshNetworkSnapshot) => void;
  vi.mocked(invoke).mockReturnValue(new Promise(resolve => { finishRead = resolve; }));
  const wrapper = mount(MeshStatusButton);
  await flushPromises();
  event?.({ payload: snapshot('online', 3) });
  await flushPromises();
  finishRead(snapshot('connecting', 3));
  await flushPromises();
  expect(wrapper.get('button').text()).toBe('Online');
  wrapper.unmount();
});

it('uses retry only for a real failed state and honors the leave guard', async () => {
  const beforeChange = vi.fn().mockResolvedValue(false);
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_network_retry' ? snapshot('connecting', 3) : snapshot('failed', 2));
  const wrapper = mount(MeshStatusButton, { props: { beforeChange } });
  await flushPromises();
  expect(wrapper.get('button').text()).toBe('Connection failed');
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Retry connection');
  await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('mesh_network_retry');
  beforeChange.mockResolvedValue(true);
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Retry connection');
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'mesh_network_retry')).toHaveLength(1);
  expect(wrapper.get('button').text()).toBe('Connecting');
  wrapper.unmount();
});

it('can go online from persisted offline state without calling discovery', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_network_set_enabled' ? snapshot('connecting', 4) : snapshot('offline', 3, false));
  const wrapper = mount(MeshStatusButton);
  await flushPromises();
  expect(wrapper.get('button').text()).toBe('Offline');
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Go online');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_network_set_enabled', { enabled: true });
  expect(invoke).not.toHaveBeenCalledWith('mesh_discover');
  expect(wrapper.get('button').text()).toBe('Connecting');
  wrapper.unmount();
});
