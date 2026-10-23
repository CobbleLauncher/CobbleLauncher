// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import { setLocale } from './i18n';
import type { ExtensionJsonValue, ExtensionSettingsSnapshot } from './extensions/settings-types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));

const pluginId = 'com.example.settings';
const control = (id: string) => `[id="extension-${pluginId}-${id}"]`;
const fields = [
  { id: 'show_hints', type: 'boolean' as const, label: '显示提示', scope: 'global' as const, advanced: false, default: true, options: [] },
  { id: 'quality', type: 'enum' as const, label: 'Quality', scope: 'global' as const, advanced: false, default: 'low', options: [{ value: 'low', label: 'Low' }, { value: 'high', label: 'High' }] },
  { id: 'display_name', type: 'string' as const, label: 'Display name', scope: 'global' as const, advanced: false, default: 'Default', maxLength: 24, options: [] },
  { id: 'worker_count', type: 'number' as const, label: 'Worker count', scope: 'instance' as const, advanced: false, default: 3, minimum: 1, maximum: 8, options: [] },
  { id: 'diagnostics', type: 'boolean' as const, label: 'Diagnostics', scope: 'global' as const, advanced: true, default: false, options: [] },
];

function makeSnapshot(instanceId?: string, translated = false): ExtensionSettingsSnapshot {
  return {
    pluginId,
    ...(instanceId ? { instanceId } : {}),
    values: { show_hints: true, quality: 'low', display_name: 'Default', worker_count: 3, diagnostics: false },
    categories: [{
      id: 'general',
      label: translated ? 'General' : '常规',
      fields: fields.map(field => ({ ...field, label: translated ? field.id : field.label, options: field.options.map(option => ({ ...option, label: translated ? option.label : `选项 ${option.value}` })) })),
    }],
  };
}

beforeEach(() => { setLocale('zh-CN'); vi.mocked(invoke).mockReset(); });
afterEach(() => { setLocale('zh-CN'); document.body.innerHTML = ''; });

it('loads native-localized fields and hides instance fields without an instance context', async () => {
  vi.mocked(invoke).mockResolvedValue(makeSnapshot());
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId } });
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('get_extension_settings', { pluginId, locale: 'zh-CN' });
  expect(wrapper.text()).toContain('常规');
  expect(wrapper.find(control('show_hints')).exists()).toBe(true);
  expect(wrapper.find(control('worker_count')).exists()).toBe(false);
  expect(wrapper.get('.settings-disclosure-trigger').attributes('aria-expanded')).toBe('false');
  await wrapper.get('.settings-disclosure-trigger').trigger('click');
  expect(wrapper.get('.settings-disclosure-trigger').attributes('aria-expanded')).toBe('true');
  wrapper.unmount();
});

it('persists boolean and enum controls immediately using their declared scope', async () => {
  const current = makeSnapshot();
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_extension_settings') return makeSnapshot('game-1');
    const commandArgs = args as Record<string, unknown>;
    const fieldId = commandArgs.fieldId as string;
    const value = commandArgs.value as ExtensionJsonValue;
    return { ...current, values: { ...current.values, [fieldId]: value } };
  });
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId, instanceId: 'game-1', categoryId: 'general' } });
  await flushPromises();

  await wrapper.get(control('show_hints')).setValue(false);
  expect(invoke).toHaveBeenCalledWith('set_extension_setting', { pluginId, fieldId: 'show_hints', value: false, locale: 'zh-CN' });
  await flushPromises();
  await wrapper.get(control('quality')).setValue('"high"');
  expect(invoke).toHaveBeenCalledWith('set_extension_setting', { pluginId, fieldId: 'quality', value: 'high', locale: 'zh-CN' });
  wrapper.unmount();
});

