// SPDX-License-Identifier: MIT
import { describe, it, expect, vi, afterEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import PluginAccessLogPanel from './PluginAccessLogPanel.vue';
import { setLocale } from './i18n';
import { accessTarget, accessTime, type PluginAccessEntry } from './plugin-access-log';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
describe('plugin access metadata', () => {
  const entry: PluginAccessEntry = { timestamp_ms: 0, plugin_id: 'doctor', capability: 'game-config-read', action: 'configs.read', outcome: 'completed', target: null };
  it('shows public game IDs and origins without additional information', () => {
    expect(accessTarget({ ...entry, target: 'game:game-123' })).toBe('game-123');
    expect(accessTarget({ ...entry, target: 'https://example.com' })).toBe('https://example.com');
    expect(accessTarget(entry)).toBe('');
  });
  it('formats the recorded time', () => { expect(accessTime(0, 'en-US')).toBe(new Date(0).toLocaleString('en-US')); });
  it('queries the selected installed ID and renders audit data as text', async () => {
    setLocale('en-US');
    vi.mocked(invoke).mockResolvedValue([{ ...entry, target: '<script>secret</script>' }]);
    const wrapper = mount(PluginAccessLogPanel, { props: { pluginId: 'doctor' } });
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith('plugin_access_log', { pluginId: 'doctor' });
    expect(wrapper.text()).toContain('Read game configuration');
    expect(wrapper.text()).toContain('Completed');
    expect(wrapper.find('script').exists()).toBe(false);
    wrapper.unmount();
  });
  it('does not retain records from a previous plugin after a failed switch', async () => {
    setLocale('en-US');
    vi.mocked(invoke).mockResolvedValueOnce([entry]).mockRejectedValueOnce(new Error('private path'));
    const wrapper = mount(PluginAccessLogPanel, { props: { pluginId: 'doctor' } });
    await flushPromises();
    await wrapper.setProps({ pluginId: 'other' }); await flushPromises();
    expect(wrapper.findAll('li')).toHaveLength(0);
    expect(wrapper.text()).toContain('Could not load access records.');
    expect(wrapper.text()).not.toContain('private path');
    wrapper.unmount();
  });
});
