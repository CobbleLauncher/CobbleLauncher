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

const privateNode: PermissionNode = { permission: 'launcher:storage/private' };
const validate = new Ajv2020().compile(schema);
const fixture = (): CatalogManifest => ({
  id: 'private-data', name: 'Private data', publisher: 'Fixture', version: '0.1.0',
  api_version: '0.2.0', capabilities: ['report'], file_extensions: [], component_sha256: 'a'.repeat(64),
  frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'b'.repeat(64) },
  permissions: [{ permission: 'launcher:ui/webview' }, privateNode],
});
afterEach(() => setLocale('zh-CN'));
it('accepts an explicit private-data frontend report without granting selected files', () => {
  const manifest = fixture();
  expect(validate(manifest)).toBe(true);
  expect(isSupportedCatalogPlugin({ manifest, description: '' })).toBe(true);
  expect(effectivePermissionNodes(manifest)).toEqual(manifest.permissions);
  expect(effectivePermissionNodes(manifest).some(node => 'extensions' in node)).toBe(false);
  expect(validate({ ...manifest, api_version: '0.1.0' })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, api_version: '0.1.0' }, description: '' })).toBe(false);
});
it('rejects undeclared storage, missing frontends and caller-controlled scope parameters', () => {
  const manifest = fixture();
  const undeclared = { ...manifest, permissions: [{ permission: 'launcher:ui/webview' } as PermissionNode] };
  expect(validate(undeclared)).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: undeclared, description: '' })).toBe(false);
  expect(validate({ ...manifest, frontend: undefined })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...manifest, frontend: undefined }, description: '' })).toBe(false);
  for (const extra of ['path', 'scopes', 'keys', 'plugin_id']) {
    const invalid = { ...manifest, permissions: [manifest.permissions![0]!, { ...privateNode, [extra]: 'arbitrary' }] };
    expect(validate(invalid)).toBe(false);
    expect(isSupportedCatalogPlugin({ manifest: invalid, description: '' })).toBe(false);
  }
});
it('allows storage alongside inspection and existing file-based frontend reports', () => {
  for (const manifest of [
    { ...fixture(), permissions: [...fixture().permissions!, { permission: 'launcher:game-inspection/read' as const, scopes: ['worlds' as const] }] },
    { ...fixture(), file_extensions: ['log'], permissions: [...fixture().permissions!, { permission: 'launcher:resource/selected/read' as const, extensions: ['log'] }] },
    { ...fixture(), api_version: '0.1.0', capabilities: ['image-layout'], permissions: [...fixture().permissions!, { permission: 'launcher:images/selected/read' as const }, { permission: 'launcher:images/output/propose' as const }] },
  ]) {
    expect(validate(manifest)).toBe(true);
    expect(isSupportedCatalogPlugin({ manifest, description: '' })).toBe(true);
  }
});
it('reviews additions without treating private JSON as sensitive filesystem access', () => {
  const manifest = fixture(), previous = { ...manifest, permissions: [manifest.permissions![0]!] };
  expect(addedPermissionNodes(manifest, previous)).toEqual([privateNode]);
  expect(addedPermissionNodes(previous, manifest)).toEqual([]);
  expect(permissionCategory(privateNode)).toBe('storage');
  expect(permissionTone(privateNode)).toBe('neutral');
  expect(requiresSensitiveConsent(privateNode)).toBe(false);
  const badges = mount(PluginPermissionBadges, { props: { manifest } });
  expect(badges.text()).toBe(''); badges.unmount();
});
it('uses concise neutral labels in both locales', async () => {
  setLocale('en-US');
  const wrapper = mount(PluginPermissions, { props: { manifest: fixture(), nodes: [privateNode] } });
  expect(wrapper.text()).toBe('Private data'); expect(wrapper.get('.permission-group').attributes('data-tone')).toBe('neutral');
  setLocale('zh-CN'); await wrapper.vm.$nextTick(); expect(wrapper.text()).toBe('插件数据'); wrapper.unmount();
});
