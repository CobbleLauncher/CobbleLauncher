// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import { nextTick } from 'vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import type { CatalogManifest } from './plugin-catalogs';
import { setLocale } from './i18n';

const manifest = (permissions: NonNullable<CatalogManifest['permissions']>): CatalogManifest => ({
  id: 'fixture', name: 'Fixture', version: '1', api_version: '0.2.0', publisher: 'Fixture',
  component_sha256: 'a'.repeat(64), capabilities: [], file_extensions: [], permissions,
});

it('shows distinct badges only for network, local-file and write permissions', () => {
  setLocale('en-US');
  const wrapper = mount(PluginPermissionBadges, { props: { manifest: manifest([
    { permission: 'launcher:provider/https/get', origins: ['https://example.test'] },
    { permission: 'launcher:resource/selected/read', extensions: ['log'] },
    { permission: 'launcher:images/output/propose' },
    { permission: 'launcher:ui/webview' },
  ]) } });
  expect(wrapper.findAll('.plugin-permission-badge').map(item => item.text())).toEqual(['Network', 'Local files', 'Can write']);
  expect(wrapper.find('.plugin-permission-badge').attributes('title')).toContain('https://example.test');
  expect(wrapper.text()).not.toContain('enabled');
  wrapper.unmount();
});

it('groups game diagnostics read access with local files and omits badges for UI-only plugins', async () => {
  setLocale('en-US');
  const wrapper = mount(PluginPermissionBadges, { props: { manifest: manifest([
    { permission: 'launcher:game-diagnostics/read' } as never,
  ]) } });
  expect(wrapper.text()).toContain('Local files');
  await wrapper.setProps({ manifest: manifest([{ permission: 'launcher:ui/webview' }]) });
  await nextTick();
  expect(wrapper.find('.plugin-permission-badge').exists()).toBe(false);
  wrapper.unmount();
});

it('shows a warning badge with the exact saved-world seed scope in English and Chinese', async () => {
  const wrapper = mount(PluginPermissionBadges, { props: { manifest: manifest([
    { permission: 'launcher:worlds/seed/read' },
  ]) } });
  expect(wrapper.findAll('.plugin-permission-badge').map(item => item.text())).toEqual(['World seed']);
  expect(wrapper.find('.plugin-permission-badge').attributes('title')).toContain('Read the selected world’s seed and generation details');
  expect(wrapper.find('.plugin-permission-badge').attributes('title')).not.toContain('all files');
  setLocale('zh-CN');
  await nextTick();
  expect(wrapper.text()).toContain('世界种子');
  expect(wrapper.find('.plugin-permission-badge').attributes('title')).toContain('读取所选存档的种子和生成信息');
  wrapper.unmount();
  setLocale('en-US');
});
