// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import { Ajv2020 } from 'ajv/dist/2020';
import schema from '../../../contracts/schemas/viewer-manifest.schema.json';
import { isSupportedCatalogPlugin, type CatalogManifest } from './plugin-catalogs';
import { addedPermissionNodes, effectivePermissionNodes, type PermissionNode } from './plugin-permissions';
import { permissionCategory, permissionTone, requiresSensitiveConsent } from './extensions/permission-review';
import PluginPermissions from './PluginPermissions.vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import { setLocale } from './i18n';
const node: PermissionNode = { permission: 'launcher:exports/text' };
const validate = new Ajv2020().compile(schema);
const fixture = (): CatalogManifest => ({
  id: 'text-export-fixture', name: 'Save text', publisher: 'Fixture', version: '0.1.0', api_version: '0.2.0',
  capabilities: ['report'], file_extensions: ['json'], component_sha256: 'a'.repeat(64),
  frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'b'.repeat(64) },
  permissions: [{ permission: 'launcher:ui/webview' }, { permission: 'launcher:resource/selected/read', extensions: ['json'] }],
});
afterEach(() => setLocale('zh-CN'));
it('adds only explicitly declared parameterless text export and requires normal upgrade review', () => {
  const previous = fixture(), manifest = { ...previous, permissions: [...previous.permissions!, node] };
  expect(validate(manifest)).toBe(true); expect(isSupportedCatalogPlugin({ manifest, description: '' })).toBe(true);
  expect(effectivePermissionNodes(previous)).not.toContainEqual(node);
  expect(addedPermissionNodes(manifest, previous)).toEqual([node]); expect(addedPermissionNodes(previous, manifest)).toEqual([]);
  expect(validate({ ...manifest, frontend: undefined })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, frontend: undefined }, description: '' })).toBe(false);
  for (const invalid of [
    { permission: 'launcher:exports/file' }, { ...node, read: true }, { ...node, extensions: ['txt'] },
    { ...node, scopes: ['text'] }, { ...node, path: '/private' },
  ]) expect(validate({ ...manifest, permissions: [...previous.permissions!, invalid] })).toBe(false);
  for (const invalid of [{ ...node, path: '/private' }, node]) {
    expect(isSupportedCatalogPlugin({ manifest: { ...manifest, permissions: [...manifest.permissions, invalid] }, description: '' })).toBe(false);
  }
});
it('supports independent export alongside all existing image frontend permissions', () => {
  const manifest: CatalogManifest = { ...fixture(), api_version: '0.1.0', capabilities: ['image-layout'], file_extensions: [], permissions: [
    { permission: 'launcher:ui/webview' }, { permission: 'launcher:images/selected/read' }, { permission: 'launcher:images/output/propose' },
    { permission: 'launcher:storage/private' }, { permission: 'launcher:clipboard/write' }, node,
  ] };
  expect(validate(manifest)).toBe(true); expect(isSupportedCatalogPlugin({ manifest, description: '' })).toBe(true);
});
it('renders a minimal localized save label and icon without additional sensitive consent or risk badges', async () => {
  expect(permissionCategory(node)).toBe('exports'); expect(permissionTone(node)).toBe('neutral'); expect(requiresSensitiveConsent(node)).toBe(false);
  const manifest = { ...fixture(), permissions: [node] };
  setLocale('en-US'); const wrapper = mount(PluginPermissions, { props: { manifest } });
  expect(wrapper.text()).toBe('Save text through a file dialog'); expect(wrapper.get('.permission-group').attributes('data-tone')).toBe('neutral');
  expect(wrapper.find('.permission-group-icon svg').exists()).toBe(true);
  setLocale('zh-CN'); await wrapper.vm.$nextTick(); expect(wrapper.text()).toBe('通过文件对话框保存文本'); wrapper.unmount();
  const badges = mount(PluginPermissionBadges, { props: { manifest } }); expect(badges.text()).toBe(''); badges.unmount();
});
