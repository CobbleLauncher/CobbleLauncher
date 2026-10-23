// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import PluginSignatureStatus from './PluginSignatureStatus.vue';
import { setLocale } from './i18n';
import type { CatalogManifest } from './plugin-catalogs';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const manifest: CatalogManifest = { id: 'fixture', name: 'Fixture', version: '1', publisher: 'Example',
  api_version: '0.2.0', capabilities: ['report'], file_extensions: ['txt'], component_sha256: 'a'.repeat(64) };
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); });
afterEach(() => setLocale('zh-CN'));
it('shows native verified identity and blocks approval until verification finishes', async () => {
  vi.mocked(invoke).mockResolvedValue({ status: 'verified', rootId: 'abc123', publisher: 'Example' });
  const wrapper = mount(PluginSignatureStatus, { props: { manifest } });
  expect(wrapper.emitted('ready')).toEqual([[false]]);
  await flushPromises();
  expect(wrapper.text()).toContain('Signature verified · Example');
  expect(wrapper.get('.plugin-signature-status').attributes('title')).toBe('abc123');
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([true]);
  wrapper.unmount();
});
it('keeps a rejected signature blocked and permits an explicitly unsigned response only', async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error('invalid signature')).mockResolvedValueOnce({ status: 'unsigned' });
  const wrapper = mount(PluginSignatureStatus, { props: { manifest } });
  await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Could not verify');
  expect(wrapper.emitted('ready')).toEqual([[false]]);
  await wrapper.get('button').trigger('click');
  await flushPromises();
  expect(wrapper.text()).toContain('Unsigned package');
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([true]);
  wrapper.unmount();
});
it('ignores a stale verification when the selected package changes', async () => {
  let resolve!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(done => { resolve = done; })).mockResolvedValueOnce({ status: 'unsigned' });
  const wrapper = mount(PluginSignatureStatus, { props: { manifest } });
  await wrapper.setProps({ manifest: { ...manifest, version: '2' } });
  await flushPromises();
  resolve({ status: 'verified', rootId: 'old', publisher: 'Old signer' });
  await flushPromises();
  expect(wrapper.text()).toContain('Unsigned package');
  expect(wrapper.text()).not.toContain('Old signer');
  wrapper.unmount();
});
