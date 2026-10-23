// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameDiagnosticsPanel from './GameDiagnosticsPanel.vue';
import { setLocale, t } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
it('analyzes only the game id and links known suggestions without applying changes', async () => {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, findings: ['java_class_version', 'fabric_required_dependency_missing', '<script>'].map(code => ({ code, severity: 'error', title: '<img>', message: '<a href="file:///secret">', evidence: [], suggestions: ['<script>'] })), tables: [] });
  const wrapper = mount(GameDiagnosticsPanel, { props: { gameId: 'owned-game' } });
  await wrapper.get('button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('analyze_game_report', { gameId: 'owned-game', locale: 'zh-CN' });
  expect(wrapper.findAll('img,a,script')).toHaveLength(0);
  await wrapper.findAll('button').find(button => button.text() === t('reports.runtime'))!.trigger('click'); await wrapper.findAll('button').find(button => button.text() === t('reports.mods'))!.trigger('click');
  expect(wrapper.emitted('runtime')).toHaveLength(1); expect(wrapper.emitted('mods')).toHaveLength(1);
  expect(invoke).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});
it('offers extensions and a translated retry after a missing plugin error', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockRejectedValueOnce('请先启用支持此文件类型的分析插件').mockResolvedValueOnce(null);
  const wrapper = mount(GameDiagnosticsPanel, { props: { gameId: 'owned-game' } });
  await wrapper.get('button').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Enable Crash Doctor');
  await wrapper.findAll('button')[1]!.trigger('click'); expect(wrapper.emitted('extensions')).toHaveLength(1);
  expect(wrapper.get('button').text()).toBe('Retry analysis');
  await wrapper.get('button').trigger('click'); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(wrapper.find('.report-results').exists()).toBe(false);
  wrapper.unmount();
});
it('cancels pending analysis and keeps unknown findings as plain text without actions', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'analyze_game_report' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(null));
  const wrapper = mount(GameDiagnosticsPanel, { props: { gameId: 'owned-game' } });
  await wrapper.get('button').trigger('click');
  expect(wrapper.get('button').attributes('disabled')).toBeDefined();
  await wrapper.findAll('button')[1]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_plugin_report');
  finish({ schema_version: 1, findings: [{ code: 'unknown', severity: 'info', title: 'Unknown finding', message: 'Details', evidence: [], suggestions: ['Check manually'] }], tables: [] }); await flushPromises();
  expect(wrapper.findAll('button').filter(button => [t('reports.runtime'), t('reports.mods')].includes(button.text()))).toHaveLength(0);
  expect(wrapper.text()).toContain('Check manually');
  wrapper.unmount();
});
it('is absent while idle with an external trigger and remains controllable from the native menu', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'analyze_game_report' ? new Promise(resolve => {finish = resolve;}) : Promise.resolve(null));
  const wrapper = mount(GameDiagnosticsPanel, {props: {gameId: 'owned-game', externalTrigger: true}});
  expect(wrapper.find('.game-diagnostics').exists()).toBe(false);
  void wrapper.vm.analyze(); await flushPromises();
  expect(wrapper.find('.game-diagnostics').exists()).toBe(true); expect(wrapper.vm.pending).toBe(true);
  finish({schema_version: 1, findings: [], tables: []}); await flushPromises();
  expect(wrapper.find('.report-results').exists()).toBe(true); expect(wrapper.vm.pending).toBe(false); wrapper.unmount();
});