it('groups string and number edits until apply and sends instance scope only for instance fields', async () => {
  let current = makeSnapshot('game-1');
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_extension_settings') return current;
    const commandArgs = args as Record<string, unknown>;
    const fieldId = commandArgs.fieldId as string;
    const value = commandArgs.value as ExtensionJsonValue;
    current = { ...current, values: { ...current.values, [fieldId]: value } };
    return current;
  });
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId, instanceId: 'game-1', categoryId: 'general' } });
  await flushPromises();
  await wrapper.get(control('display_name')).setValue('Updated');
  await wrapper.get(control('worker_count')).setValue(5);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_extension_setting')).toHaveLength(0);

  await wrapper.get('button.secondary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenNthCalledWith(2, 'set_extension_setting', { pluginId, fieldId: 'display_name', value: 'Updated', locale: 'zh-CN' });
  expect(invoke).toHaveBeenNthCalledWith(3, 'set_extension_setting', { pluginId, fieldId: 'worker_count', value: 5, locale: 'zh-CN', instanceId: 'game-1' });
  expect(wrapper.find('.settings-actions').exists()).toBe(false);
  wrapper.unmount();
});

it('cancels drafts and exposes a leave guard for dirty settings', async () => {
  vi.mocked(invoke).mockResolvedValue(makeSnapshot());
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId } });
  await flushPromises();
  await wrapper.get(control('display_name')).setValue('Unsaved');
  const api = wrapper.vm as unknown as { canLeave: () => boolean; confirmLeave: () => Promise<boolean> };
  expect(api.canLeave()).toBe(false);
  const decision = api.confirmLeave();
  await flushPromises();
  const discard = [...document.body.querySelectorAll<HTMLButtonElement>('.settings-leave-dialog button')].find(button => button.textContent?.includes('放弃'));
  expect(discard).toBeDefined();
  discard?.click();
  await flushPromises();
  expect(await decision).toBe(true);
  expect(api.canLeave()).toBe(true);
  expect((wrapper.get(control('display_name')).element as HTMLInputElement).value).toBe('Default');
  wrapper.unmount();
});

it('reloads localized labels and ignores an older plugin response', async () => {
  let finishFirst!: (snapshot: ExtensionSettingsSnapshot) => void;
  vi.mocked(invoke).mockImplementation((_command, args) => {
    const commandArgs = args as Record<string, unknown>;
    if (commandArgs.locale === 'zh-CN') return new Promise(resolve => { finishFirst = resolve; });
    return Promise.resolve(makeSnapshot(undefined, true));
  });
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId } });
  setLocale('en-US');
  await flushPromises();
  finishFirst(makeSnapshot());
  await flushPromises();

  expect(invoke).toHaveBeenNthCalledWith(1, 'get_extension_settings', { pluginId, locale: 'zh-CN' });
  expect(invoke).toHaveBeenNthCalledWith(2, 'get_extension_settings', { pluginId, locale: 'en' });
  expect(wrapper.text()).toContain('display_name');
  expect(wrapper.text()).not.toContain('常规');
  wrapper.unmount();
});


it('finishes both draft writes before reloading labels after a language change', async () => {
  let current = makeSnapshot('game-1');
  let finishFirst!: () => void;
  let writes = 0;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    const values = args as Record<string, unknown>;
    if (command === 'get_extension_settings') return { ...current, categories: makeSnapshot('game-1', values.locale === 'en').categories };
    if (++writes === 1) await new Promise<void>(resolve => { finishFirst = resolve; });
    current = { ...current, values: { ...current.values, [values.fieldId as string]: values.value as ExtensionJsonValue } };
    return current;
  });
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId, instanceId: 'game-1', categoryId: 'general' } });
  await flushPromises();
  await wrapper.get(control('display_name')).setValue('Updated');
  await wrapper.get(control('worker_count')).setValue(5);
  await wrapper.get('button.secondary-button').trigger('click');
  setLocale('en-US');
  await flushPromises();
  const api = wrapper.vm as unknown as { canLeave: () => boolean; confirmLeave: () => Promise<boolean> };
  expect(api.canLeave()).toBe(false);
  expect(await api.confirmLeave()).toBe(false);
  expect(wrapper.get(control('display_name')).attributes('disabled')).toBeDefined();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'get_extension_settings')).toHaveLength(1);
  finishFirst();
  await flushPromises();
  expect(current.values.display_name).toBe('Updated');
  expect(current.values.worker_count).toBe(5);
  expect(invoke).toHaveBeenCalledWith('set_extension_setting', { pluginId, fieldId: 'worker_count', value: 5, locale: 'zh-CN', instanceId: 'game-1' });
  expect(invoke).toHaveBeenLastCalledWith('get_extension_settings', { pluginId, locale: 'en', instanceId: 'game-1' });
  expect(api.canLeave()).toBe(true);
  expect(wrapper.find('.settings-actions').exists()).toBe(false);
  wrapper.unmount();
});

