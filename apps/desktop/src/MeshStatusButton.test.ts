// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import MeshStatusButton from './MeshStatusButton.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

beforeEach(() => {
  vi.useFakeTimers(); vi.mocked(invoke).mockReset();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
});
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); setLocale('zh-CN'); document.body.innerHTML = ''; });

async function selectMenuItem(label: string) {
  await vi.waitFor(() => {
    if (![...document.querySelectorAll('[role="menuitem"]')].some(item => item.textContent?.includes(label))) {
      throw new Error(`Menu item not found: ${label}; body=${document.body.innerText}`);
    }
  });
  const item = [...document.querySelectorAll('[role="menuitem"]')].find(candidate => candidate.textContent?.includes(label));
  if (!item) throw new Error(`Menu item not found: ${label}`);
  item.dispatchEvent(new MouseEvent('click', { bubbles: true }));
}

it('shows the native connected snapshot, emits open, and stops polling when hidden or unmounted', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue({ connected: true });
  const removed = vi.spyOn(document, 'removeEventListener');
  const wrapper = mount(MeshStatusButton, { props: { active: true } });
  await vi.advanceTimersByTimeAsync(0);
  await flushPromises();

  expect(invoke).toHaveBeenCalledExactlyOnceWith('mesh_network_status');
  expect(wrapper.get('button').classes()).toContain('is-connected');
  expect(wrapper.get('button').attributes('aria-label')).toBe('Mesh connected');
  expect(wrapper.get('button').attributes('aria-current')).toBe('page');
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Multiplayer & contacts');
  expect(wrapper.emitted('open')).toHaveLength(1);

  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' });
  document.dispatchEvent(new Event('visibilitychange'));
  await vi.advanceTimersByTimeAsync(30_000);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'mesh_network_status')).toHaveLength(2);
  wrapper.unmount();
  expect(removed).toHaveBeenCalledWith('visibilitychange', expect.any(Function));
});

it('changes Mesh status natively, honors the leave guard, and discovers only on explicit re-enable', async () => {
  setLocale('en-US');
  let networkEnabled = false;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'mesh_network_status') return { enabled: networkEnabled, connected: false };
    if (command === 'mesh_set_enabled') {
      networkEnabled = typeof args === 'object' && args !== null && !Array.isArray(args) && 'enabled' in args && args.enabled === true;
      return { enabled: networkEnabled, connected: false };
    }
    return null;
  });
  const beforeChange = vi.fn().mockResolvedValue(true);
  const wrapper = mount(MeshStatusButton, { props: { beforeChange } });
  await vi.advanceTimersByTimeAsync(0);
  await flushPromises();

  await wrapper.get('button').trigger('click');
  await selectMenuItem('Go online');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_set_enabled', { enabled: true });
  expect(invoke).toHaveBeenCalledWith('mesh_discover');
  expect(wrapper.emitted('changed')).toHaveLength(1);

  beforeChange.mockResolvedValueOnce(false);
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Go offline');
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'mesh_set_enabled')).toHaveLength(1);

  // Retry after the rejected item selection closes the dropdown.
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Go offline');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_set_enabled', { enabled: false });
  expect(wrapper.emitted('changed')).toHaveLength(2);
  wrapper.unmount();
});

it('keeps null and failed snapshots gray and ignores replies after unmount', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue(null);
  const wrapper = mount(MeshStatusButton);
  await vi.advanceTimersByTimeAsync(0);
  await flushPromises();
  expect(wrapper.get('button').classes()).not.toContain('is-connected');
  expect(wrapper.get('button').attributes('aria-label')).toBe('Mesh not connected');

  vi.mocked(invoke).mockRejectedValueOnce(new Error('offline'));
  await vi.advanceTimersByTimeAsync(15_000);
  await flushPromises();
  expect(wrapper.get('button').classes()).not.toContain('is-connected');

  let resolveStatus!: (value: { connected: boolean }) => void;
  vi.mocked(invoke).mockReturnValue(new Promise(resolve => { resolveStatus = resolve; }));
  await vi.advanceTimersByTimeAsync(15_000);
  expect(invoke).toHaveBeenCalledTimes(3);
  const trigger = wrapper.get('button').element;
  wrapper.unmount();
  resolveStatus({ connected: true });
  await flushPromises();
  expect(trigger.classList.contains('is-connected')).toBe(false);
});

it('distinguishes a reachable relay from an active connection and offers identity setup separately', async () => {
  setLocale('zh-CN');
  let network = { enabled: true, connected: false, relay_available: true, discovery_checked: true };
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_identity_state' ? { profile: null, mode: 'persistent', active: false } : network);
  const wrapper = mount(MeshStatusButton);
  await vi.advanceTimersByTimeAsync(0); await flushPromises();
  expect(wrapper.get('button').text()).toBe('可联机');
  expect(wrapper.get('button').attributes('title')).toBe('中继可用，尚未建立联机会话');
  await wrapper.get('button').trigger('click'); await flushPromises();
  await selectMenuItem('创建 Mesh 身份');
  expect(wrapper.emitted('open')).toHaveLength(1);
  network = { ...network, relay_available: false };
  await vi.advanceTimersByTimeAsync(15_000); await flushPromises();
  expect(wrapper.get('button').text()).toBe('未连接');
  network = { ...network, discovery_checked: false };
  await vi.advanceTimersByTimeAsync(15_000); await flushPromises();
  expect(wrapper.get('button').text()).toBe('待检查');
  network = { ...network, enabled: false };
  await vi.advanceTimersByTimeAsync(15_000); await flushPromises();
  expect(wrapper.get('button').text()).toBe('离线');
  wrapper.unmount();
});

it('shows connecting during the explicit relay check and readiness after it finishes', async () => {
  setLocale('en-US');
  let checked = false;
  let complete!: () => void;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_discover') { await new Promise<void>(resolve => { complete = resolve; }); checked = true; return {}; }
    if (command === 'mesh_identity_state') return { profile: { display_name: 'Player' } };
    return { enabled: true, connected: false, relay_available: checked, discovery_checked: checked };
  });
  const wrapper = mount(MeshStatusButton);
  await vi.advanceTimersByTimeAsync(0); await flushPromises();
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Check connection'); await flushPromises();
  expect(wrapper.get('button').text()).toBe('Connecting');
  complete(); await flushPromises();
  expect(wrapper.get('button').text()).toBe('Ready');
  expect(wrapper.emitted('changed')).toHaveLength(1);
  wrapper.unmount();
});

it('refreshes after an older status request finishes behind a connection check', async () => {
  setLocale('en-US');
  let first = true;
  let oldReply!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_network_status' && first) { first = false; return new Promise(resolve => { oldReply = resolve; }); }
    return { enabled: true, connected: false, relay_available: true, discovery_checked: true };
  });
  const wrapper = mount(MeshStatusButton);
  await vi.advanceTimersByTimeAsync(0); await flushPromises();
  await wrapper.get('button').trigger('click');
  await selectMenuItem('Check connection'); await flushPromises();
  oldReply({ enabled: false, connected: false }); await flushPromises();
  expect(wrapper.get('button').text()).toBe('Ready');
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'mesh_network_status')).toHaveLength(2);
  wrapper.unmount();
});
