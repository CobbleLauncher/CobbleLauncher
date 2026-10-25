// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { Ajv2020 } from 'ajv/dist/2020';
import { mount } from '@vue/test-utils';
import schema from '../../../contracts/schemas/viewer-manifest.schema.json';
import { type CatalogManifest, isSupportedCatalogPlugin } from './plugin-catalogs';
import { addedPermissionNodes, effectivePermissionNodes, type PermissionNode } from './plugin-permissions';
import { permissionCategory, permissionConsentKey, permissionTone, requiresSensitiveConsent } from './extensions/permission-review';
import PluginPermissions from './PluginPermissions.vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import { setLocale } from './i18n';

const validate = new Ajv2020({ strict: false }).compile(schema);
const send: PermissionNode = { permission: 'launcher:mesh/events/send', namespaces: ['org.example.coordinates'] };
function manifest(): CatalogManifest {
  return { id: 'mesh-fixture', name: 'Mesh fixture', publisher: 'Fixture', version: '0.1.0', api_version: '0.2.0', capabilities: ['report'], file_extensions: [], component_sha256: 'a'.repeat(64),
    frontend: { api_version: '0.1.0', entry: 'ui/index.html', assets_sha256: 'b'.repeat(64) }, permissions: [{ permission: 'launcher:ui/webview' }, send] };
}
afterEach(() => setLocale('zh-CN'));

it('accepts only explicit, bounded namespace grants for a frontend', () => {
  const current = manifest();
  expect(validate(current), JSON.stringify(validate.errors)).toBe(true);
  expect(isSupportedCatalogPlugin({ manifest: current, description: '' })).toBe(true);
  expect(validate({ ...current, frontend: undefined })).toBe(false);
  expect(isSupportedCatalogPlugin({ manifest: { ...current, frontend: undefined }, description: '' })).toBe(false);
  for (const namespace of ['*', 'org.*', 'cobble.events', 'org', 'Org.example', 'org.example-', 'org.1example']) {
    const invalid = { ...current, permissions: [{ ...send, namespaces: [namespace] }] };
    expect(validate(invalid)).toBe(false);
    expect(isSupportedCatalogPlugin({ manifest: invalid, description: '' })).toBe(false);
  }
});

it('reviews namespace expansion and receive access independently', () => {
  const before = manifest(), after = manifest();
  after.permissions = [{ permission: 'launcher:ui/webview' }, { ...send, namespaces: ['org.example.coordinates', 'org.example.notes'] }];
  expect(addedPermissionNodes(after, before)).toEqual([{ ...send, namespaces: ['org.example.notes'] }]);
  const receive: PermissionNode = { permission: 'launcher:mesh/events/receive', namespaces: ['org.example.coordinates'] };
  after.permissions = [...before.permissions!, receive];
  expect(addedPermissionNodes(after, before)).toEqual([receive]);
  expect(permissionConsentKey(send)).not.toBe(permissionConsentKey(receive));
  expect(permissionConsentKey({ ...send, namespaces: ['org.b', 'org.a'] })).toBe(permissionConsentKey({ ...send, namespaces: ['org.a', 'org.b'] }));
  expect(effectivePermissionNodes({ ...before, frontend: undefined, permissions: undefined, capabilities: ['message-composer'], composer_permissions: ['draft', 'public_identity'] }).some(node => 'namespaces' in node)).toBe(false);
});

it('shows direction and exact namespace in both languages and a network badge', async () => {
  const current = manifest(); current.permissions!.push({ permission: 'launcher:mesh/events/receive', namespaces: ['org.example.notes'] });
  for (const node of current.permissions!.filter(node => 'namespaces' in node)) {
    expect(permissionCategory(node)).toBe('network'); expect(permissionTone(node)).toBe('network'); expect(requiresSensitiveConsent(node)).toBe(true);
  }
  setLocale('en-US');
  const wrapper = mount(PluginPermissions, { props: { manifest: current } });
  const badges = mount(PluginPermissionBadges, { props: { manifest: current } });
  expect(wrapper.text()).toContain('Send data to Mesh contacts: org.example.coordinates'); expect(wrapper.text()).toContain('Receive data from Mesh contacts: org.example.notes');
  expect(badges.text()).toContain('Network'); expect(badges.html()).toContain('org.example.coordinates');
  setLocale('zh-CN'); await wrapper.vm.$nextTick();
  expect(wrapper.text()).toContain('向 Mesh 联系人发送数据'); expect(wrapper.text()).toContain('接收 Mesh 联系人的数据'); expect(wrapper.text()).not.toContain('launcher:');
  wrapper.unmount(); badges.unmount();
});
