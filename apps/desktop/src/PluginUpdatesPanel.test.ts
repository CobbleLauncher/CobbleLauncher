// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import PluginUpdatesPanel from './PluginUpdatesPanel.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const old = { id: 'test', name: 'Test', version: '1.0.0', publisher: 'Publisher', api_version: '1.0.0', capabilities: ['viewer'], file_extensions: ['schem'], component_sha256: 'a'.repeat(64) };
const next = { ...old, version: '1.1.0' };
afterEach(() => { vi.resetAllMocks(); setLocale('zh-CN'); });
it('does not install an unsigned batch candidate and preserves an individual review route', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_plugin_catalogs') return [];
    if (command === 'list_bundled_plugins') return [next];
    if (command === 'preview_bundled_plugin') return next;
    if (command === 'get_pending_plugin_signature') return { status: 'unsigned' };
    return undefined;
  });
  const wrapper = mount(PluginUpdatesPanel, { props: { installed: [old] } });
  await flushPromises();
  await wrapper.get('.updates-heading button').trigger('click');
  await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('update_plugin', expect.anything());
  expect(invoke).toHaveBeenCalledWith('cancel_plugin_preview');
  expect(wrapper.get('[role="alert"]').text()).toContain('Review this signature individually');
  await wrapper.get('button.update-review').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('prepared')?.[0]).toEqual([{ manifest: next, sourceName: 'CobbleLauncher', updateFrom: old }]);
  wrapper.unmount();
});
it('continues a signed batch after one transactional update fails', async () => {
  const secondOld = { ...old, id: 'second', name: 'Second' };
  const secondNext = { ...secondOld, version: '1.1.0' };
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    const id = (args as { id?: string } | undefined)?.id;
    if (command === 'list_plugin_catalogs') return [];
    if (command === 'list_bundled_plugins') return [next, secondNext];
    if (command === 'preview_bundled_plugin') return id === 'test' ? next : secondNext;
    if (command === 'get_pending_plugin_signature') return { status: 'verified', rootId: 'trusted', publisher: old.publisher };
    if (command === 'update_plugin' && id === 'test') throw new Error('disk full');
    return [];
  });
  const wrapper = mount(PluginUpdatesPanel, { props: { installed: [old, secondOld] } });
  await flushPromises();
  await wrapper.get('.updates-heading button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('update_plugin', { id: 'second', reviewedManifest: secondNext });
  expect(wrapper.get('[role="alert"]').text()).toContain('Test: Error: disk full');
  expect(wrapper.emitted('updated')).toHaveLength(1);
  expect(wrapper.props('installed')).toEqual([old, secondOld]);
  wrapper.unmount();
});
