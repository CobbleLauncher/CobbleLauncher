// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameDirectoryMigrationPage from './GameDirectoryMigrationPage.vue';
import TransferWorldScene from '../ui/TransferWorldScene.vue';
import { setLocale } from '../i18n';
import type { TaskSnapshot } from '../tasks';
const callbacks = vi.hoisted(() => ({ task: null as null | ((event: { payload: TaskSnapshot }) => void), stop: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (_name, callback) => { callbacks.task = callback; return callbacks.stop; }) }));
const preview = { planId: 'host-plan', source: 'C:\\Games', target: 'D:\\Games', gameCount: 3, requiredBytes: 1073741824, availableBytes: 8589934592, enoughSpace: true };
const snapshot = (state: TaskSnapshot['state']): TaskSnapshot => ({ id: 'host-task', game_id: null, game_name: '', title: '迁移游戏位置', kind: 'directory_migration', state, progress: { phase: 'copying', completed: 25, total: 100, reused: 0 }, error: null, can_retry: state === 'failed' || state === 'interrupted', created_at: 1, updated_at: 2, has_result: false });
let records: TaskSnapshot[];
beforeEach(() => {
  setLocale('en-US'); records = []; callbacks.task = null; callbacks.stop.mockClear();
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_tasks') return records;
    if (command === 'preview_game_directory_migration') return preview;
    if (command === 'start_game_directory_migration') { records = [snapshot('running')]; return 'host-task'; }
    if (command === 'inspect_game_directory_migration') return { source: preview.source, target: preview.target, sourceRetained: true, canRecycle: true };
    if (command === 'cancel_task') return snapshot('cancelled');
    if (command === 'retry_task') return snapshot('running');
    return undefined;
  });
});
afterEach(() => { document.body.innerHTML = ''; vi.useRealTimers(); vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
async function open(props = {}) { const wrapper = mount(GameDirectoryMigrationPage, { props, attachTo: document.body }); await flushPromises(); return wrapper; }
function button(wrapper: ReturnType<typeof mount>, label: string) { return wrapper.findAll('button').find(item => item.text() === label)!; }
it('separates choosing, review, and actual transfer while preserving a reviewed draft on Back', async () => {
  const wrapper = await open(); expect(wrapper.get('h2').text()).toBe('Choose a new location');
  expect(wrapper.findAll('.migration-capacity')).toHaveLength(0);
  await button(wrapper, 'Choose folder').trigger('click'); await flushPromises();
  expect(wrapper.get('h2').text()).toBe('Review the move'); expect(wrapper.text()).toContain(preview.source); expect(wrapper.text()).toContain(preview.target);
  callbacks.task?.({ payload: { ...snapshot('failed'), id: 'late-unrelated-task' } }); await flushPromises(); expect(wrapper.get('h2').text()).toBe('Review the move');
  expect(invoke).not.toHaveBeenCalledWith('start_game_directory_migration', expect.anything());
  await button(wrapper, 'Back').trigger('click'); expect(wrapper.text()).toContain(preview.target);
  await button(wrapper, 'Continue').trigger('click'); await button(wrapper, 'Start move').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('start_game_directory_migration', { planId: 'host-plan' }); expect(wrapper.get('progress').attributes('value')).toBe('25');
  expect(wrapper.getComponent(TransferWorldScene).props()).toMatchObject({ state: 'working', progress: .25 });
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'preview_game_directory_migration')).toHaveLength(1);
  callbacks.task?.({ payload: { ...snapshot('succeeded'), updated_at: 4 } });
  callbacks.task?.({ payload: { ...snapshot('queued'), updated_at: 3 } }); await flushPromises();
  expect(wrapper.get('h2').text()).toBe('Move complete');
  wrapper.unmount();
});
it('keeps unknown space honest, allows a real attempt, and directs insufficient space to another pick', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_tasks' ? [] : { ...preview, availableBytes: null, enoughSpace: null });
  const wrapper = await open(); await button(wrapper, 'Choose folder').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Unknown'); expect(button(wrapper, 'Start move').attributes('disabled')).toBeUndefined();
  await button(wrapper, 'Back').trigger('click'); vi.mocked(invoke).mockResolvedValueOnce({ ...preview, availableBytes: 0, enoughSpace: false });
  await button(wrapper, 'Change folder').trigger('click'); await flushPromises();
  expect(button(wrapper, 'Start move')).toBeUndefined(); expect(button(wrapper, 'Change location / recheck')).toBeDefined(); wrapper.unmount();
});
it('does not cancel the host task when leaving, and reopens the exact requested task from the task center', async () => {
  records = [snapshot('running'), { ...snapshot('failed'), id: 'other' }]; const wrapper = await open({ initialTaskId: 'host-task' });
  await button(wrapper, 'Continue in background').trigger('click'); expect(wrapper.emitted('back')).toHaveLength(1);
  wrapper.unmount(); expect(callbacks.stop).toHaveBeenCalledOnce(); expect(invoke).not.toHaveBeenCalledWith('cancel_task', expect.anything());
  const restored = await open({ initialTaskId: 'other' }); expect(restored.get('h2').text()).toBe('Failed');
  callbacks.task?.({ payload: snapshot('succeeded') }); await flushPromises(); expect(restored.get('h2').text()).toBe('Failed');
  await button(restored, 'Retry').trigger('click'); await flushPromises(); expect(invoke).toHaveBeenCalledWith('retry_task', { taskId: 'other' }); restored.unmount();
});
it('uses static waiting/failed states and does not invent progress without a total', async () => {
  records = [{ ...snapshot('queued'), progress: null }]; const wrapper = await open();
  expect(wrapper.getComponent(TransferWorldScene).props()).toMatchObject({ state: 'waiting', progress: null }); expect(wrapper.text()).not.toContain('0%');
  callbacks.task?.({ payload: { ...snapshot('failed'), progress: null, error: 'DIRECTORY_MIGRATION_CHANGED' } }); await flushPromises();
  expect(wrapper.getComponent(TransferWorldScene).props()).toMatchObject({ state: 'failed', progress: null }); expect(wrapper.text()).toContain('The originals are kept'); wrapper.unmount();
});
it('cancels only through the explicit cancel action', async () => {
  records = [snapshot('running')]; const wrapper = await open(); await button(wrapper, 'Cancel move').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_task', { taskId: 'host-task' }); expect(wrapper.getComponent(TransferWorldScene).props('state')).toBe('cancelled'); wrapper.unmount();
});
it('never cleans on completion and requires the separate delayed confirmation', async () => {
  records = [snapshot('running')]; const wrapper = await open(); callbacks.task?.({ payload: snapshot('succeeded') }); await flushPromises();
  expect(wrapper.emitted('completed')).toHaveLength(1); expect(wrapper.get('h2').text()).toBe('Move complete');
  expect(invoke).not.toHaveBeenCalledWith('cleanup_game_directory_migration_source', expect.anything());
  vi.useFakeTimers(); await button(wrapper, 'Clean up original location').trigger('click'); await flushPromises();
  const confirm = Array.from(document.querySelectorAll<HTMLButtonElement>('[role=alertdialog] button')).find(item => item.textContent?.includes('Move to Recycle Bin'))!;
  expect(confirm.disabled).toBe(true); expect(document.querySelector('[role=alertdialog]')?.textContent).toContain('Empty the Recycle Bin');
  await vi.advanceTimersByTimeAsync(3100); confirm.click(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cleanup_game_directory_migration_source', { taskId: 'host-task' }); wrapper.unmount();
});
it('keeps useful feedback and the original-location action if cleanup is refused', async () => {
  records = [snapshot('succeeded')]; const wrapper = await open({ initialTaskId: 'host-task' });
  vi.mocked(invoke).mockRejectedValueOnce('DIRECTORY_MIGRATION_CHANGED'); await button(wrapper, 'Check original location').trigger('click'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('The originals are kept'); expect(button(wrapper, 'Check original location')).toBeDefined(); wrapper.unmount();
});
