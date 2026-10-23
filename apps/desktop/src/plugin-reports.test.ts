// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import PluginReportPanel from './PluginReportPanel.vue';
import { isReportPlugin, type PluginReport } from './plugin-reports';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const sample: PluginReport = {
  schema_version: 1,
  findings: [{ code: 'test', severity: 'warning', title: '<img src=x onerror=alert(1)>', message: 'Check configuration', evidence: [{ input_index: 0, first_line: 2, last_line: 4 }], suggestions: ['<script>alert(1)</script>'] }],
  tables: [{ columns: ['Key'], rows: [['<a href="file:///secret">value</a>']] }],
};

describe('plugin reports', () => {
  it('only accepts the report API and capability', () => {
    expect(isReportPlugin({ api_version: '0.1.0', capabilities: ['report'] })).toBe(true);
    expect(isReportPlugin({ api_version: '0.2.0', capabilities: ['report'] })).toBe(true);
    expect(isReportPlugin({ api_version: '1.0.0', capabilities: ['report'] })).toBe(false);
    expect(isReportPlugin({ api_version: '0.1.0', capabilities: ['report', 'viewer'] })).toBe(false);
  });
  it('passes only an opaque plugin id and displays untrusted report data as text', async () => {
    vi.mocked(invoke).mockResolvedValue(sample);
    const wrapper = mount(PluginReportPanel, { props: { pluginId: 'owned.report' } });
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('analyze_plugin_report', { pluginId: 'owned.report', locale: 'zh-CN' });
    expect(wrapper.get('h3').text()).toContain(sample.findings[0]!.title);
    expect(wrapper.findAll('img,script,a')).toHaveLength(0);
    expect(wrapper.get('.report-evidence').text()).toContain('文件 1，第 2–4 行');
    expect(wrapper.get('td').text()).toContain('file:///secret');
    setLocale('en-US'); await flushPromises();
    expect(wrapper.get('.secondary-button').text()).toBe('Analyze files…');
    expect(wrapper.get('.report-evidence').text()).toBe('File 1, lines 2–4');
    wrapper.unmount();
  });
  it('cancels pending analysis and treats picker cancellation as no result', async () => {
    let finish!: (value: null) => void;
    vi.mocked(invoke).mockImplementation(command => command === 'analyze_plugin_report' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(null));
    const wrapper = mount(PluginReportPanel, { props: { pluginId: 'owned.report' } });
    await wrapper.get('.secondary-button').trigger('click');
    expect(wrapper.get('.secondary-button').attributes('disabled')).toBeDefined();
    await wrapper.get('.text-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('cancel_plugin_report');
    finish(null); await flushPromises();
    expect(wrapper.find('.report-results').exists()).toBe(false);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.get('.secondary-button').attributes('disabled')).toBeUndefined();
    wrapper.unmount();
  });
  it('allows retry after failure and states only that an empty report has no findings', async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error('Analysis failed')).mockResolvedValueOnce({ schema_version: 1, findings: [], tables: [] });
    const wrapper = mount(PluginReportPanel, { props: { pluginId: 'owned.report' } });
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('Analysis failed');
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.get('.report-results').text()).toBe('未报告发现。');
    wrapper.unmount();
  });
});
