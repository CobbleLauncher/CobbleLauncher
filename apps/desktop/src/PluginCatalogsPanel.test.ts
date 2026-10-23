// SPDX-License-Identifier: MIT
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import PluginCatalogsPanel from './PluginCatalogsPanel.vue';
import { setLocale } from './i18n';
import { isSupportedCatalogPlugin, matchesPluginMetadata, type CatalogPlugin, type PluginCatalogView } from './plugin-catalogs';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const supported = {
  manifest: {
    id: 'test-viewer', name: 'Test Viewer', version: '1.0.0', api_version: '1.0.0',
    publisher: 'Test Publisher', capabilities: ['viewer'], file_extensions: ['schem'], component_sha256: 'a'.repeat(64),
  },
  description: 'Reads schematic files.',
  component_url: 'https://catalog.example.invalid/test-viewer.wasm',
};
const enabledCatalog: PluginCatalogView = {
  source: { id: 'catalog-1', name: 'Community', url: 'https://catalog.example.invalid/index.json', enabled: true },
  snapshot: {
    source_id: 'catalog-1', source_url: 'https://catalog.example.invalid/index.json', retrieved_at: 1,
    index: { name: 'Community catalog', schema_version: 1, plugins: [supported] },
  },
};

beforeEach(() => setLocale('en-US'));
it('accepts only a unique nonempty subset of the two supported viewer formats', () => {
  for (const extensions of [['schem'], ['litematic'], ['schem', 'litematic']]) {
    expect(isSupportedCatalogPlugin({ ...supported, manifest: { ...supported.manifest, file_extensions: extensions } })).toBe(true);
  }
  for (const extensions of [[], ['schem', 'schem'], ['litematic', 'exe'], ['schematic']]) {
    expect(isSupportedCatalogPlugin({ ...supported, manifest: { ...supported.manifest, file_extensions: extensions } })).toBe(false);
  }
});
it('supports the report contract without granting arbitrary capabilities or extensions', () => {
  const plugin = { ...supported, manifest: { ...supported.manifest, api_version: '0.1.0', capabilities: ['report'], file_extensions: ['log', 'txt'] } };
  expect(isSupportedCatalogPlugin(plugin)).toBe(true);
  for (const extensions of [[], ['exe'], ['log', 'log']]) {
    expect(isSupportedCatalogPlugin({ ...plugin, manifest: { ...plugin.manifest, file_extensions: extensions } })).toBe(false);
  }
  expect(isSupportedCatalogPlugin({ ...plugin, manifest: { ...plugin.manifest, capabilities: ['report', 'shell'] } })).toBe(false);
});
it('supports composer catalog entries only with bounded declared scopes', () => {
  const plugin = {
    ...supported,
    manifest: {
      ...supported.manifest,
      api_version: '0.1.0',
      capabilities: ['message-composer'],
      file_extensions: [],
      composer_permissions: ['draft', 'selected_session'],
      interfaces: { provides: [{ id: 'launcher:message-composer', version: '0.1.0' }] },
    },
  } as CatalogPlugin;
  const withManifest = (manifest: object) => ({ ...plugin, manifest }) as unknown as CatalogPlugin;
  expect(isSupportedCatalogPlugin(plugin)).toBe(true);
  expect(isSupportedCatalogPlugin(withManifest({ ...plugin.manifest, composer_permissions: ['selected_session'] }))).toBe(false);
  expect(isSupportedCatalogPlugin(withManifest({ ...plugin.manifest, composer_permissions: ['draft', 'filesystem'] }))).toBe(false);
  expect(isSupportedCatalogPlugin(withManifest({ ...plugin.manifest, file_extensions: ['txt'] }))).toBe(false);
});
afterEach(() => { vi.clearAllMocks(); setLocale('zh-CN'); });

it('loads cached listings without fetching or subscribing on mount', async () => {
  vi.mocked(invoke).mockResolvedValue([enabledCatalog]);
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('list_plugin_catalogs');
  expect(wrapper.text()).toContain('Test Viewer');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'add_plugin_catalog' || command === 'refresh_plugin_catalog')).toBe(false);
  wrapper.unmount();
});

it('shows an accessible skeleton during the initial disk-only catalog read', async () => {
  let resolve!: (value: PluginCatalogView[]) => void;
  const pendingRead = new Promise<PluginCatalogView[]>(finish => { resolve = finish; });
  vi.mocked(invoke).mockReturnValue(pendingRead as never);
  const wrapper = mount(PluginCatalogsPanel);
  expect(wrapper.get('[role="status"]').attributes('aria-live')).toBe('polite');
  expect(wrapper.get('[role="status"]').text()).toContain('Loading catalogs');
  expect(wrapper.findAll('.skeleton-row')).toHaveLength(3);
  resolve([enabledCatalog]);
  await flushPromises();
  expect(wrapper.find('.loading-skeleton').exists()).toBe(false);
  wrapper.unmount();
});

