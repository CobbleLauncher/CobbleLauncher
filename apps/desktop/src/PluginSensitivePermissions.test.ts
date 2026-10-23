// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import PluginSensitivePermissions from './PluginSensitivePermissions.vue';
import PluginPermissions from './PluginPermissions.vue';
import type { CatalogManifest } from './plugin-catalogs';
import type { PermissionNode } from './plugin-permissions';
const manifest: CatalogManifest = { id: 'test', name: 'Test', publisher: 'Test', version: '1', api_version: '0.2.0', component_sha256: 'a'.repeat(64), capabilities: [], file_extensions: [] };
const nodes: PermissionNode[] = [
  { permission: 'launcher:provider/https/get', origins: ['https://one.example'] },
  { permission: 'launcher:resource/selected-folder/read', extensions: ['json'] },
];
it('keeps each concrete scope unchecked and labels it with its visible card content', async () => {
  const wrapper = mount(PluginSensitivePermissions, { props: { manifest, nodes, modelValue: [] } });
  const checks = wrapper.findAll<HTMLInputElement>('input');
  expect(checks.every(check => !check.element.checked)).toBe(true);
  for (const check of checks) {
    const id = check.attributes('aria-labelledby');
    expect(wrapper.find(`[id="${id}"]`).text().length).toBeGreaterThan(0);
  }
  await checks[0]!.setValue(true);
  expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([['launcher:provider/https/get:https://one.example']]);
  await wrapper.setProps({ disabled: true });
  expect(checks.every(check => check.element.disabled)).toBe(true);
});
it('groups related scopes without repeating category headings', () => {
  const wrapper = mount(PluginPermissions, { props: { manifest, nodes: [...nodes, { permission: 'launcher:resource/selected/read', extensions: ['log'] }] } });
  expect(wrapper.findAll('.permission-group')).toHaveLength(2);
  expect(wrapper.findAll('li')).toHaveLength(3);
  expect(wrapper.text()).toContain('https://one.example');
  expect(wrapper.text()).toContain('.json');
  expect(wrapper.text()).toContain('.log');
});
