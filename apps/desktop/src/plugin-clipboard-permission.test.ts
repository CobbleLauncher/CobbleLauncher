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
const node: PermissionNode = { permission: 'launcher:clipboard/write' };
const validate = new Ajv2020().compile(schema);
const fixture = (): CatalogManifest => ({
  id: 'clipboard-fixture', name: 'Copy text', publisher: 'Fixture', version: '0.1.0', api_version: '0.2.0',
  capabilities: ['report'], file_extensions: ['json'], component_sha256: 'a'.repeat(64),
  frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'b'.repeat(64) },
  permissions: [{ permission: 'launcher:ui/webview' }, { permission: 'launcher:resource/selected/read', extensions: ['json'] }],
});
afterEach(() => setLocale('zh-CN'));
it('supports only explicitly declared parameterless clipboard writes from a frontend', () => {
  const previous = fixture(), manifest = { ...previous, permissions: [...previous.permissions!, node] };
  expect(validate(manifest)).toBe(true); expect(isSupportedCatalogPlugin({ manifest, description: '' })).toBe(true);
  expect(effectivePermissionNodes(previous)).not.toContainEqual(node);
  expect(addedPermissionNodes(manifest, previous)).toEqual([node]); expect(addedPermissionNodes(previous, manifest)).toEqual([]);
  expect(validate({ ...manifest, frontend: undefined })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, frontend: undefined }, description: '' })).toBe(false);
  for (const invalid of [
    { permission: 'launcher:clipboard/read' },
    { ...node, read: true }, { ...node, scopes: ['text'] }, { ...node, path: '/private' },
  ]) expect(validate({ ...manifest, permissions: [...previous.permissions!, invalid] })).toBe(false);
});
it('renders clipboard access as neutral localized text without a filesystem or network risk badge', async () => {
  expect(permissionCategory(node)).toBe('clipboard'); expect(permissionTone(node)).toBe('neutral'); expect(requiresSensitiveConsent(node)).toBe(false);
  const manifest = { ...fixture(), permissions: [node] };
  setLocale('en-US'); const wrapper = mount(PluginPermissions, { props: { manifest } });
  expect(wrapper.text()).toBe('Copy text to clipboard'); expect(wrapper.get('.permission-group').attributes('data-tone')).toBe('neutral');
  setLocale('zh-CN'); await wrapper.vm.$nextTick(); expect(wrapper.text()).toBe('复制文本到剪贴板'); wrapper.unmount();
  const badges = mount(PluginPermissionBadges, { props: { manifest } }); expect(badges.text()).toBe(''); badges.unmount();
});
