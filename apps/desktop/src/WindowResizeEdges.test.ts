// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import WindowResizeEdges from './WindowResizeEdges.vue';

const native = vi.hoisted(() => ({
  enabled: true,
  isMaximized: vi.fn(), isFullscreen: vi.fn(), isResizable: vi.fn(),
  startResizeDragging: vi.fn(), onResized: vi.fn(), unlisten: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => native.enabled }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => native }));

beforeEach(() => {
  native.enabled = true;
  native.isMaximized.mockResolvedValue(false);
  native.isFullscreen.mockResolvedValue(false);
  native.isResizable.mockResolvedValue(true);
  native.onResized.mockResolvedValue(native.unlisten);
});

function edge(direction: string) { return document.querySelector(`[data-resize-direction="${direction}"]`)!; }

describe('native window resize edges', () => {
  it('starts all eight actual Tauri resize directions for left mouse gestures', async () => {
    const wrapper = mount(WindowResizeEdges);
    await flushPromises();
    const directions = ['North', 'NorthEast', 'East', 'SouthEast', 'South', 'SouthWest', 'West', 'NorthWest'];
    expect(document.querySelectorAll('[data-resize-direction]')).toHaveLength(8);
    for (const direction of directions) {
      edge(direction).dispatchEvent(new MouseEvent('mousedown', { button: 0, bubbles: true, cancelable: true }));
      await flushPromises();
      expect(native.startResizeDragging).toHaveBeenLastCalledWith(direction);
    }
    wrapper.unmount();
    expect(native.unlisten).toHaveBeenCalledOnce();
  });

  it('does not claim browser resizing and does not intercept right clicks', async () => {
    native.enabled = false;
    const browser = mount(WindowResizeEdges);
    await flushPromises();
    expect(document.querySelector('[data-resize-direction]')).toBeNull();
    browser.unmount();
    native.enabled = true;
    const wrapper = mount(WindowResizeEdges);
    await flushPromises();
    edge('North').dispatchEvent(new MouseEvent('mousedown', { button: 2, bubbles: true }));
    await flushPromises();
    expect(native.startResizeDragging).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it.each(['isMaximized', 'isFullscreen', 'isResizable'] as const)('removes edges when %s prevents resizing', async (state) => {
    const wrapper = mount(WindowResizeEdges);
    await flushPromises();
    native[state].mockResolvedValue(state !== 'isResizable');
    window.dispatchEvent(new Event('focus'));
    await flushPromises();
    expect(document.querySelector('[data-resize-direction]')).toBeNull();
    wrapper.unmount();
  });

  it('rechecks stale window state before resizing and supports explicit disabling', async () => {
    const wrapper = mount(WindowResizeEdges);
    await flushPromises();
    native.isMaximized.mockResolvedValue(true);
    edge('North').dispatchEvent(new MouseEvent('mousedown', { button: 0, bubbles: true }));
    await flushPromises();
    expect(native.startResizeDragging).not.toHaveBeenCalled();
    native.isMaximized.mockResolvedValue(false);
    window.dispatchEvent(new Event('focus'));
    await flushPromises();
    await wrapper.setProps({ disabled: true });
    expect(document.querySelector('[data-resize-direction]')).toBeNull();
    wrapper.unmount();
  });

  it('updates native resize state and surfaces a rejected resize action', async () => {
    const wrapper = mount(WindowResizeEdges);
    await flushPromises();
    const failure = new Error('Native resize unavailable');
    native.startResizeDragging.mockRejectedValueOnce(failure);
    edge('North').dispatchEvent(new MouseEvent('mousedown', { button: 0, bubbles: true }));
    await flushPromises();
    expect(wrapper.emitted('error')).toEqual([[failure]]);
    native.isFullscreen.mockResolvedValue(true);
    native.onResized.mock.calls[0]![0]();
    await flushPromises();
    expect(document.querySelector('[data-resize-direction]')).toBeNull();
    wrapper.unmount();
  });
});
