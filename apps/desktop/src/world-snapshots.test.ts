// SPDX-License-Identifier: MIT
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import WorldSnapshotsPanel from './WorldSnapshotsPanel.vue';
import type { LocalWorld } from './games';
import type { WorldSnapshot } from './world-snapshots';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(()=>vi.useFakeTimers({toFake:['Date','setTimeout','clearTimeout','setInterval','clearInterval']}));
afterEach(() => { vi.useRealTimers(); vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const world: LocalWorld = { folder_name: 'World', name: 'My world', minecraft_version: '1.21', game_mode: 0, hardcore: false, last_played: null, metadata_error: false };
const snapshot: WorldSnapshot = { id: 'opaque-snapshot', world_id: 'World', name: '<img src=x>', created_at: '2026-10-22T08:00:00Z', processed_bytes: 4096, added_bytes: 1024 };
const repositoryStorage = { repository_stored_bytes: 2048, total_snapshot_logical_bytes: 4096, snapshot_count: 1 };
function button(wrapper: ReturnType<typeof mount>, text: string) { return wrapper.findAll('button').find(value => value.text() === text)!; }
const cleanupPreview = { previewId: 'approved-cleanup', policy: { keep_last: 10, keep_named: true }, remove: [snapshot], keep_count: 1, named_kept: 1, estimated_reclaim_bytes: 1024, repack_bytes: 4096 };
it('previews scoped retention and requires confirmation before any permanent cleanup', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [snapshot] : cleanupPreview));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } }); await flushPromises();
  await wrapper.get('.snapshot-cleanup-policy').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_world_snapshot_cleanup', { gameId: 'game', folderName: 'World', policy: { keep_last: 10, keep_named: true } });
  expect(invoke).not.toHaveBeenCalledWith('apply_world_snapshot_cleanup', expect.anything());
  expect(wrapper.get('.cleanup-preview').text()).toContain('Versions to remove: 1 · Keep: 1');
  expect(wrapper.get('.cleanup-preview').text()).toContain('1 KiB');
  expect(wrapper.find('img').exists()).toBe(false);
  await wrapper.get('input[type="number"]').setValue(2);
  expect(wrapper.find('.cleanup-preview').exists()).toBe(false);
  await wrapper.get('input[type="number"]').setValue(0);
  expect(button(wrapper, 'Preview cleanup').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});
it('applies only the approved token and reports actual reclaimed space and partial cleanup honestly', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [snapshot] : command === 'preview_world_snapshot_cleanup' ? cleanupPreview : { removed_count: 1, reclaimed_bytes: 512, cleanup_complete: false, remaining: [] }));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } }); await flushPromises();
  await wrapper.get('.snapshot-cleanup-policy').trigger('submit'); await flushPromises();
  await vi.advanceTimersByTimeAsync(3000); await button(wrapper, 'Confirm cleanup').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('apply_world_snapshot_cleanup', { previewId: 'approved-cleanup', confirmed: true });
  expect(wrapper.text()).toContain('Versions removed: 1 · Freed 512 B');
  expect(wrapper.get('[role="alert"]').text()).toContain('Space cleanup did not finish');
  expect(wrapper.find('.cleanup-preview').exists()).toBe(false);
  expect(wrapper.find('.snapshot-list').exists()).toBe(false);
  wrapper.unmount();
});
it('keeps cleanup disabled for locked games and refreshes history after a stale approval fails', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(command => command === 'apply_world_snapshot_cleanup' ? Promise.reject('Preview expired') : Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [snapshot] : cleanupPreview));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: true } }); await flushPromises();
  expect(button(wrapper, 'Preview cleanup').attributes('disabled')).toBeDefined();
  await wrapper.setProps({ locked: false });
  await wrapper.get('.snapshot-cleanup-policy').trigger('submit'); await flushPromises();
  await vi.advanceTimersByTimeAsync(3000); await button(wrapper, 'Confirm cleanup').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Preview expired');
  expect(wrapper.find('.cleanup-preview').exists()).toBe(false);
  expect(wrapper.find('.snapshot-list').exists()).toBe(true);
  wrapper.unmount();
});
it('saves a named or timestamp-default version with only scoped ids and refreshes history', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [snapshot] : snapshot));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_world_snapshots', { gameId: 'game', folderName: 'World' });
  expect(wrapper.text()).toContain('Stored by this version 1 KiB'); expect(wrapper.text()).toContain('Logical world size 4 KiB');
  expect(wrapper.find('img').exists()).toBe(false);
  await wrapper.get('input').setValue('Before update'); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_world_snapshot', { gameId: 'game', folderName: 'World', name: 'Before update' });
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_world_snapshot', { gameId: 'game', folderName: 'World', name: null });
  expect(wrapper.emitted('busy')).toEqual([[true], [false], [true], [false]]);
  wrapper.unmount();
});
it('requires restore confirmation, retains the matching world and reports the real restore outcome', async () => {
  setLocale('en-US'); const outcome = { worlds: [world], rollback_id: 'backup-id' };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [snapshot] : outcome));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } }); await flushPromises();
  await button(wrapper, 'Restore this version').trigger('click');
  expect(invoke).not.toHaveBeenCalledWith('restore_world_snapshot', expect.anything()); expect(wrapper.get('.snapshot-confirm').text()).toContain('saved under Backups');
  expect(wrapper.get('.snapshot-confirm').text()).toContain('Restore target: My world');
  expect(wrapper.get('.snapshot-confirm').text()).toContain('Logical world size 4 KiB');
  await button(wrapper, 'Cancel').trigger('click'); expect(wrapper.find('.snapshot-confirm').exists()).toBe(false);
  await button(wrapper, 'Restore this version').trigger('click'); await vi.advanceTimersByTimeAsync(3000); await button(wrapper, 'Confirm restore').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('restore_world_snapshot', { gameId: 'game', folderName: 'World', snapshotId: 'opaque-snapshot', replaceExisting: true });
  expect(wrapper.emitted('restored')).toEqual([[outcome, 'World']]);
  wrapper.unmount();
});