it('keeps cached entries visible when an explicit refresh fails', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_plugin_catalogs') return [enabledCatalog];
    if (command === 'refresh_plugin_catalog') throw new Error('network unavailable');
    return [enabledCatalog];
  });
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();
  await wrapper.get('button[aria-label="Refresh Community"]').trigger('click');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('refresh_plugin_catalog', { id: enabledCatalog.source.id });
  expect(wrapper.text()).toContain('Test Viewer');
  expect(wrapper.get('[role="alert"]').text()).toContain('network unavailable');
  wrapper.unmount();
});

it('uses a switch for source state and puts removal in its labeled actions menu', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_plugin_catalogs') return [enabledCatalog];
    if (command === 'set_plugin_catalog_enabled') return [{ ...enabledCatalog, source: { ...enabledCatalog.source, enabled: false } }];
    if (command === 'remove_plugin_catalog') return [];
    return [enabledCatalog];
  });
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();
  const toggle = wrapper.get('.catalog-actions [role="switch"]');
  expect(toggle.attributes('aria-label')).toBe('Enabled status for Community');
  expect(toggle.attributes('aria-checked')).toBe('true');
  await toggle.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_plugin_catalog_enabled', { id: 'catalog-1', enabled: false });
  expect(wrapper.findAll('.catalog-actions button').some(button => button.text().includes('Remove'))).toBe(false);
  await wrapper.get('button[aria-label="More actions for Community"]').trigger('click');
  await flushPromises();
  await new DOMWrapper(document.body).findAll('[role="menuitem"]').find(item => item.text() === 'Remove catalog')!.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('remove_plugin_catalog', { id: 'catalog-1' });
  wrapper.unmount();
});

it('keeps cached entries visible with compact feedback during refresh', async () => {
  let finishRefresh!: (value: PluginCatalogView[]) => void;
  const pendingRefresh = new Promise<PluginCatalogView[]>(resolve => { finishRefresh = resolve; });
  vi.mocked(invoke).mockImplementation(command => command === 'list_plugin_catalogs'
    ? Promise.resolve([enabledCatalog])
    : pendingRefresh as never);
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();
  await wrapper.get('button[aria-label="Refresh Community"]').trigger('click');
  expect(wrapper.get('.loading-indicator.compact').text()).toBe('Refreshing catalog…');
  expect(wrapper.text()).toContain('Test Viewer');
  finishRefresh([enabledCatalog]);
  await flushPromises();
  expect(wrapper.find('.loading-indicator').exists()).toBe(false);
  wrapper.unmount();
});

it('keeps a disabled feed browsable but blocks refresh and install', async () => {
  vi.mocked(invoke).mockResolvedValue([{ ...enabledCatalog, source: { ...enabledCatalog.source, enabled: false } }]);
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();

  expect(wrapper.text()).toContain('Disabled');
  expect(wrapper.get('button[aria-label="Refresh Community"]').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('button').find(button => button.text() === 'Install')!.attributes('disabled')).toBeDefined();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'refresh_plugin_catalog')).toBe(false);
  wrapper.unmount();
});

it('previews a compatible catalog plugin and emits source attribution for approval', async () => {
  const manifest = supported.manifest;
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugin_catalogs' ? [enabledCatalog] : manifest);
  const wrapper = mount(PluginCatalogsPanel);
  await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Install')!.trigger('click');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('preview_catalog_plugin', { sourceId: enabledCatalog.source.id, pluginId: manifest.id });
  expect(wrapper.emitted('prepared')?.[0]).toEqual([{ manifest, sourceName: enabledCatalog.source.name }]);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_plugin')).toBe(false);
  wrapper.unmount();
});


it('matches manifest descriptions and tags while retaining cached source actions', async () => {
  const plugin = { ...supported, manifest: { ...supported.manifest, description: { en: 'Inspect blueprints', 'zh-CN': '检查蓝图' }, tags: ['analysis'] } };
  expect(matchesPluginMetadata(plugin.manifest, 'analysis blueprints')).toBe(true);
  expect(matchesPluginMetadata(plugin.manifest, '检查')).toBe(true);
  vi.mocked(invoke).mockResolvedValue([{ ...enabledCatalog, snapshot: { ...enabledCatalog.snapshot!, index: { ...enabledCatalog.snapshot!.index, plugins: [plugin] } } }]);
  const wrapper = mount(PluginCatalogsPanel, { props: { search: 'analysis' } });
  await flushPromises();
  expect(wrapper.findAll('.catalog-entry')).toHaveLength(1);
  expect(wrapper.text()).toContain('Inspect blueprints');
  expect(wrapper.text()).toContain('Community catalog');
  await wrapper.setProps({ search: 'absent' });
  expect(wrapper.findAll('.catalog-entry')).toHaveLength(0);
  expect(wrapper.find('.catalog-actions button').exists()).toBe(true);
  wrapper.unmount();
});

it('supports only the current Provider ABI', () => {
  const plugin = { ...supported, manifest: { ...supported.manifest, api_version: '0.2.0', capabilities: ['blueprint-provider'], file_extensions: [], provider_origins: ['https://catalog.example'] } };
  expect(isSupportedCatalogPlugin(plugin)).toBe(true);
  expect(isSupportedCatalogPlugin({ ...plugin, manifest: { ...plugin.manifest, api_version: '0.1.0' } })).toBe(false);
});


