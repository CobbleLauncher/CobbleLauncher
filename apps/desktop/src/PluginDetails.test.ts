// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import PluginDetails from './PluginDetails.vue';
import PluginsPanel from './PluginsPanel.vue';
import { hasGlobalPluginSettings, type PluginDetailsData } from './plugin-details';
import { isSupportedCatalogPlugin } from './plugin-catalogs';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const manifest = { id: 'detail-viewer', name: 'Detail Viewer', description: 'Preview blueprints', version: '1', api_version: '1.0.0', publisher: 'Example', capabilities: ['viewer'], file_extensions: ['schem'], component_sha256: 'a'.repeat(64) };
const snapshot = (): PluginDetailsData => ({ plugin: { manifest, enabled: false, problem: 'PLUGIN_MISSING_INTERFACE' }, provides: [{ id: 'launcher:viewer', version: '1.0.0' }], dependencies: [], companions: [] });
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); });
afterEach(() => { setLocale('zh-CN'); });

it('shows native installed signature status and never asks pending approval for details', async () => {
  const data = { ...snapshot(), signature: { status: 'verified', rootId: 'a'.repeat(64), publisher: 'Approved publisher' } };
  vi.mocked(invoke).mockImplementation(async command => command === 'get_plugin_details' ? data : []);
  const wrapper = mount(PluginDetails, { props: { pluginId: manifest.id } }); await flushPromises();
  expect(wrapper.get('.plugin-signature-status').text()).toContain('Approved publisher');
  expect(wrapper.get('.plugin-signature-status').attributes('title')).toBe('a'.repeat(64));
  expect(invoke).not.toHaveBeenCalledWith('get_pending_plugin_signature');
  wrapper.unmount();
});

it('keeps unavailable extensions inspectable and removable without exposing raw failure codes', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_plugin_details' ? snapshot() : []);
  const wrapper = mount(PluginDetails, { props: { pluginId: manifest.id } }); await flushPromises();
  expect(wrapper.emitted('title')).toEqual([[manifest.name]]);
  expect(wrapper.find('h1').exists()).toBe(false);
  expect(wrapper.text()).not.toContain('PLUGIN_MISSING_INTERFACE');
  const overview = wrapper.get('.plugin-details-overview');
  expect(overview.text()).toContain('Example · 1');
  expect(overview.findAll('button').map(button => button.text())).toEqual(['Enable', 'Uninstall']);
  expect(overview.findAll('button svg')).toHaveLength(2);
  expect(wrapper.get('.plugin-manifest').text()).toContain('"id": "detail-viewer"');
  expect(wrapper.find('.extension-settings-panel').exists()).toBe(false);
  const remove = wrapper.findAll('button').find(button => button.text() === 'Uninstall')!;
  await remove.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('uninstall_plugin', { id: manifest.id });
  expect(wrapper.emitted('removed')).toEqual([[]]);
  wrapper.unmount();
});

it('hands installed and cached compatible combinations to detail and approval navigation only', async () => {
  const data = snapshot();
  data.dependencies = [{ requirement: { id: 'launcher:blueprint-provider', version: '^0.2.0' }, optional: true, state: 'optional_missing', implementations: [{ plugin_id: 'inactive', name: 'Inactive source', version: '2', enabled: false, available: false }], candidates: [{ source_id: 'catalog', source_name: 'Community', plugin_id: 'candidate', name: 'Candidate source', version: '3', project_url: null }] }];
  data.companions = [{ plugin_id: 'consumer', name: 'Compatible consumer', version: '4', requirement: { id: 'launcher:viewer', version: '^1.0.0' }, optional: true, installed: false, enabled: false, available: false, source_id: 'catalog', source_name: 'Community', project_url: null }];
  vi.mocked(invoke).mockResolvedValue(data);
  const wrapper = mount(PluginDetails, { props: { pluginId: manifest.id } }); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Details')!.trigger('click');
  expect(wrapper.emitted('openDetail')).toEqual([['inactive']]);
  const review = wrapper.findAll('button').filter(button => button.text() === 'Review installation');
  await review[0]!.trigger('click'); await review[1]!.trigger('click');
  expect(wrapper.emitted('browseCatalog')).toEqual([[{ sourceId: 'catalog', pluginId: 'candidate' }], [{ sourceId: 'catalog', pluginId: 'consumer' }]]);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_plugin' || command === 'preview_catalog_plugin')).toBe(false);
  wrapper.unmount();
});

it('reuses global schema controls and forwards the settings draft leave guard', async () => {
  const data = snapshot(); data.plugin.enabled = true; data.plugin.problem = null;
  data.plugin.manifest = { ...manifest, contributions: { schema_version: 1, commands: [], settings: { categories: [{ id: 'general', fields: [{ scope: 'global' }] }] } } };
  expect(hasGlobalPluginSettings(data.plugin.manifest)).toBe(true);
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_plugin_details') return data;
    if (command === 'list_extension_contributions') return [];
    if (command === 'get_extension_settings') return { pluginId: manifest.id, values: { title: 'Saved' }, categories: [{ id: 'general', label: 'General', fields: [{ id: 'title', type: 'string', scope: 'global', label: 'Title', default: 'Saved', options: [], advanced: false, maxLength: 30 }] }] };
    return [];
  });
  const wrapper = mount(PluginDetails, { props: { pluginId: manifest.id } }); await flushPromises();
  expect(wrapper.findComponent({ name: 'ExtensionSettingsPanel' }).exists()).toBe(true);
  await wrapper.get('input[type="text"]').setValue('Unsaved');
  const api = wrapper.vm as unknown as { canLeave(): boolean };
  expect(api.canLeave()).toBe(false);
  wrapper.unmount();
});

it('ignores stale detail responses and never runs plugin commands from management details', async () => {
  let finish!: (value: PluginDetailsData) => void;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_plugin_details' && (args as { id: string }).id === manifest.id) return new Promise(resolve => { finish = resolve; });
    if (command === 'get_plugin_details') return { ...snapshot(), plugin: { manifest: { ...manifest, id: 'new-viewer', name: 'New Viewer' }, enabled: true, problem: null } };
    if (command === 'list_extension_contributions') return [{ pluginId: 'new-viewer', commands: [{ id: 'new-viewer.open', label: 'Preview', menus: [{ position: 'tools' }] }] }];
    if (command === 'resolve_extension_command') return { pluginId: 'new-viewer', commandId: 'new-viewer.open', label: 'Preview', workflow: 'viewer' };
    return [];
  });
  const wrapper = mount(PluginDetails, { props: { pluginId: manifest.id } }); await flushPromises();
  await wrapper.setProps({ pluginId: 'new-viewer' }); await flushPromises();
  finish(snapshot()); await flushPromises();
  expect(wrapper.emitted('title')).toEqual([['New Viewer']]);
  expect(wrapper.findAll('button').some(button => button.text().includes('Preview'))).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'list_extension_contributions' || command === 'resolve_extension_command')).toBe(false);
  expect(wrapper.emitted('command')).toBeUndefined();
  wrapper.unmount();
});

it('exposes the details action even when the installed plugin is disabled', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [snapshot().plugin] : []);
  const wrapper = mount(PluginsPanel); await flushPromises();
  await wrapper.get('.plugin-list .plugin-identity-button').trigger('click');
  expect(wrapper.emitted('openDetail')).toEqual([[manifest.id]]);
  wrapper.unmount();
});

it('marks explicitly mismatched known provided ABIs unsupported', () => {
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, interfaces: { provides: [{ id: 'launcher:viewer', version: '2.0.0' }] } }, description: '', component_url: 'https://catalog.example/plugin.wasm' })).toBe(false);
});
