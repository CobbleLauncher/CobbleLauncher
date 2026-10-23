// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import BundledPluginsPanel from './BundledPluginsPanel.vue';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const manifest = { id: 'crash-doctor', name: 'Crash Doctor', version: '0.1.0', api_version: '0.1.0', publisher: 'Cobble', capabilities: ['report'], file_extensions: ['log'], component_sha256: 'a'.repeat(64) };
afterEach(() => { vi.mocked(invoke).mockReset(); });
describe('included plugin approval', () => {
  it('stages by known id and requests existing approval without installing or granting', async () => {
    vi.mocked(invoke).mockImplementation(async command => command === 'list_bundled_plugins' ? [manifest] : manifest);
    const wrapper = mount(BundledPluginsPanel, { props: { installedIds: [] } });
    await flushPromises();
    expect(wrapper.text()).toContain('Crash Doctor');
    await wrapper.get('li button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('preview_bundled_plugin', { id: 'crash-doctor' });
    expect(wrapper.emitted('prepared')).toEqual([[{ manifest, sourceName: '内置插件' }]]);
    expect(wrapper.emitted('preparing')).toEqual([[true], [false]]);
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_plugin')).toBe(false);
    vi.mocked(invoke).mockResolvedValue([]);
    await wrapper.setProps({ installedIds: ['crash-doctor'] }); await flushPromises();
    expect(wrapper.find('li').exists()).toBe(false);
    vi.mocked(invoke).mockResolvedValue([manifest]);
    await wrapper.setProps({ installedIds: [] }); await flushPromises();
    expect(wrapper.find('li').exists()).toBe(true);
    wrapper.unmount();
  });
  it('hides unavailable bundles and exposes a retry for loading errors', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    const empty = mount(BundledPluginsPanel, { props: { installedIds: [] } });
    await flushPromises(); expect(empty.find('section').exists()).toBe(false); empty.unmount();
    vi.mocked(invoke).mockRejectedValueOnce(new Error('Unreadable bundle')).mockResolvedValueOnce([manifest]);
    const wrapper = mount(BundledPluginsPanel, { props: { installedIds: [] } });
    await flushPromises(); expect(wrapper.get('[role="alert"]').text()).toContain('Unreadable bundle');
    await wrapper.get('button').trigger('click'); await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.text()).toContain('Crash Doctor'); wrapper.unmount();
  });
});

it('offers changed installed bundles through update approval without installing automatically', async () => {
  const previous = { ...manifest, component_sha256: 'b'.repeat(64) };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_bundled_plugins' ? [manifest] : manifest);
  const wrapper = mount(BundledPluginsPanel, { props: { installedIds: [manifest.id], installedManifests: [previous] } });
  await flushPromises();
  await wrapper.get('li button').trigger('click'); await flushPromises();
  expect(wrapper.emitted('prepared')).toEqual([[{ manifest, sourceName: '内置插件', updateFrom: previous }]]);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => ['install_plugin', 'update_plugin'].includes(command))).toBe(false);
  vi.mocked(invoke).mockResolvedValue([]);
  await wrapper.setProps({ installedManifests: [manifest] }); await flushPromises();
  expect(wrapper.find('li').exists()).toBe(false);
  wrapper.unmount();
});


it('filters Discover by manifest tags and keeps explicit installation approval', async () => {
  const candidate = { ...manifest, description: 'Diagnose game logs', tags: ['diagnostics'] };
  vi.mocked(invoke).mockResolvedValue([candidate]);
  const wrapper = mount(BundledPluginsPanel, { props: { installedIds: [], search: 'diagnostics' } });
  await flushPromises();
  expect(wrapper.get('.plugin-identity').text()).toContain('Diagnose game logs');
  expect(wrapper.findAll('.bundled-plugins>ul>li')).toHaveLength(1);
  await wrapper.setProps({ search: 'absent' });
  expect(wrapper.findAll('.bundled-plugins>ul>li')).toHaveLength(0);
  expect(wrapper.find('[role="status"]').exists()).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'preview_bundled_plugin')).toBe(false);
  wrapper.unmount();
});