it.each([
  { description: 'Updated manifest description' },
  { icon: { file: 'icon.png', sha256: 'b'.repeat(64) } },
  { contributions: { schema_version: 1, commands: [], settings: null } },
])('reviews same-version same-component manifest changes: %j', async change => {
  const candidate = { ...supported.manifest, ...change };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugin_catalogs' ? [{ ...enabledCatalog, snapshot: { ...enabledCatalog.snapshot!, index: { ...enabledCatalog.snapshot!.index, plugins: [{ ...supported, manifest: candidate }] } } }] : candidate);
  const wrapper = mount(PluginCatalogsPanel, { props: { installedManifests: [supported.manifest] } });
  await flushPromises();
  const review = wrapper.findAll('.catalog-entry button').find(button => button.text() === 'Review update')!;
  expect(review.attributes('disabled')).toBeUndefined();
  await review.trigger('click'); await flushPromises();
  expect(wrapper.emitted('prepared')?.[0]).toEqual([{ manifest: candidate, sourceName: enabledCatalog.source.name, updateFrom: supported.manifest }]);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'update_plugin')).toBe(false);
  wrapper.unmount();
});

it('treats reordered keys and omitted empty optional fields as the installed manifest', async () => {
  const candidate = { provider_origins: [], tags: [], report_protocols: [], ...supported.manifest };
  vi.mocked(invoke).mockResolvedValue([{ ...enabledCatalog, snapshot: { ...enabledCatalog.snapshot!, index: { ...enabledCatalog.snapshot!.index, plugins: [{ ...supported, manifest: candidate }] } } }]);
  const wrapper = mount(PluginCatalogsPanel, { props: { installedManifests: [supported.manifest] } });
  await flushPromises();
  const installed = wrapper.findAll('.catalog-entry button').find(button => button.text() === 'Installed')!;
  expect(installed.attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('selects the exact candidate source while leaving review explicit', async () => {
  vi.mocked(invoke).mockResolvedValue([enabledCatalog, {...enabledCatalog,source:{...enabledCatalog.source,id:'another-source',name:'Other source'}}]);
  const wrapper=mount(PluginCatalogsPanel,{props:{candidate:{sourceId:'catalog-1',pluginId:'test-viewer'},search:'test-viewer'}});
  await flushPromises();
  expect(wrapper.findAll('.catalog-source')).toHaveLength(1);
  expect(wrapper.text()).toContain('Community');
  expect(wrapper.text()).not.toContain('Other source');
  expect(vi.mocked(invoke).mock.calls.every(([command])=>command==='list_plugin_catalogs')).toBe(true);
  await wrapper.setProps({search:''});
  expect(wrapper.findAll('.catalog-source')).toHaveLength(2);
  wrapper.unmount();
});

it('recognizes the bounded report formats used by configuration diagnostics',()=>{
 const plugin={...supported,manifest:{...supported.manifest,api_version:'0.2.0',capabilities:['report'],file_extensions:['log','txt','json','mrpack','toml','properties','yaml','yml']}};
 expect(isSupportedCatalogPlugin(plugin)).toBe(true);
 expect(isSupportedCatalogPlugin({...plugin,manifest:{...plugin.manifest,file_extensions:[...plugin.manifest.file_extensions,'cfg']}})).toBe(false);
});

it('recognizes image layout interfaces only with the two explicit bounded image permissions',()=>{
 const manifest={...supported.manifest,api_version:'0.1.0',capabilities:['image-layout'],file_extensions:[],permissions:[{permission:'launcher:images/selected/read' as const},{permission:'launcher:images/output/propose' as const}],interfaces:{provides:[{id:'launcher:image-layout',version:'0.1.0'}]}};
 expect(isSupportedCatalogPlugin({...supported,manifest})).toBe(true);expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,permissions:[]}})).toBe(false);expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,file_extensions:['png']}})).toBe(false);expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,interfaces:{provides:[{id:'launcher:image-layout',version:'0.2.0'}]}}})).toBe(false);
});

it('offers config-folder reports only with the exact protocol, API and selected-folder permission',()=>{
 const manifest={...supported.manifest,api_version:'0.2.0',capabilities:['report'],file_extensions:['json','toml'],report_protocols:['cobble.config-folder@1'],permissions:[{permission:'launcher:resource/selected/read' as const,extensions:['json','toml']},{permission:'launcher:resource/selected-folder/read' as const,extensions:['json','toml']}]};
 expect(isSupportedCatalogPlugin({...supported,manifest})).toBe(true);
 expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,api_version:'0.1.0'}})).toBe(false);
 expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,permissions:[manifest.permissions[0]!]}})).toBe(false);
 expect(isSupportedCatalogPlugin({...supported,manifest:{...manifest,file_extensions:['log']}})).toBe(false);
});
