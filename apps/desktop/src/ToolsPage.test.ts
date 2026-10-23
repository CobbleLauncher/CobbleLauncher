// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import PluginReportPanel from './PluginReportPanel.vue';
import PluginWebView from './PluginWebView.vue';
import ToolsPage from './ToolsPage.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('a late extension resolution cannot replace the newly selected blueprint tool', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => {
    if (command === 'resolve_extension_command') return new Promise(resolve => { finish = resolve; });
    if (command === 'list_plugins') return Promise.resolve([{ enabled: true, manifest: { id: 'report', name: 'Logbook', description: 'Summarizes game logs', tags: ['logs'], api_version: '0.2.0', capabilities: ['report'], file_extensions: ['log'] } }]);
    return Promise.resolve([{ pluginId: 'report', pluginName: 'Report', settings: [], commands: [{ id: 'analyze', label: 'Analyze', workflow: 'report', menus: [{ position: 'tools', when: 'always' }] }] }]);
  });
  setLocale('en-US');
  const wrapper = mount(ToolsPage, { global: { stubs: { BlueprintsPage: true, PluginReportPanel: true } } });
  await flushPromises();
  expect(wrapper.text()).toContain('Logbook');
  expect(wrapper.text()).toContain('Summarizes game logs');
  await wrapper.findAll('button').find(button => button.attributes('aria-label') === 'Logbook')!.trigger('click');
  await wrapper.findAll('button').find(button => button.text().includes('Blueprint library'))!.trigger('click');
  finish({ pluginId: 'report', commandId: 'analyze', label: 'Analyze', workflow: 'report' });
  await flushPromises();
  expect(wrapper.find('blueprints-page-stub').exists()).toBe(true);
  expect(wrapper.find('plugin-report-panel-stub').exists()).toBe(false);
  wrapper.unmount();
});

it('uses a viewer command instance as the blueprint workspace game', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const BlueprintsStub = defineComponent({
    props: { initialGameId: { type: String, default: '' } },
    template: '<div data-testid="blueprints-stub" :data-initial-game="initialGameId" />',
  });
  const wrapper = mount(ToolsPage, {
    props: {
      initialGameId: 'game-a',
      selectedCommand: { pluginId: 'viewer', commandId: 'open', label: 'Open schematic', workflow: 'viewer', instanceId: 'game-b' },
    },
    global: { stubs: { BlueprintsPage: BlueprintsStub, PluginReportPanel: true } },
  });
  await flushPromises();
  expect(wrapper.get('[data-testid="blueprints-stub"]').attributes('data-initial-game')).toBe('game-b');
  wrapper.unmount();
});

it('reports controlled blueprint navigation and removes the duplicate back button', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  setLocale('en-US');
  const wrapper = mount(ToolsPage, { props: { location: { pluginId: 'builtin:blueprints', blueprintView: 'library', instanceId: 'context-game' } }, global: { stubs: { BlueprintsPage: true } } });
  await flushPromises();
  expect(wrapper.text()).not.toContain('All tools');
  expect(wrapper.getComponent({ name: 'BlueprintsPage' }).props('initialGameId')).toBe('context-game');
  wrapper.getComponent({ name: 'BlueprintsPage' }).vm.$emit('navigate', { view: 'online' });
  expect(wrapper.emitted('navigate')?.at(-1)).toEqual([{ pluginId: 'builtin:blueprints', command: undefined, instanceId: 'context-game', blueprintView: 'online', title: undefined }]);
  await wrapper.setProps({ location: undefined });
  expect(wrapper.find('.tool-catalog').exists()).toBe(true);
  wrapper.unmount();
});

it('reports the real loaded report title to the unified page model', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ enabled: true, manifest: { id: 'report', name: 'Logbook', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['log'] } }] : []);
  const wrapper = mount(ToolsPage, { props: { location: { pluginId: 'report', instanceId: 'context-game' } }, global: { stubs: { PluginReportPanel: true } } });
  await flushPromises();
  expect(wrapper.emitted('navigate')?.at(-1)).toEqual([{ pluginId: 'report', instanceId: 'context-game', title: 'Logbook' }]);
  wrapper.unmount();
});