it('orders versions newest first and keeps restore target details attached to the selected version', async () => {
  setLocale('en-US');
  const older = { ...snapshot, id: 'older', name: 'Older', created_at: '2026-10-20T08:00:00Z', processed_bytes: 2048, added_bytes: 256 };
  const newer = { ...snapshot, id: 'newer', name: 'Newer', created_at: '2026-10-22T08:00:00Z', processed_bytes: 4096, added_bytes: 1024 };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : [older, newer]));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } });
  await flushPromises();

  expect([...wrapper.findAll('.snapshot-list h3')].map(heading => heading.text())).toEqual(['Newer', 'Older']);
  await button(wrapper, 'Restore this version').trigger('click');
  expect(wrapper.get('.snapshot-confirm .confirmation-title').text()).toContain('Newer');
  expect(wrapper.get('.snapshot-confirm').text()).toContain('Stored by this version 1 KiB');
  expect(wrapper.get('.snapshot-confirm').text()).toContain('Restore target: My world');
  wrapper.unmount();
});

it('compares two selected versions in chronological order and pages changed paths', async () => {
  setLocale('en-US');
  const older = { ...snapshot, id: 'older', name: 'Older', created_at: '2026-10-20T08:00:00Z' };
  const newer = { ...snapshot, id: 'newer', name: 'Newer', created_at: '2026-10-22T08:00:00Z' };
  const firstPage = { added: 1, modified: 2, deleted: 1, unchanged: 5, total: 2, offset: 0, rows: [{ path: 'a.txt', kind: 'modified', before_bytes: 10, after_bytes: 12 }] };
  const secondPage = { ...firstPage, offset: 1, rows: [{ path: 'b.txt', kind: 'added', before_bytes: null, after_bytes: 20 }] };
  vi.mocked(invoke).mockImplementation((command, args) => Promise.resolve(command === 'get_world_snapshot_storage' ? repositoryStorage : command === 'list_world_snapshots' ? [older, newer] : (args as Record<string, unknown>).offset === 0 ? firstPage : secondPage));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } });
  await flushPromises();
  const checks = wrapper.findAll('.snapshot-list input[type="checkbox"]');
  await checks[0]!.setValue(true);
  await checks[1]!.setValue(true);
  await button(wrapper, 'Compare versions').trigger('click');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('compare_world_snapshots', { gameId: 'game', folderName: 'World', beforeId: 'older', afterId: 'newer', offset: 0, limit: 50 });
  expect(wrapper.get('.snapshot-compare').text()).toContain('Added 1 · Modified 2 · Deleted 1 · Unchanged 5');
  expect(wrapper.get('.snapshot-change-list').text()).toContain('a.txt');
  await button(wrapper, 'Load more changes').trigger('click');
  await flushPromises();
  expect(wrapper.get('.snapshot-change-list').text()).toContain('b.txt');
  expect(wrapper.findAll('.snapshot-change-list li')).toHaveLength(2);
  wrapper.unmount();
});
it('keeps locked worlds read-only and permits retry after an actual list failure', async () => {
  setLocale('en-US'); let listReads = 0; vi.mocked(invoke).mockImplementation(command => command === 'get_world_snapshot_storage' ? Promise.resolve(repositoryStorage) : ++listReads === 1 ? Promise.reject('Read failed') : Promise.resolve([snapshot]));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: true } }); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Read failed');
  await button(wrapper, 'Retry loading').trigger('click'); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(button(wrapper, 'Save version').attributes('disabled')).toBeDefined(); expect(button(wrapper, 'Restore this version').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});
