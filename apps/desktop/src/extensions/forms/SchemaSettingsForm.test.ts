// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import Ajv from 'ajv';
import { execFileSync } from 'node:child_process';
import SchemaSettingsForm from './SchemaSettingsForm.vue';
import ExtensionSettingsPanel from '../../ExtensionSettingsPanel.vue';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const schema = { type: 'object', additionalProperties: false, properties: {
  format: { type: 'string', title: 'Text file format', enum: ['auto', 'json', 'yaml'], default: 'auto' },
  warnings: { type: 'boolean', title: 'Warnings', default: true },
}, required: ['format', 'warnings'] };
const snapshot = { pluginId: 'doctor', values: { format: 'auto', warnings: true }, categories: [], form: { schema_version: 1, scope: 'global', schema } };
afterEach(() => { vi.restoreAllMocks(); vi.mocked(invoke).mockReset(); });
it('renders standard auto layout and enum controls without runtime Function compilation', async () => {
  const compile = vi.spyOn(Ajv.prototype, 'compile').mockImplementation(() => { throw new Error('CSP blocked validation compilation'); });
  execFileSync(process.execPath, ['--disallow-code-generation-from-strings', '--input-type=module', '-e', `import {coreReducer,Actions} from '@jsonforms/core'; const schema=${JSON.stringify(schema)}; coreReducer(undefined,Actions.init({format:'auto',warnings:true},schema,undefined,{validationMode:'NoValidation'}));`]);
  const wrapper = mount(SchemaSettingsForm, { props: { schema, data: snapshot.values, translations: { 'format.auto': 'From file type' } } });
  await flushPromises();
  expect(wrapper.text()).toContain('From file type');
  await wrapper.find('select').setValue('"json"');
  await flushPromises();
  expect(wrapper.emitted('change')?.at(-1)?.[0]).toEqual({ format: 'json', warnings: true });
  expect(compile).not.toHaveBeenCalled();
  wrapper.unmount();
});
it('saves a complete draft through native validation and shows field errors without losing the draft', async () => {
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'get_extension_settings' ? snapshot : {
    snapshot: null, errors: [{ instancePath: '/format', schemaPath: '#/properties/format/enum', keyword: 'enum', params: {}, message: 'Choose an allowed format' }],
  }));
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId: 'doctor' } });
  await flushPromises();
  await wrapper.find('select').setValue('"yaml"');
  expect(wrapper.vm.canLeave()).toBe(false);
  const apply = wrapper.findAll('button').find(button => /Apply|应用/.test(button.text()))!;
  await apply.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_extension_settings_form', { pluginId: 'doctor', values: { format: 'yaml', warnings: true }, locale: 'zh-CN' });
  expect(wrapper.text()).toContain('请选择允许的选项。');
  const { setLocale } = await import('../../i18n');
  setLocale('en-US');
  await flushPromises();
  expect(wrapper.text()).toContain('Choose an allowed option.');
  expect(wrapper.text()).not.toContain('请选择允许的选项。');
  setLocale('zh-CN');
  await flushPromises();
  expect(wrapper.vm.canLeave()).toBe(false);
  expect(wrapper.find('select').element.value).toBe('"yaml"');
  wrapper.unmount();
});
it('keeps the form draft across locale reload and saves all values together', async () => {
  const { setLocale } = await import('../../i18n');
  vi.mocked(invoke).mockImplementation((command, args) => Promise.resolve(command === 'get_extension_settings' ? snapshot : { snapshot: { ...snapshot, values: (args as { values: object }).values }, errors: [] }));
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId: 'doctor' } });
  await flushPromises();
  await wrapper.find('select').setValue('"json"');
  await wrapper.find('input[type="checkbox"]').setValue(false);
  setLocale('en-US');
  await flushPromises();
  expect(wrapper.find('select').element.value).toBe('"json"');
  expect(wrapper.find<HTMLInputElement>('input[type="checkbox"]').element.checked).toBe(false);
  await wrapper.find('.settings-actions .secondary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_extension_settings_form', { pluginId: 'doctor', locale: 'en', values: { format: 'json', warnings: false } });
  expect(wrapper.vm.canLeave()).toBe(true);
  expect(wrapper.emitted('busy')?.some(([busy]) => busy === true)).toBe(true);
  wrapper.unmount();
  setLocale('zh-CN');
});
it('merges a native YAML preview into the full form before explicit saving', async () => {
  vi.mocked(invoke).mockImplementation((command, args) => Promise.resolve(command === 'get_extension_settings' ? snapshot
    : command === 'preview_extension_settings_yaml' ? { values: { format: 'yaml' } }
    : { snapshot: { ...snapshot, values: (args as { values: object }).values }, errors: [] }));
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId: 'doctor' } });
  await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === '导入 YAML')!.trigger('click');
  const textarea = document.body.querySelector<HTMLTextAreaElement>('.settings-yaml-dialog textarea')!;
  textarea.value = 'format: yaml';
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
  await flushPromises();
  [...document.body.querySelectorAll<HTMLButtonElement>('.settings-yaml-dialog button')].find(button => button.textContent?.includes('预览设置'))!.click();
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'save_extension_settings_form')).toBe(false);
  await wrapper.find('.settings-actions .secondary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_extension_settings_form', { pluginId: 'doctor', locale: 'zh-CN', values: { format: 'yaml', warnings: true } });
  wrapper.unmount();
});
it('uses standard Group layout labels and readonly controls with the shared settings components', async () => {
  const wrapper = mount(SchemaSettingsForm, { props: {
    schema, data: snapshot.values, disabled: true,
    translations: { 'Format options': '格式选项', 'format.label': '文件格式' },
    uiSchema: { type: 'Group', label: 'Format options', elements: [
      { type: 'Control', scope: '#/properties/format' }, { type: 'Control', scope: '#/properties/warnings' },
    ] },
  } });
  await flushPromises();
  expect(wrapper.text()).toContain('格式选项');
  expect(wrapper.text()).toContain('文件格式');
  expect(wrapper.find('select').attributes('disabled')).toBeDefined();
  expect(wrapper.find('input[type="checkbox"]').attributes('disabled')).toBeDefined();
  expect(wrapper.findAll('.settings-row')).toHaveLength(2);
  wrapper.unmount();
});
