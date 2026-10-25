// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { Ajv2020 } from 'ajv/dist/2020';
import { mount } from '@vue/test-utils';
import schema from '../../../contracts/schemas/viewer-manifest.schema.json';
import { isSupportedCatalogPlugin, type CatalogManifest } from './plugin-catalogs';
import { addedPermissionNodes, type PermissionNode } from './plugin-permissions';
import { requiresSensitiveConsent } from './extensions/permission-review';
import PluginPermissions from './PluginPermissions.vue';
import { setLocale } from './i18n';
const validate = new Ajv2020({ strict: false }).compile(schema);
const frontend = { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'b'.repeat(64) } as const;
function manifest(permissions: PermissionNode[], extensions: string[] = []): CatalogManifest {
  return { id: 'maintenance-fixture', name: 'Maintenance', publisher: 'Fixture', version: '0.1.0', api_version: '0.2.0', capabilities: ['report'], file_extensions: extensions, component_sha256: 'a'.repeat(64), frontend, permissions };
}
const ui: PermissionNode = { permission: 'launcher:ui/webview' };
const workflows: PermissionNode['permission'][] = ['launcher:game-config/write', 'launcher:storage/cache/clear', 'launcher:instances/move-request', 'launcher:instances/copy-request', 'launcher:game-recovery/read', 'launcher:game-recovery/open-request', 'launcher:play-manifest/export', 'launcher:files/import-request'];
afterEach(() => setLocale('zh-CN'));
it('keeps host-only sharing and file handoff independent of fake file reads and private storage', () => {
  for (const permission of ['launcher:play-manifest/export', 'launcher:files/import-request'] as const) {
    const current = manifest([ui, { permission }]);
    expect(validate(current), JSON.stringify(validate.errors)).toBe(true);
    expect(isSupportedCatalogPlugin({ manifest: current, description: '' })).toBe(true);
    expect(validate({ ...current, frontend: undefined })).toBe(false);
    expect(validate({ ...current, permissions: [ui, { permission, path: '/private' }] })).toBe(false);
    const previous = manifest([ui, { permission: 'launcher:storage/private' }]);
    expect(addedPermissionNodes(current, previous)).toEqual([{ permission }]);
  }
});
it('requires exact existing storage scope for cache, move and copy actions', () => {
  for (const permission of ['launcher:storage/cache/clear', 'launcher:instances/move-request', 'launcher:instances/copy-request'] as const) {
    const current = manifest([ui, { permission: 'launcher:game-inspection/read', scopes: ['storage'] }, { permission }]);
    expect(validate(current), JSON.stringify(validate.errors)).toBe(true);
    expect(isSupportedCatalogPlugin({ manifest: current, description: '' })).toBe(true);
    const missing = manifest([ui, { permission }]);
    expect(validate(missing)).toBe(false); expect(isSupportedCatalogPlugin({ manifest: missing, description: '' })).toBe(false);
    expect(requiresSensitiveConsent({ permission })).toBe(true);
  }
});
it('requires recovery reads before requesting host recovery and does not grant automatic changes', () => {
  const current = manifest([ui, { permission: 'launcher:resource/selected/read', extensions: ['log'] }, { permission: 'launcher:game-recovery/read' }, { permission: 'launcher:game-recovery/open-request' }], ['log']);
  expect(validate(current)).toBe(true); expect(isSupportedCatalogPlugin({ manifest: current, description: '' })).toBe(true);
  current.permissions = current.permissions!.filter(node => node.permission !== 'launcher:game-recovery/read');
  expect(validate(current)).toBe(false); expect(isSupportedCatalogPlugin({ manifest: current, description: '' })).toBe(false);
  expect(requiresSensitiveConsent({ permission: 'launcher:game-recovery/open-request' })).toBe(false);
  expect(requiresSensitiveConsent({ permission: 'launcher:files/import-request' })).toBe(false);
});
it('renders every closed workflow with specific English and Chinese labels', async () => {
  const current = manifest(workflows.map(permission => ({ permission }) as PermissionNode), ['json']);
  setLocale('en-US'); const wrapper = mount(PluginPermissions, { props: { manifest: current } });
  expect(wrapper.text()).toContain('Edit and restore'); expect(wrapper.text()).toContain('separate copy');
  expect(wrapper.text()).not.toContain('launcher:'); expect(wrapper.text()).not.toContain('undefined');
  setLocale('zh-CN'); await wrapper.vm.$nextTick(); expect(wrapper.text()).toContain('独立副本'); expect(wrapper.text()).toContain('导入流程'); wrapper.unmount();
});