it('previews YAML into editable drafts and applies only after explicit save', async () => {
  const current = makeSnapshot();
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'get_extension_settings') return current;
    if (command === 'preview_extension_settings_yaml') return { values: { show_hints: false } };
    const values = args as Record<string, unknown>;
    return { ...current, values: { ...current.values, [values.fieldId as string]: values.value } };
  });
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId } });
  await flushPromises();
  await wrapper.get(control('display_name')).setValue('Existing edit');

  await wrapper.findAll('button').find(button => button.text() === '导入 YAML')!.trigger('click');
  const importArea = document.body.querySelector<HTMLTextAreaElement>('.settings-yaml-dialog textarea')!;
  importArea.value = 'display_name: From YAML';
  importArea.dispatchEvent(new Event('input', { bubbles: true }));
  await flushPromises();
  [...document.body.querySelectorAll<HTMLButtonElement>('.settings-yaml-dialog button')].find(button => button.textContent?.includes('预览设置'))!.click();
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('preview_extension_settings_yaml', { pluginId, locale: 'zh-CN', yaml: 'display_name: From YAML' });
  expect((wrapper.get(control('display_name')).element as HTMLInputElement).value).toBe('Existing edit');
  expect((wrapper.get(control('show_hints')).element as HTMLInputElement).checked).toBe(false);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_extension_setting')).toHaveLength(0);

  await wrapper.get('.settings-actions .secondary-button').trigger('click');
  await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'set_extension_setting')).toHaveLength(2);
  expect(vi.mocked(invoke).mock.calls.find(([command, args]) => command === 'set_extension_setting' && (args as Record<string, unknown>).fieldId === 'show_hints')?.[1]).toEqual({ pluginId, fieldId: 'show_hints', value: false, locale: 'zh-CN' });
  expect(vi.mocked(invoke).mock.calls.find(([command, args]) => command === 'set_extension_setting' && (args as Record<string, unknown>).fieldId === 'display_name')?.[1]).toEqual({ pluginId, fieldId: 'display_name', value: 'Existing edit', locale: 'zh-CN' });
  wrapper.unmount();
});

it('exports the selected instance settings and offers a copy action', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'get_extension_settings' ? makeSnapshot('game-1') : 'worker_count: 5\n');
  const wrapper = mount(ExtensionSettingsPanel, { props: { pluginId, instanceId: 'game-1', scope: 'instance' } });
  await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === '导出 YAML')!.trigger('click');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('export_extension_settings_yaml', { pluginId, instanceId: 'game-1', locale: 'zh-CN' });
  expect(document.body.querySelector<HTMLTextAreaElement>('.settings-yaml-dialog textarea')?.value).toBe('worker_count: 5\n');
  [...document.body.querySelectorAll<HTMLButtonElement>('.settings-yaml-dialog button')].find(button => button.textContent?.includes('复制 YAML'))!.click();
  await flushPromises();
  expect(writeText).toHaveBeenCalledWith('worker_count: 5\n');
  wrapper.unmount();
});
