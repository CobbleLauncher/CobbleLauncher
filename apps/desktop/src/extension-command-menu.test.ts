// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { defineComponent, type PropType } from 'vue';
import type { ExtensionContext } from './extensions/contributions';
import { DropdownMenuRoot, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import ExtensionMenuItems from './ExtensionMenuItems.vue';
import InstanceExtensionSettings from './InstanceExtensionSettings.vue';
import PluginReportPanel from './PluginReportPanel.vue';
import InspectorPanel from './InspectorPanel.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); document.body.innerHTML = ''; });
const contribution = { pluginId: 'custom.report', pluginName: '<img>Example', commands: [{ id: 'inspect', label: '<script>Inspect</script>', workflow: 'report', menus: [{ position: 'instance', when: 'instance_selected' }] }, { id: 'tools-only', label: 'Tools only', workflow: 'report', menus: [{ position: 'tools', when: 'always' }] }], settings: [{ id: 'options', label: 'Options', fields: [{ scope: 'global' }, { scope: 'instance' }] }] };
const MenuHarness = defineComponent({
  components: { DropdownMenuRoot, DropdownMenuContent, DropdownMenuItem, ExtensionMenuItems },
  props: { position: {type:String,required:true}, context: {type:Object as PropType<ExtensionContext>,default:undefined}, disabled: Boolean },
  template: '<DropdownMenuRoot :open="true"><DropdownMenuContent><DropdownMenuItem>Native action</DropdownMenuItem><ExtensionMenuItems :position="position" :context="context" :disabled="disabled" /></DropdownMenuContent></DropdownMenuRoot>',
});
it('renders contextual actions beside native items and resolves the exact position/context', async () => {
  setLocale('en-US'); const binding = { pluginId: 'custom.report', commandId: 'inspect', label: 'Inspect', workflow: 'report', instanceId: 'game' };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_extension_contributions' ? [contribution] : binding));
  const wrapper = mount(MenuHarness, { props: { position: 'instance', context: { instanceId: 'game' } }, attachTo: document.body });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_extension_contributions', { position:'instance', locale: 'en', context: { instanceId: 'game' } });
  const menu = new DOMWrapper(document.body); expect(menu.findAll('[role="menuitem"]')).toHaveLength(2); expect(menu.findAll('img,script')).toHaveLength(0);
  expect(menu.text()).not.toContain('Extension actions');
  await menu.findAll('[role="menuitem"]')[1]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('resolve_extension_command', { pluginId: 'custom.report', commandId: 'inspect', position:'instance', locale: 'en', context: { instanceId: 'game' } });
  expect(wrapper.findComponent(ExtensionMenuItems).emitted('selected')).toEqual([[binding]]); wrapper.unmount();
});
it('renders no extension structure for zero matching actions in either language', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  for (const language of ['en-US','zh-CN'] as const) {
    setLocale(language);
    const wrapper=mount(MenuHarness,{props:{position:'blueprint',context:{blueprintId:'saved'}},attachTo:document.body});
    await flushPromises();
    expect(wrapper.findAll('[role="menuitem"]')).toHaveLength(1);
    expect(wrapper.findAll('[role="group"],[role="separator"],button')).toHaveLength(0);
    expect(wrapper.text()).toBe('Native action');
    wrapper.unmount();
  }
});
it('blocks disabled actions and discards resolution after a context change', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'list_extension_contributions' ? Promise.resolve([contribution]) : new Promise(resolve => { finish = resolve; }));
  const wrapper = mount(MenuHarness, { props: { position: 'instance', context: { instanceId: 'first' },disabled:true }, attachTo: document.body }); await flushPromises();
  await wrapper.findAll('[role="menuitem"]')[1]!.trigger('click');
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='resolve_extension_command')).toBe(false);
  await wrapper.setProps({disabled:false});
  await wrapper.findAll('[role="menuitem"]')[1]!.trigger('click'); await wrapper.setProps({ context: { instanceId: 'second' } });
  finish({ pluginId: 'custom.report', commandId: 'inspect', label: 'Inspect', workflow: 'report', instanceId: 'first' }); await flushPromises();
  expect(wrapper.findComponent(ExtensionMenuItems).emitted('selected')).toBeUndefined(); wrapper.unmount();
});
it('lazily opens instance-only settings with the registered instance id and blocks leaving dirty fields', async () => {
  setLocale('en-US');
  const field = { type: 'string', advanced: false, default: '', options: [] };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_extension_contributions' ? [contribution] : { pluginId: 'custom.report', instanceId: 'game', values: { global: '', instance: '' }, categories: [{ id: 'options', label: 'Options', fields: [{ ...field, id: 'global', label: 'Global option', scope: 'global' }, { ...field, id: 'instance', label: 'Instance option', scope: 'instance' }] }] }));
  const wrapper = mount(InstanceExtensionSettings, { props: { instanceId: 'game' } }); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'get_extension_settings')).toBe(false);
  await wrapper.get('.settings-disclosure-trigger').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_extension_settings', { pluginId: 'custom.report', instanceId: 'game', locale: 'en' });
  expect(wrapper.text()).not.toContain('Global option');
  await wrapper.get('input').setValue('draft'); expect(wrapper.vm.canLeave()).toBe(false); wrapper.unmount();
});
it('passes instance context to both finite report workflows without paths or settings values', async () => {
  vi.mocked(invoke).mockResolvedValue(null);
  const report = mount(PluginReportPanel, { props: { pluginId: 'custom.report', instanceId: 'game' } }); await report.get('.secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('analyze_plugin_report', { pluginId: 'custom.report', instanceId: 'game', locale: 'zh-CN' }); report.unmount();
  const inspector = mount(InspectorPanel, { props: { pluginId: 'custom.inspector', instanceId: 'game' } }); await inspector.get('.secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('begin_pack_inspection', { pluginId: 'custom.inspector', instanceId: 'game', compare: false, locale: 'zh-CN' }); inspector.unmount();
});

it('scopes a pending Inspector cancellation to the exact plugin and instance', async () => {
  let finish!: (value: null) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'begin_pack_inspection' ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(null));
  const wrapper = mount(InspectorPanel, { props: { pluginId: 'custom.inspector', instanceId: 'first-game' } });
  await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
  wrapper.unmount();
  expect(invoke).toHaveBeenCalledWith('close_pack_inspection', { pluginId: 'custom.inspector', instanceId: 'first-game' });
  finish(null); await flushPromises();
});
