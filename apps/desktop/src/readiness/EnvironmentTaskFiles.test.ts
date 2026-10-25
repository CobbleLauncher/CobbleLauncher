// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import EnvironmentTaskFiles from './EnvironmentTaskFiles.vue';
import type { EnvironmentRequirements } from '../readiness';
import type { CurseForgeMissing } from '../curseforge';
import { setLocale } from '../i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

const missing = (fileId: string, name: string): CurseForgeMissing => ({
  project_id: `project-${fileId}`, file_id: fileId, name, file_name: `${name}.jar`,
  size_bytes: 1024, website_url: 'https://www.curseforge.com/minecraft/mc-mods/example', reason: 'manual',
});
const requirements = (files: CurseForgeMissing[], ready = files.length === 0): EnvironmentRequirements => ({ ready, missing: files });

beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue(requirements([missing('file-a', 'First mod'), missing('file-b', 'Second mod')], false));
});
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('keeps missing files after cancelling the picker, selects by task/project/file identity, and enables retry only when ready', async () => {
  const first = missing('file-a', 'First mod');
  const second = missing('file-b', 'Second mod');
  let selections = 0;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_task_environment_requirements') return requirements([first, second], false);
    if (command === 'select_task_environment_manual_file') {
      selections++;
      return selections === 1 ? null : selections === 2 ? requirements([second], false) : requirements([], true);
    }
    return null;
  });
  const wrapper = mount(EnvironmentTaskFiles, { props: { taskId: 'task-7' } });
  await flushPromises();
  expect(wrapper.findAll('.task-manual-files li')).toHaveLength(2);
  expect(wrapper.find('.task-manual-files').findAll('button').some(button => button.text() === 'Retry')).toBe(false);

  await wrapper.get('.task-manual-files li:first-child button.secondary-button').trigger('click');
  await flushPromises();
  expect(wrapper.findAll('.task-manual-files li')).toHaveLength(2);
  expect(invoke).toHaveBeenCalledWith('select_task_environment_manual_file', { taskId: 'task-7', projectId: first.project_id, fileId: first.file_id });
  const selectionArgs = vi.mocked(invoke).mock.calls.find(([command]) => command === 'select_task_environment_manual_file')?.[1];
  expect(selectionArgs).not.toHaveProperty('path');

  await wrapper.get('.task-manual-files li:first-child button.secondary-button').trigger('click');
  await flushPromises();
  expect(wrapper.findAll('.task-manual-files li')).toHaveLength(1);
  expect(wrapper.text()).toContain('Second mod');

  await wrapper.get('.task-manual-files li:first-child button.secondary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('select_task_environment_manual_file', { taskId: 'task-7', projectId: second.project_id, fileId: second.file_id });
  expect(wrapper.findAll('.task-manual-files li')).toHaveLength(0);
  expect(wrapper.text()).toContain('Required files are ready. You can retry the task.');
  await wrapper.findAll('button').find(button => button.text() === 'Retry')!.trigger('click');
  expect(wrapper.emitted('retry')).toHaveLength(1);
  wrapper.unmount();
});

it('shows a safe retry message after a picker error without exposing its raw details', async () => {
  const privateError = 'C:/Users/Player/AppData/Secret\nAuthorization: Bearer private-token';
  vi.mocked(invoke).mockImplementationOnce(() => Promise.reject(new Error(privateError)))
    .mockResolvedValueOnce(requirements([missing('retry-file', 'Retry mod')], false));
  const wrapper = mount(EnvironmentTaskFiles, { props: { taskId: 'task-private' } });
  await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('The file could not be read or verified. Please try again.');
  expect(wrapper.text()).not.toContain('private-token');
  expect(wrapper.text()).not.toContain('C:/Users/Player');
  await wrapper.get('[role=alert] button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_task_environment_requirements', { taskId: 'task-private' });
  expect(wrapper.findAll('.task-manual-files li')).toHaveLength(1);
  expect(wrapper.find('[role=alert]').exists()).toBe(false);
  wrapper.unmount();
});

it('does not let a late response for the previous task replace the current task requirements', async () => {
  const old = deferred<EnvironmentRequirements>();
  const current = deferred<EnvironmentRequirements>();
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command !== 'get_task_environment_requirements') return Promise.resolve(null);
    return (args as { taskId: string }).taskId === 'task-old' ? old.promise : current.promise;
  });
  const wrapper = mount(EnvironmentTaskFiles, { props: { taskId: 'task-old' } });
  await wrapper.setProps({ taskId: 'task-current' });
  current.resolve(requirements([missing('current-file', 'Current task file')], false));
  await flushPromises();
  expect(wrapper.text()).toContain('Current task file');
  old.resolve(requirements([missing('old-file', 'Stale task file')], false));
  await flushPromises();
  expect(wrapper.text()).toContain('Current task file');
  expect(wrapper.text()).not.toContain('Stale task file');
  expect(invoke).toHaveBeenCalledWith('get_task_environment_requirements', { taskId: 'task-old' });
  expect(invoke).toHaveBeenCalledWith('get_task_environment_requirements', { taskId: 'task-current' });
  wrapper.unmount();
});
