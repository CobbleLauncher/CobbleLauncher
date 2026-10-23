// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import SelectField from './ui/SelectField.vue';
import MaterialExportButton from './MaterialExportButton.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => vi.clearAllMocks());

it('exports the open resource without sending materials or a path', async () => {
  vi.mocked(invoke).mockResolvedValue(true);
  const wrapper = mount(MaterialExportButton, { props: { resourceId: 'opaque-resource' } });
  await wrapper.get('[data-material-export]').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('export_blueprint_materials', { resourceId: 'opaque-resource', format: 'csv' });
  expect(wrapper.emitted('error')).toBeUndefined();
  wrapper.unmount();
});

it('saves text using the same opaque resource and prevents duplicate saves while busy', async () => {
  let complete!: (value: boolean) => void;
  vi.mocked(invoke).mockReturnValue(new Promise<boolean>((resolve) => { complete = resolve; }));
  const wrapper = mount(MaterialExportButton, { props: { resourceId: 'opaque-resource' } });
  await wrapper.findComponent(SelectField).vm.$emit('update:modelValue', 'text');
  await wrapper.get('[data-material-export]').trigger('click');
  expect(invoke).toHaveBeenCalledExactlyOnceWith('export_blueprint_materials', { resourceId: 'opaque-resource', format: 'text' });
  expect(wrapper.get('[role=combobox]').attributes('disabled')).toBeDefined();
  expect(wrapper.get('[data-material-export]').attributes('disabled')).toBeDefined();
  complete(false);
  await flushPromises();
  expect(wrapper.get('[role=combobox]').attributes('disabled')).toBeUndefined();
  wrapper.unmount();
});

it('treats save dialog cancellation as a normal result', async () => {
  vi.mocked(invoke).mockResolvedValue(false);
  const wrapper = mount(MaterialExportButton, { props: { resourceId: 'opaque-resource' } });
  await wrapper.get('[data-material-export]').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('error')).toBeUndefined();
  expect(wrapper.get('[data-material-export]').attributes('disabled')).toBeUndefined();
  wrapper.unmount();
});

it('reports write failures and allows retrying', async () => {
  vi.mocked(invoke).mockRejectedValue('Disk full');
  const wrapper = mount(MaterialExportButton, { props: { resourceId: 'opaque-resource' } });
  await wrapper.get('[data-material-export]').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('error')).toEqual([['Disk full']]);
  expect(wrapper.get('[data-material-export]').attributes('disabled')).toBeUndefined();
  wrapper.unmount();
});
