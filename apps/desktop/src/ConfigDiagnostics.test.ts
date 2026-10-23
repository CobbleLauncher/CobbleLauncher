// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import ConfigDiagnostics from './ConfigDiagnostics.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const document = { id: 'opaque-config', name: 'example.toml', text: 'a=1', revision: 'rev-1', backups: [] };
const plugin = { enabled: true, manifest: { id: 'config-doctor', name: 'Config Doctor', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['toml'] } };
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); });
afterEach(() => setLocale('zh-CN'));
it('checks the selected saved revision with a compatible enabled plugin and discards stale results', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [plugin]
    : new Promise(resolve => { finish = resolve; }));
  const wrapper = mount(ConfigDiagnostics, { props: { gameId: 'game', document } });
  await flushPromises();
  await wrapper.get('.secondary-button').trigger('click');
  expect(invoke).toHaveBeenCalledWith('analyze_game_config_report', {
    gameId: 'game', configId: 'opaque-config', expectedRevision: 'rev-1', pluginId: 'config-doctor', locale: 'en',
  });
  expect(wrapper.emitted('busy')).toEqual([[true]]);
  await wrapper.setProps({ document: { ...document, revision: 'rev-2' } });
  finish({ schema_version: 1, findings: [], tables: [] });
  await flushPromises();
  expect(wrapper.findComponent({ name: 'ReportResults' }).exists()).toBe(false);
  expect(wrapper.emitted('busy')).toEqual([[true], [false]]);
  wrapper.unmount();
});
it('hides unsupported or disabled plugins and prevents diagnosing unsaved edits', async () => {
  vi.mocked(invoke).mockResolvedValue([{ ...plugin, enabled: false }]);
  const hidden = mount(ConfigDiagnostics, { props: { gameId: 'game', document } });
  await flushPromises(); expect(hidden.find('section').exists()).toBe(false); hidden.unmount();
  vi.mocked(invoke).mockResolvedValue([plugin]);
  const wrapper = mount(ConfigDiagnostics, { props: { gameId: 'game', document, disabled: true } });
  await flushPromises();
  expect(wrapper.get('.secondary-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.secondary-button').trigger('click');
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'analyze_game_config_report')).toHaveLength(0);
  wrapper.unmount();
});