it('shows restore failure without a success state and permits a fresh review', async () => {
  setLocale('en-US'); vi.mocked(invoke).mockImplementation(command => command === 'get_world_snapshot_storage' ? Promise.resolve(repositoryStorage) : command === 'list_world_snapshots' ? Promise.resolve([snapshot]) : Promise.reject('Restore failed'));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false } }); await flushPromises();
  await button(wrapper, 'Restore this version').trigger('click'); await vi.advanceTimersByTimeAsync(3000); await button(wrapper, 'Confirm restore').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Restore failed'); expect(wrapper.find('.snapshot-confirm').exists()).toBe(false); expect(wrapper.emitted('restored')).toBeUndefined();
  wrapper.unmount();
});

it('uses the shared task identity for phase progress and cancellation, holding locks until the awaited worker stops', async () => {
  setLocale('en-US');
  let stop!: () => void;
  vi.mocked(invoke).mockImplementation(command => command === 'get_world_snapshot_storage' ? Promise.resolve(repositoryStorage) : command === 'list_world_snapshots' ? Promise.resolve([]) : command === 'create_world_snapshot' ? new Promise((_resolve, reject) => { stop = () => reject('Backup operation cancelled'); }) : Promise.resolve(null));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false, tasks: [] } }); await flushPromises();
  await wrapper.get('form').trigger('submit');
  const task = { id: 'snapshot-task-id', game_id: 'game', world_folder: world.folder_name, game_name: 'Game', title: 'Save version', kind: 'world_snapshot' as const, state: 'running' as const, progress: { phase: 'world-snapshot-write', completed: 1024, total: 4096, reused: 0 }, error: null, can_retry: false, created_at: 1, updated_at: 1, has_result: false };
  await wrapper.setProps({ tasks: [{ ...task, id: 'wrong-world-task', world_folder: 'Other' }] });
  expect(wrapper.find('.snapshot-task').exists()).toBe(false);
  await wrapper.setProps({ tasks: [task] });
  expect(wrapper.get('.snapshot-task').text()).toContain('Saving version content');
  expect(wrapper.get('.snapshot-task').text()).toContain('1 KiB / 4 KiB');
  expect(wrapper.get('progress').attributes('value')).toBe('1024');
  await wrapper.setProps({ tasks: [{ ...task, progress: { phase: 'world-commit', completed: 0, total: 1, reused: 0 } }] });
  expect(wrapper.get('.snapshot-task').text()).toContain('0 / 1');
  expect(wrapper.get('.snapshot-task').text()).not.toContain(' B');
  await button(wrapper, 'Cancel task').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_task', { taskId: task.id });
  expect(button(wrapper, 'Cancelling…').attributes('disabled')).toBeDefined();
  await wrapper.setProps({ tasks: [{ ...task, state: 'cancelled', updated_at: 2 }] });
  expect(wrapper.get('form button').attributes('disabled')).toBeDefined();
  stop(); await flushPromises();
  expect(wrapper.get('form button').attributes('disabled')).toBeUndefined();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(wrapper.get('[role="status"]').text()).toBe('Operation cancelled');
  expect(wrapper.text()).not.toContain('Version saved');
  wrapper.unmount();
});

it('reports success when a late cancellation request arrives after commit', async () => {
  setLocale('en-US');
  let finish!: () => void;
  vi.mocked(invoke).mockImplementation(command => command === 'get_world_snapshot_storage' ? Promise.resolve(repositoryStorage) : command === 'list_world_snapshots' ? Promise.resolve([]) : command === 'create_world_snapshot' ? new Promise(resolve => { finish = () => resolve(snapshot); }) : Promise.resolve(null));
  const wrapper = mount(WorldSnapshotsPanel, { global:{stubs:{teleport:{template:'<slot />'}}}, props: { gameId: 'game', world, locked: false, tasks: [] } }); await flushPromises();
  await wrapper.get('form').trigger('submit');
  const task = { id: 'late-cancel-task', game_id: 'game', world_folder: world.folder_name, game_name: 'Game', title: 'Save', kind: 'world_snapshot' as const, state: 'running' as const, progress: null, error: null, can_retry: false, created_at: 1, updated_at: 1, has_result: false };
  await wrapper.setProps({ tasks: [task] });
  await button(wrapper, 'Cancel task').trigger('click'); await flushPromises();
  await wrapper.setProps({ tasks: [{ ...task, state: 'succeeded', updated_at: 2 }] });
  finish(); await flushPromises();
  expect(wrapper.get('[role="status"]').text()).toBe('Version saved');
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  wrapper.unmount();
});
