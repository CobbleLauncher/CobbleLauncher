// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import PluginWebView from './PluginWebView.vue';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.mocked(invoke).mockReset(); });
function setup() {
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ left: 24, top: 160, right: 700, bottom: 600, width: 676, height: 440 } as DOMRect);
}
it('opens logical bounds and hides its exact owned view before a modal', async () => {
  setup();
  vi.mocked(invoke).mockResolvedValue({ view_id: 'owned' });
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page', instanceId: 'game-a' } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_plugin_view', expect.objectContaining({ pluginId: 'custom-page', instanceId: 'game-a', keybindingTargetId: null, bounds: { x: 24, y: 160, width: 676, height: 440 } }));
  await wrapper.vm.suspend();
  expect(invoke).toHaveBeenCalledWith('set_plugin_view_visible', { viewId: 'owned', visible: false });
  await wrapper.vm.resume();
  expect(invoke).toHaveBeenCalledWith('set_plugin_view_visible', { viewId: 'owned', visible: true });
  wrapper.unmount();
});
it('passes the requested keybinding target into the native view context', async () => {
  setup();
  vi.mocked(invoke).mockResolvedValue({ view_id: 'owned' });
  const wrapper = mount(PluginWebView, { props: { pluginId: 'config-doctor', keybindingTargetId: 'game-target' } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_plugin_view', expect.objectContaining({ instanceId: null, keybindingTargetId: 'game-target' }));
  wrapper.unmount();
});
it('closes a late open after navigation without adopting it', async () => {
  setup();
  let finish!: (value: { view_id: string }) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'open_plugin_view' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve());
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page' } });
  await flushPromises();
  await wrapper.setProps({ active: false });
  finish({ view_id: 'late' });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('close_plugin_view', { viewId: 'late' });
  wrapper.unmount();
});
it('does not open an inactive page', async () => {
  setup();
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page', active: false } });
  await flushPromises();
  expect(invoke).not.toHaveBeenCalled();
  wrapper.unmount();
});
it('waits for an opening child to hide before suspension completes', async () => {
  setup();
  let finish!: (value: { view_id: string }) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'open_plugin_view' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve());
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page' } });
  await flushPromises();
  let suspended = false;
  const closing = wrapper.vm.suspend().then(() => { suspended = true; });
  await flushPromises();
  expect(suspended).toBe(false);
  finish({ view_id: 'pending-child' });
  await closing;
  expect(invoke).toHaveBeenCalledWith('set_plugin_view_visible', { viewId: 'pending-child', visible: false });
  wrapper.unmount();
});
