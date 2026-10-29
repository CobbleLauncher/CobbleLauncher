// SPDX-License-Identifier: MIT
/* eslint-disable vue/one-component-per-file -- test doubles expose native view leave guards */
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import PluginReportPanel from './PluginReportPanel.vue';
import PluginWebView from './PluginWebView.vue';
import ToolsPage from './ToolsPage.vue';
import { clearPageWorkspacesForTests, usePageWorkspace } from './use-page-workspace';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => { clearPageWorkspacesForTests(); });
afterEach(() => { vi.useRealTimers(); vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('opens a generic form tool with only a visible version default and runs its leave hook', async () => {
  const form = { schema_version: 1, schema: { type: 'object', additionalProperties: false, properties: { version: { type: 'string', enum: ['1.21.1'] } } } };
  const manifest = { id: 'community-tool', name: 'Community tool', api_version: '0.1.0', capabilities: ['tool-form'], file_extensions: [], tool_form: form };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ enabled: true, manifest }] : []);
  const leave = vi.fn(async () => true);
  const Form = defineComponent({ name: 'ToolFormPanel', props: { pluginId: { type: String, required: true }, form: { type: Object, required: true }, contextVersion: { type: String, default: undefined } }, setup(_props, { expose }) { expose({ confirmLeave: leave }); }, template: '<div class="test-tool-form" />' });
  const wrapper = mount(ToolsPage, { props: { location: { pluginId: manifest.id, instanceId: 'game' }, games: [{ id: 'game', name: 'Game', version: '1.21.1', directory: 'private-path', status: 'installed', error: null, progress: null }] }, global: { stubs: { ToolFormPanel: Form } } });
  await flushPromises();
  const panel = wrapper.getComponent({ name: 'ToolFormPanel' });
  expect(panel.props()).toEqual({ pluginId: manifest.id, form, contextVersion: '1.21.1' });
  expect(invoke).not.toHaveBeenCalledWith('run_plugin_tool_form', expect.anything());
  expect(await wrapper.vm.confirmLeave()).toBe(true);
  expect(leave).toHaveBeenCalledOnce();
  wrapper.unmount();
});

