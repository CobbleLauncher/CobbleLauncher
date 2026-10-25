// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import PluginWebView from './PluginWebView.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
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
it('suspends a dirty plugin draft until the user keeps editing or discards it', async () => {
  setup();
  vi.mocked(invoke).mockImplementation(async command => command === 'open_plugin_view' ? { view_id: 'owned' } : command === 'plugin_view_is_dirty' ? true : undefined);
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page' } });
  await flushPromises();

  const cancelled = wrapper.vm.confirmLeave();
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_plugin_view_visible', { viewId: 'owned', visible: false });
  expect(document.body.querySelector('[role="dialog"]')).not.toBeNull();
  await Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent?.includes('继续编辑'))?.click();
  await flushPromises();
  expect(await cancelled).toBe(false);
  expect(invoke).toHaveBeenCalledWith('set_plugin_view_visible', { viewId: 'owned', visible: true });

  const discarded = wrapper.vm.confirmLeave();
  await flushPromises();
  Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent?.includes('放弃更改'))?.click();
  await flushPromises();
  expect(await discarded).toBe(true);
  expect(invoke).toHaveBeenCalledWith('close_plugin_view', { viewId: 'owned' });
  wrapper.unmount();
});
it('preserves the page and reports a failed dirty check', async () => {
  setup();
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'open_plugin_view') return { view_id: 'owned' };
    if (command === 'plugin_view_is_dirty') throw new Error('query failed');
    return undefined;
  });
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page' } });
  await flushPromises();
  const leave = wrapper.vm.confirmLeave();
  await flushPromises();
  expect(document.body.textContent).toContain('无法检查插件中尚未保存的更改');
  expect(invoke).not.toHaveBeenCalledWith('close_plugin_view', { viewId: 'owned' });
  Array.from(document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find(button => button.textContent?.includes('放弃更改'))?.click();
  await flushPromises();
  expect(await leave).toBe(true);
  expect(invoke).toHaveBeenCalledWith('close_plugin_view', { viewId: 'owned' });
  wrapper.unmount();
});
it('defers theme and locale reopen while dirty, then refreshes after the draft is clean', async () => {
  setup();
  let dirty = true;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'open_plugin_view') return { view_id: `view-${vi.mocked(invoke).mock.calls.filter(([name]) => name === 'open_plugin_view').length}` };
    if (command === 'plugin_view_is_dirty') return dirty;
    return undefined;
  });
  const wrapper = mount(PluginWebView, { props: { pluginId: 'custom-page', handoffId: 'handoff-once' } });
  await flushPromises();
  document.documentElement.dataset.theme = 'dark';
  await flushPromises();
  setLocale('en-US');
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'open_plugin_view')).toHaveLength(1);
  expect(vi.mocked(invoke).mock.calls.find(([command]) => command === 'open_plugin_view')?.[1]).toEqual(expect.objectContaining({ handoffId: 'handoff-once' }));
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'close_plugin_view')).toHaveLength(0);

  dirty = false;
  await wrapper.vm.suspend();
  await wrapper.vm.resume();
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'open_plugin_view')).toHaveLength(2);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'open_plugin_view')[1]?.[1]).toEqual(expect.objectContaining({ handoffId: null }));
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'close_plugin_view')).toHaveLength(1);
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
