// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import PluginsPanel from './PluginsPanel.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const manifest = { id: 'owned.viewer', name: 'Schematic Viewer', version: '1.0', api_version: '1.0.0', publisher: 'Cobble', capabilities: ['viewer'], file_extensions: ['schem'] };
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
async function choosePluginAction(wrapper: ReturnType<typeof mount>, label: string) {
  await wrapper.get('button[aria-haspopup="menu"][aria-label*="Schematic Viewer"]').trigger('click');
  await flushPromises();
  await new DOMWrapper(document.body).findAll('[role="menuitem"]').find(item => item.text().includes(label))!.trigger('click');
  await flushPromises();
}
describe('local plugin approval', () => {
  it('keeps damaged plugins removable without offering to run them', async () => {
    vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ manifest, enabled: false, problem: 'invalid component' }] : []);
    const wrapper = mount(PluginsPanel); await flushPromises();
    expect(wrapper.get('.plugin-enable-toggle').attributes('disabled')).toBeDefined();
    expect(wrapper.find('.plugin-open-action').exists()).toBe(false);
    await choosePluginAction(wrapper, '卸载');
    expect(invoke).toHaveBeenCalledWith('uninstall_plugin', { id: manifest.id });
    expect(wrapper.find('.plugin-list li').exists()).toBe(false);
    wrapper.unmount();
  });
  it('can retry a failed initial listing without reopening settings', async () => {
    let attempts = 0;
    vi.mocked(invoke).mockImplementation(async command => {
      if (command === 'list_plugin_catalogs') return [];
      if (command === 'list_plugins' && attempts++ === 0) throw new Error('temporary read failure');
      return [{ manifest, enabled: true }];
    });
    const wrapper = mount(PluginsPanel); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('temporary read failure');
    await wrapper.findAll('button').find(button => button.text() === '重试' || button.text() === 'Retry')!.trigger('click'); await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.findAll('.plugin-list li')).toHaveLength(1);
    wrapper.unmount();
  });
  it('requires approval after a preview and passes the opaque plugin id to mutations', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'preview_plugin_package') return manifest;
      if (command === 'install_plugin') return [{ manifest, enabled: true }];
      if (command === 'set_plugin_enabled') return [{ manifest, enabled: false }];
      return [];
    });
    const wrapper = mount(PluginsPanel); await flushPromises();
    await wrapper.get('.extensions-install').trigger('click'); await flushPromises();
    expect(wrapper.get('.plugin-approval').text()).toContain('.schem');
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_plugin')).toBe(false);
    await wrapper.get('.plugin-approval .primary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('install_plugin', { id: manifest.id, reviewedManifest: manifest });
    await wrapper.get('.plugin-enable-toggle').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('set_plugin_enabled', { id: manifest.id, enabled: false });
    await choosePluginAction(wrapper, '卸载');
    expect(invoke).toHaveBeenCalledWith('uninstall_plugin', { id: manifest.id });
    expect(wrapper.find('.plugin-list li').exists()).toBe(false);
    wrapper.unmount();
  });
  it('shows only the declared composer scopes before installation approval', async () => {
    const composer = {
      id: 'session-card', name: 'Session Card', version: '0.1.0', api_version: '0.1.0',
      publisher: 'Cobble', capabilities: ['message-composer'], file_extensions: [],
      composer_permissions: ['draft', 'selected_session'],
    };
    setLocale('en-US');
    vi.mocked(invoke).mockImplementation(async command => command === 'preview_plugin_package' ? composer : []);
    const wrapper = mount(PluginsPanel); await flushPromises();
    await wrapper.get('.extensions-install').trigger('click'); await flushPromises();
    const approval = wrapper.get('.plugin-approval').text();
    expect(approval).toContain('draft text');
    expect(approval).toContain('the selected game session');
    expect(approval).not.toContain('public profile');
    expect(approval).not.toContain('selected player');
    wrapper.unmount();
  });
  it('retains approval on install failure and cancellation never installs', async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'preview_plugin_package') return manifest;
      if (command === 'install_plugin') throw new Error('digest changed');
      return [];
    });
    const wrapper = mount(PluginsPanel); await flushPromises();
    await wrapper.get('.extensions-install').trigger('click'); await flushPromises();
    await wrapper.get('.plugin-approval .primary-button').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('digest changed');
    expect(wrapper.find('.plugin-approval').exists()).toBe(true);
    const count = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'install_plugin').length;
    await wrapper.get('.plugin-approval .text-button').trigger('click');
    expect(wrapper.find('.plugin-approval').exists()).toBe(false);
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'install_plugin')).toHaveLength(count);
    wrapper.unmount();
  });
  it('shows catalog source attribution and cancels the staged preview', async () => {
    setLocale('zh-CN');
    const catalog = {
      source: { id: 'catalog-1', name: 'Community', url: 'https://catalog.example.invalid/index.json', enabled: true },
      snapshot: { source_id: 'catalog-1', source_url: 'https://catalog.example.invalid/index.json', retrieved_at: 1, index: { name: 'Community catalog', schema_version: 1, plugins: [{ manifest: { ...manifest, component_sha256: 'a'.repeat(64) }, description: 'A viewer', component_url: 'https://catalog.example.invalid/viewer.wasm' }] } },
    };
    vi.mocked(invoke).mockImplementation(async command => {
      if (command === 'list_plugins') return [];
      if (command === 'list_bundled_plugins') return [];
      if (command === 'list_plugin_catalogs') return [catalog];
      if (command === 'preview_catalog_plugin') return manifest;
      return null;
    });
    const wrapper = mount(PluginsPanel); await flushPromises();
    await flushPromises();
    await wrapper.findAll('.catalog-entry button').find(button => button.text() === '安装')!.trigger('click'); await flushPromises();
    expect(wrapper.get('.plugin-approval').text()).toContain('Community');
    await wrapper.get('.plugin-approval .text-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('cancel_plugin_preview');
    expect(wrapper.find('.plugin-approval').exists()).toBe(false);
    wrapper.unmount();
  });
  it('offers reviewed catalog updates and shows publisher and permission changes before approval', async () => {
    setLocale('zh-CN');
    const installed = { ...manifest, api_version: '0.2.0', version: '1.0', publisher: 'Old Publisher', component_sha256: 'a'.repeat(64), capabilities: ['blueprint-provider'], file_extensions: [], provider_origins: ['https://old.example.invalid'] };
    const candidate = { ...manifest, api_version: '0.2.0', version: '2.0', publisher: 'New Publisher', component_sha256: 'b'.repeat(64), capabilities: ['blueprint-provider'], file_extensions: [], provider_origins: ['https://new.example.invalid'] };
    const catalog = {
      source: { id: 'catalog-1', name: 'Community', url: 'https://catalog.example.invalid/index.json', enabled: true },
      snapshot: { source_id: 'catalog-1', source_url: 'https://catalog.example.invalid/index.json', retrieved_at: 1, index: { name: 'Community catalog', schema_version: 1, plugins: [{ manifest: candidate, description: 'Search blueprints', component_url: 'https://catalog.example.invalid/provider.wasm' }] } },
    };
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'list_plugins') return [{ manifest: installed, enabled: true }];
      if (command === 'list_bundled_plugins') return [];
      if (command === 'list_plugin_catalogs') return [catalog];
      if (command === 'preview_catalog_plugin') return candidate;
      if (command === 'update_plugin') return [{ manifest: candidate, enabled: true }];
      return null;
    });
    const wrapper = mount(PluginsPanel); await flushPromises();
    await flushPromises();
    const entry = wrapper.find('.catalog-entry');
    expect(entry.text()).toContain('New Publisher');
    expect(entry.text()).toContain('查看更新');
    await entry.find('button').trigger('click'); await flushPromises();
    const approval = wrapper.get('.plugin-approval');
    expect(approval.text()).toContain('1.0 → 2.0');
    expect(approval.text()).toContain('Old Publisher → New Publisher');
    expect(approval.text()).toContain(`${'a'.repeat(16)}… → ${'b'.repeat(16)}…`);
    expect(approval.text()).toContain('https://old.example.invalid → https://new.example.invalid');
    expect(approval.text()).toContain('Community');
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'update_plugin')).toBe(false);
    await approval.get('.primary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('update_plugin', { id: manifest.id });
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_plugin')).toBe(false);
    wrapper.unmount();
  });
  it('offers review when catalog publisher metadata changes for the installed component', async () => {
    setLocale('zh-CN');
    const installed = { ...manifest, publisher: 'Old Publisher', component_sha256: 'a'.repeat(64) };
    const candidate = { ...manifest, publisher: 'New Publisher', component_sha256: 'a'.repeat(64) };
    const catalog = {
      source: { id: 'catalog-1', name: 'Community', url: 'https://catalog.example.invalid/index.json', enabled: true },
      snapshot: { source_id: 'catalog-1', source_url: 'https://catalog.example.invalid/index.json', retrieved_at: 1, index: { name: 'Community catalog', schema_version: 1, plugins: [{ manifest: candidate, description: 'A viewer', component_url: 'https://catalog.example.invalid/viewer.wasm' }] } },
    };
    vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ manifest: installed, enabled: true }] : command === 'list_bundled_plugins' ? [] : command === 'list_plugin_catalogs' ? [catalog] : command === 'preview_catalog_plugin' ? candidate : null);
    const wrapper = mount(PluginsPanel); await flushPromises();
    await flushPromises();
    const entry = wrapper.find('.catalog-entry');
    expect(entry.text()).toContain('Old Publisher → New Publisher');
    expect(entry.find('button').text()).toBe('查看更新');
    expect(entry.find('button').attributes('disabled')).toBeUndefined();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'preview_catalog_plugin')).toBe(false);
    await entry.find('button').trigger('click'); await flushPromises();
    expect(wrapper.get('.plugin-approval').text()).toContain('Old Publisher → New Publisher');
    wrapper.unmount();
  });
});


it('searches installed metadata without hiding the selected plugin actions', async () => {
  const candidate = { ...manifest, description: { en: 'Inspect pack contents', 'zh-CN': '检查整合包内容' }, tags: ['analysis', 'packs'] };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_plugins' ? [{ manifest: candidate, enabled: true }, { manifest: { ...manifest, id: 'other', name: 'Other' }, enabled: false }] : []);
  const wrapper = mount(PluginsPanel); await flushPromises();
  await wrapper.get('input[type="search"]').setValue('analysis pack');
  expect(wrapper.findAll('.plugin-list>li')).toHaveLength(1);
  expect(wrapper.get('.plugin-list').text()).toContain('analysis');
  expect(wrapper.get('.plugin-enable-toggle').attributes('disabled')).toBeUndefined();
  await wrapper.get('input[type="search"]').setValue('missing');
  expect(wrapper.findAll('.plugin-list>li')).toHaveLength(0);
  expect(wrapper.find('[role="status"]').exists()).toBe(true);
  wrapper.unmount();
});