it('opens a mixed inspector frontend first while retaining its native pack-file workflow', async () => {
  setLocale('en-US');
  const manifest = { id: 'modpack-inspector', name: 'Inspector', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['json', 'mrpack'], report_protocols: ['cobble.mrpack-page@1'], frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'a'.repeat(64) } };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ enabled: true, manifest }] : []);
  const Frontend = defineComponent({ name: 'PluginWebView', setup(_props, { expose }) { expose({ confirmLeave: async () => true }); }, template: '<div class="test-plugin-web-view" />' });
  const wrapper = mount(ToolsPage, { props: { location: { pluginId: manifest.id, instanceId: 'game' } }, global: { stubs: { PluginWebView: Frontend, InspectorPanel: true } } });
  await flushPromises();
  expect(wrapper.find('.test-plugin-web-view').exists()).toBe(true);
  expect(wrapper.find('inspector-panel-stub').exists()).toBe(false);
  await wrapper.findAll('.inspector-modes button')[1]!.trigger('click');
  await flushPromises();
  expect(wrapper.find('.test-plugin-web-view').exists()).toBe(false);
  expect(wrapper.getComponent({ name: 'InspectorPanel' }).props('instanceId')).toBe('game');
  await wrapper.findAll('.inspector-modes button')[0]!.trigger('click');
  await flushPromises();
  wrapper.getComponent(PluginWebView).vm.$emit('unsupported');
  await flushPromises();
  expect(wrapper.find('inspector-panel-stub').exists()).toBe(true);
  expect(wrapper.find('.inspector-modes').exists()).toBe(false);
  wrapper.unmount();
});

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
  expect(wrapper.get('button[aria-label="Logbook"]').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('button').find(button => button.text().includes('Blueprint library'))!.attributes('disabled')).toBeUndefined();
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
    props: { initialGameId: 'previous-game', location: { pluginId: 'config-doctor', keybindingTargetId: 'new-game', handoffId: 'handoff-1' } },
    global: { stubs: { PluginWebView: true, PluginReportPanel: true, InspectorPanel: true } },
  });
  await flushPromises();
  expect(wrapper.findComponent(PluginWebView).props('keybindingTargetId')).toBe('new-game');
  expect(wrapper.findComponent(PluginWebView).props('handoffId')).toBe('handoff-1');
  expect(wrapper.findComponent(PluginWebView).props('instanceId')).toBeUndefined();
  wrapper.unmount();
});
it('honors a cancelled custom-plugin draft leave request', async () => {
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_plugins' ? [{ enabled: true, manifest: { id: 'custom-page', name: 'Custom page', api_version: '0.2.0', capabilities: ['report'], file_extensions: [], frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'a'.repeat(64) } } }] : []));
  const Frontend = defineComponent({ name: 'PluginWebView', setup(_props, { expose }) { expose({ confirmLeave: async () => false }); }, template: '<div class="test-plugin-web-view" />' });
  const wrapper = mount(ToolsPage, { props: { location: { pluginId: 'custom-page' } }, global: { stubs: { PluginWebView: Frontend } } });
  await flushPromises();
  expect(await wrapper.vm.confirmLeave()).toBe(false);
  expect(wrapper.find('.test-plugin-web-view').exists()).toBe(true);
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

it('retries the failed tool command with its original game context instead of reloading the catalog', async () => {
  setLocale('en-US');
  let resolutions = 0;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_plugins') return [{enabled: true, manifest: {id: 'report', name: 'Logbook', api_version: '0.2.0', capabilities: ['report'], file_extensions: ['log']}}];
    if (command === 'resolve_extension_command') {
      if (++resolutions === 1) throw new Error('temporarily unavailable');
      return {pluginId: 'report', commandId: 'analyze', label: 'Analyze', workflow: 'report'};
    }
    return [{pluginId: 'report', settings: [], commands: [{id: 'analyze', label: 'Analyze', workflow: 'report', menus: [{position: 'tools', when: 'always'}]}]}];
  });
  const game = (id: string, last_launched_at: number) => ({id, name: id, last_launched_at, version: '1.21.1', directory: id, status: 'installed' as const, error: null, progress: null});
  const wrapper = mount(ToolsPage, {props: {games: [game('original', 10), game('newer', 5)]}, global: {stubs: {PluginReportPanel: true}}});
  await flushPromises(); await wrapper.get('[data-tool-id="report"]').trigger('click'); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  await wrapper.setProps({games: [game('original', 10), game('newer', 20)]});
  await wrapper.get('[role="alert"] button').trigger('click'); await flushPromises();
  expect(resolutions).toBe(2);
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'list_plugins')).toHaveLength(1);
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'resolve_extension_command').at(-1)?.[1]).toMatchObject({pluginId: 'report', commandId: 'analyze', context: {instanceId: 'original'}});
  expect(wrapper.findComponent(PluginReportPanel).props('instanceId')).toBe('original');
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  wrapper.unmount();
});

it('shows catalog loading and a retryable failure without claiming the saved search has no matches', async () => {
  setLocale('en-US');
  vi.useFakeTimers({toFake: ['setTimeout', 'clearTimeout']});
  usePageWorkspace('tools-catalog', undefined, () => ({query: 'saved search', page: 2, returnToolId: '', scrollTop: 0, restoreReturn: false}));
  let reject!: (cause: Error) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'list_plugins' ? new Promise((_resolve, fail) => {reject = fail;}) : Promise.resolve([]));
  const wrapper = mount(ToolsPage);
  expect(wrapper.find('.tool-empty').exists()).toBe(false);
  expect(wrapper.find('.loading-indicator').exists()).toBe(false);
  await vi.advanceTimersByTimeAsync(140);
  expect(wrapper.get('.loading-indicator').text()).toContain('Loading');
  reject(new Error('local catalog unavailable')); await flushPromises();
  expect(wrapper.find('.tool-empty').exists()).toBe(false);
  expect(wrapper.find('.loading-indicator').exists()).toBe(false);
  expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  expect(wrapper.get('input[type="search"]').element).toHaveProperty('value', 'saved search');
  vi.mocked(invoke).mockResolvedValue([]);
  await wrapper.get('[role="alert"] button').trigger('click'); await flushPromises();
  expect(wrapper.get('.tool-empty').text()).toBe('No tools found.');
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  wrapper.unmount();
});