it('passes the actual installed manifest to folder-capable report tools without plugin ID special cases',async()=>{
 const manifest={id:'community-config-reader',name:'Community configuration reader',api_version:'0.2.0',capabilities:['report'],file_extensions:['json'],report_protocols:['cobble.config-folder@1'],permissions:[{permission:'launcher:resource/selected-folder/read',extensions:['json']}]};
 vi.mocked(invoke).mockImplementation(async command=>command==='list_plugins'?[{enabled:true,manifest}]:[]);
 const wrapper=mount(ToolsPage,{props:{selectedPluginId:manifest.id},global:{stubs:{PluginReportPanel:true}}});await flushPromises();expect(wrapper.findComponent(PluginReportPanel).props('manifest')).toEqual(manifest);wrapper.unmount();
});
it('opens the bundled frontend with its game context and falls back on unsupported runtimes', async () => {
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_plugins' ? [{ enabled: true, manifest: { id: 'custom-config', name: 'Custom config', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['json'], frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'a'.repeat(64) } } }] : []));
  const wrapper = mount(ToolsPage, {
    props: { location: { pluginId: 'custom-config', instanceId: 'game-a' } },
    global: { stubs: { PluginWebView: true, PluginReportPanel: true, InspectorPanel: true } },
  });
  await flushPromises();
  expect(wrapper.findComponent(PluginWebView).props('instanceId')).toBe('game-a');
  wrapper.findComponent(PluginWebView).vm.$emit('unsupported');
  await flushPromises();
  expect(wrapper.findComponent(PluginReportPanel).props('pluginId')).toBe('custom-config');
  expect(wrapper.findComponent(PluginReportPanel).props('instanceId')).toBe('game-a');
  expect(wrapper.find('plugin-web-view-stub').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'open_plugin_view')).toBe(false);
  wrapper.unmount();
});

it('passes a keybinding target from tool navigation to the plugin view', async () => {
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_plugins' ? [{ enabled: true, manifest: { id: 'config-doctor', name: 'Config Doctor', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['toml'], frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'a'.repeat(64) } } }] : []));
  const wrapper = mount(ToolsPage, {
    props: { initialGameId: 'previous-game', location: { pluginId: 'config-doctor', keybindingTargetId: 'new-game' } },
    global: { stubs: { PluginWebView: true, PluginReportPanel: true, InspectorPanel: true } },
  });
  await flushPromises();
  expect(wrapper.findComponent(PluginWebView).props('keybindingTargetId')).toBe('new-game');
  expect(wrapper.findComponent(PluginWebView).props('instanceId')).toBeUndefined();
  wrapper.unmount();
});

it('retains game context when opening a command from the catalog', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_plugins') return [{ enabled: true, manifest: { id: 'report', name: 'Logbook', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['log'] } }];
    if (command === 'resolve_extension_command') return { pluginId: 'report', commandId: 'analyze', label: 'Analyze', workflow: 'report' };
    return [{ pluginId: 'report', commands: [{ id: 'analyze', label: 'Analyze', workflow: 'report', menus: [{ position: 'tools', when: 'always' }] }], settings: [] }];
  });
  const wrapper = mount(ToolsPage, { props: { initialGameId: 'current-game' }, global: { stubs: { PluginReportPanel: true } } });
  await flushPromises();
  await wrapper.get('[aria-label="Logbook"]').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('resolve_extension_command', { pluginId: 'report', commandId: 'analyze', position: 'tools', locale: 'zh-CN', context: { instanceId: 'current-game' } });
  expect(wrapper.findComponent(PluginReportPanel).props('instanceId')).toBe('current-game');
  expect(wrapper.emitted('navigate')?.at(-1)?.[0]).toMatchObject({ instanceId: 'current-game' });
  wrapper.unmount();
});

it('opens an unbound tool with the most recently launched installed game', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ enabled: true, manifest: { id: 'inspector', name: 'Inspector', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['mrpack'], report_protocols: ['cobble.mrpack-page@1'] } }] : []);
  const game = (id: string, last_launched_at: number, status: 'installed' | 'installing') => ({ id, name: id, version: '1.21.1', directory: id, status, last_launched_at, error: null, progress: null });
  const wrapper = mount(ToolsPage, { props: { selectedPluginId: 'inspector', games: [game('older', 1, 'installed'), game('recent', 8, 'installed'), game('installing', 10, 'installing')] }, global: { stubs: { InspectorPanel: true } } });
  await flushPromises();
  expect(wrapper.getComponent({ name: 'InspectorPanel' }).props('instanceId')).toBe('recent');
  wrapper.unmount();
});
