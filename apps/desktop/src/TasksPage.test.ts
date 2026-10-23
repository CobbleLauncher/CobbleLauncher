// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import TasksPage from './TasksPage.vue';
import type { TaskSnapshot } from './tasks';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('locates only completed game tasks through their game identity', async () => {
  setLocale('en-US');
  const completed: TaskSnapshot = { id: 'completed', game_id: 'owned-game', game_name: 'Example', title: 'Install Example', kind: 'game_install', state: 'succeeded', progress: null, error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: false };
  const wrapper = mount(TasksPage, { props: { tasks: [completed, { ...completed, id: 'running', state: 'running' }, { ...completed, id: 'archive', game_id: null, kind: 'modpack_download' }], loading: false, error: '' } });
  const locate = wrapper.findAll('button').filter(button => button.text() === 'View game');
  expect(locate).toHaveLength(1);
  await locate[0]!.trigger('click');
  expect(wrapper.emitted('locate')).toEqual([['owned-game']]);
  expect(invoke).not.toHaveBeenCalled();
  wrapper.unmount();
});

it.each([
  ['curseforge_pack_download', 'get_task_curseforge_pack_preview', 'selectedCfPack'],
  ['modpack_download', 'get_task_pack_preview', 'selectedPack'],
] as const)('continues %s through its own provider preview', async (kind, command, event) => {
  const preview = { preview_id: 'owned-preview', name: 'Pack' };
  vi.mocked(invoke).mockResolvedValue(preview);
  const task: TaskSnapshot = { id: 'owned-task', game_id: null, game_name: 'Pack', title: 'Pack', kind, state: 'succeeded', progress: null, error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: true };
  const wrapper = mount(TasksPage, { props: { tasks: [task], loading: false, error: '' } });
  await wrapper.get('.task-actions button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { taskId: task.id });
  expect(wrapper.emitted(event)?.[0]).toEqual([preview]);
  expect(wrapper.emitted(event === 'selectedPack' ? 'selectedCfPack' : 'selectedPack')).toBeUndefined();
  wrapper.unmount();
});

it('localizes task phases and actions in English', () => {
  setLocale('en-US');
  const task: TaskSnapshot = { id: 'install-task', game_id: null, game_name: 'Example', title: 'Install Example', kind: 'modpack_download', state: 'running', progress: { phase: 'pack-download', completed: 1, total: 2, reused: 0 }, error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: false };
  const wrapper = mount(TasksPage, { props: { tasks: [task], loading: false, error: '' } });
  expect(wrapper.find('.task-progress-caption').text()).toContain('Downloading pack files');
  expect(wrapper.find('.task-state').text()).toBe('Running');
  expect(wrapper.get('.task-actions button').text()).toBe('Cancel task');
  wrapper.unmount();
});

it('shows activity only for running work and keeps transfer detail collapsed', () => {
  const running: TaskSnapshot = {
    id: 'active-download', game_id: null, game_name: 'Example', title: 'Install Example', kind: 'game_install', state: 'running',
    progress: { phase: 'files', completed: 2, total: 5, reused: 0, detail: { bytesCompleted: 200, bytesTotal: 500, bytesReceived: 100, bytesPerSecond: 50, activeDownloads: 1, items: [{ source: 'official', item: 'very-long-artifact-filename-that-should-truncate.jar', bytesReceived: 100, bytesTotal: 500 }] } },
    error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: false,
  };
  const failed: TaskSnapshot = { ...running, id: 'failed-download', state: 'failed', progress: { phase: 'files', completed: 2, total: 5, reused: 0 } };
  const wrapper = mount(TasksPage, { props: { tasks: [failed, running], loading: false, error: '' } });

  expect(wrapper.find('.task-state-running .ui-busy-icon').exists()).toBe(true);
  expect(wrapper.find('.task-state-failed .ui-busy-icon').exists()).toBe(false);
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('2');
  const details = wrapper.get('.task-transfer-details');
  expect(details.attributes('open')).toBeUndefined();
  expect(details.get('.task-transfer-file').text()).toContain('very-long-artifact-filename');
  expect(details.get('.task-transfer-meta > span:last-child').text()).toContain('100 B / 500 B');
  wrapper.unmount();
});

it('filters and searches task history while keeping active work first and retry available', async () => {
  setLocale('en-US');
  const makeTask = (id: string, state: TaskSnapshot['state'], updatedAt: number, title: string, error: string | null = null): TaskSnapshot => ({
    id, game_id: null, game_name: 'Example', title, kind: 'game_install', state,
    progress: null, error, can_retry: state === 'failed', created_at: 1, updated_at: updatedAt, has_result: false,
  });
  const tasks = [
    makeTask('failed', 'failed', 8, 'Install Modpack', 'Network connection failed'),
    makeTask('done-1', 'succeeded', 7, 'Install Vanilla'),
    makeTask('running', 'running', 3, 'Install Fabric'),
    makeTask('canceling', 'cancelling', 2, 'Install Forge'),
    makeTask('done-2', 'succeeded', 1, 'Update configuration'),
  ];
  vi.mocked(invoke).mockResolvedValue({ ...tasks[0], state: 'running' });
  const wrapper = mount(TasksPage, { props: { tasks, loading: false, error: '' } });

  expect(wrapper.get('.task-filter-group').text()).toContain('All · 5');
  expect(wrapper.get('.task-filter-group').text()).toContain('Active · 2');
  expect(wrapper.get('.task-filter-group').text()).toContain('Failed · 1');
  expect(wrapper.findAll('.task-row').map(row => row.attributes('class'))[0]).toContain('task-active');
  expect(wrapper.get('.task-row h2').text()).toBe('Install Fabric');

  await wrapper.get('.task-filter[aria-pressed="false"]:nth-child(3)').trigger('click');
  expect(wrapper.findAll('.task-row')).toHaveLength(1);
  expect(wrapper.get('.task-row h2').text()).toBe('Install Modpack');
  expect(wrapper.get('.task-actions button').text()).toBe('Retry task');
  await wrapper.get('.task-actions button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('retry_task', { taskId: 'failed' });

  await wrapper.get('.task-filter-group button:first-child').trigger('click');
  await wrapper.get('.task-search input').setValue('configuration');
  expect(wrapper.findAll('.task-row')).toHaveLength(1);
  expect(wrapper.get('.task-row h2').text()).toBe('Update configuration');
  await wrapper.get('.task-search input').setValue('no matching task');
  expect(wrapper.get('.content-empty').text()).toBe('No tasks match these filters.');
  await wrapper.get('button[aria-label="Clear task search"]').trigger('click');
  expect(wrapper.findAll('.task-row')).toHaveLength(5);
  wrapper.unmount();
});

it('keeps history controls out of a short one-task list', () => {
  const task: TaskSnapshot = { id: 'single-task', game_id: null, game_name: 'Example', title: 'Install Example', kind: 'game_install', state: 'succeeded', progress: null, error: null, can_retry: false, created_at: 1, updated_at: 1, has_result: false };
  const wrapper = mount(TasksPage, { props: { tasks: [task], loading: false, error: '' } });
  expect(wrapper.find('.task-history-controls').exists()).toBe(false);
  wrapper.unmount();
});
