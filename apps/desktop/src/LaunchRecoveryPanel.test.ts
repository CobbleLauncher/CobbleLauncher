// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { i18n, setLocale } from './i18n';
import type { Game } from './games';
import type { TaskSnapshot } from './tasks';
import LaunchRecoveryPanel from './LaunchRecoveryPanel.vue';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const game = { id: 'owned', name: 'Test', launch_error: 'failed', launch_state: 'idle', status: 'installed' } as Game;
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
function panel(value = game) { return mount(LaunchRecoveryPanel, { props: { game: value }, global: { plugins: [i18n] } }); }
it('does no lookup for a healthy game and offers no stale log for prepare failure', async () => {
  const healthy = panel({ ...game, launch_error: null }); await flushPromises(); expect(invoke).not.toHaveBeenCalled(); healthy.unmount();
  vi.mocked(invoke).mockResolvedValue({ kind: 'java', current_log: false, can_repair: false });
  setLocale('en-US'); const wrapper = panel(); await flushPromises();
  expect(wrapper.text()).toContain('Choose compatible Java'); expect(wrapper.text()).not.toContain('Open launch log');
  await wrapper.findAll('button')[0]!.trigger('click'); expect(wrapper.emitted('runtime')).toHaveLength(1); wrapper.unmount();
});
it('analyzes only current failure and sends optional plugin install through extensions', async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ kind: 'unknown', current_log: true, can_repair: false }).mockResolvedValueOnce([{ manifest: { id: 'crash-doctor' }, enabled: true }]).mockRejectedValueOnce('请先启用支持此文件类型的分析插件');
  setLocale('en-US'); const wrapper = panel(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Analyze this failure')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('analyze_game_report', { gameId: 'owned', locale: 'en', currentLaunch: true });
  await wrapper.findAll('button').find(button => button.text().includes('Install or enable'))!.trigger('click');
  expect(wrapper.emitted('extensions')).toHaveLength(1); wrapper.unmount();
});
it('offers extensions for missing or disabled Crash Doctor without executing a report', async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ kind: 'unknown', current_log: true, can_repair: false }).mockResolvedValueOnce([{ manifest: { id: 'crash-doctor' }, enabled: false }]);
  setLocale('en-US'); const wrapper = panel(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Analyze this failure')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Install or enable Crash Doctor');
  expect(invoke).not.toHaveBeenCalledWith('analyze_game_report', expect.anything()); wrapper.unmount();
});
it('can cancel a pending current-launch report', async () => {
  let finish!: (value: null) => void;
  vi.mocked(invoke).mockResolvedValueOnce({ kind: 'unknown', current_log: true, can_repair: false }).mockResolvedValueOnce([{ manifest: { id: 'crash-doctor' }, enabled: true }]).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; })).mockResolvedValueOnce(undefined);
  setLocale('en-US'); const wrapper = panel(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Analyze this failure')!.trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Cancel analysis')!.trigger('click');
  expect(invoke).toHaveBeenLastCalledWith('cancel_plugin_report'); finish(null); await flushPromises(); wrapper.unmount();
});
it('repairs only when host authorizes it and blocks duplicate pending actions', async () => {
  let finish!: (value: TaskSnapshot) => void;
  vi.mocked(invoke).mockResolvedValueOnce({ kind: 'managed_artifacts', current_log: false, can_repair: true }).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  setLocale('en-US'); const wrapper = panel(); await flushPromises();
  const repair = wrapper.findAll('button').find(button => button.text() === 'Repair game files')!;
  await repair.trigger('click'); await repair.trigger('click'); expect(invoke).toHaveBeenCalledTimes(2);
  expect(wrapper.findAll('button').every(button => button.attributes('disabled') !== undefined)).toBe(true);
  const task = { id: 'repair-owned', game_id: 'owned', kind: 'game_repair', state: 'running', progress: { phase: 'files', completed: 2, total: 4, reused: 0 }, error: null } as TaskSnapshot;
  finish(task); await flushPromises();
  expect(wrapper.get('progress').attributes('value')).toBe('2');
  vi.mocked(invoke).mockResolvedValueOnce({ ...task, state: 'cancelling' });
  await wrapper.findAll('button').find(button => button.text() === 'Cancel')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('cancel_task', { taskId: 'repair-owned' });
  await wrapper.setProps({ tasks: [{ ...task, state: 'cancelled' }] });
  expect(wrapper.find('progress').exists()).toBe(false);
  expect(wrapper.findAll('button').find(button => button.text() === 'Repair game files')!.attributes('disabled')).toBeUndefined(); wrapper.unmount();
});
