// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useTaskExit } from './use-task-exit';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('./i18n', () => ({ locale: { value: 'en' } }));
function setup(leave = async () => true) {
  let api!: ReturnType<typeof useTaskExit>;
  const wrapper = mount(defineComponent({ setup() { api = useTaskExit(true, leave); return () => null; } }));
  return { api, wrapper };
}
afterEach(() => { vi.clearAllMocks(); });
describe('task exit', () => {
  it('respects unsaved edits before checking tasks', async () => {
    const { api, wrapper } = setup(async () => false);
    expect(await api.confirmExit()).toBe(false);
    expect(invoke).not.toHaveBeenCalled(); wrapper.unmount();
  });
  it('keeps tasks running when waiting is selected', async () => {
    invoke.mockResolvedValue(2);
    const { api, wrapper } = setup(); const result = api.confirmExit();
    await vi.waitFor(() => expect(api.open.value).toBe(true));
    api.keepWaiting(); expect(await result).toBe(false);
    expect(invoke).toHaveBeenCalledTimes(1); wrapper.unmount();
  });
  it('only allows exit after native workers have cleaned up', async () => {
    let count = 1;
    invoke.mockImplementation(async (command: string) => command === 'get_active_task_count' ? count : undefined);
    const { api, wrapper } = setup(); const result = api.confirmExit();
    await vi.waitFor(() => expect(api.open.value).toBe(true));
    const cancel = api.cancelAndExit();
    await vi.waitFor(() => expect(api.busy.value).toBe(true));
    expect(api.open.value).toBe(true); count = 0;
    await cancel; expect(await result).toBe(true); wrapper.unmount();
  });
  it('shows cancellation failures and keeps the window open', async () => {
    invoke.mockImplementation(async (command: string) => { if (command === 'cancel_active_tasks') throw new Error('fixture'); return 1; });
    const { api, wrapper } = setup(); const result = api.confirmExit();
    await vi.waitFor(() => expect(api.open.value).toBe(true));
    await api.cancelAndExit(); expect(api.error.value).not.toBe(''); expect(api.open.value).toBe(true);
    api.keepWaiting(); expect(await result).toBe(false); wrapper.unmount();
  });
});
