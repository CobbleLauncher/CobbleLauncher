// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import Ajv from 'ajv';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { RuleEffect } from '@jsonforms/core';
import ToolFormPanel from './ToolFormPanel.vue';
import { setLocale } from './i18n';
import { toolFormDefaults, toolFormUiSchema, type ToolFormDefinition } from './tool-forms';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const form: ToolFormDefinition = {
  schema_version:1,
  schema:{ type:'object', additionalProperties:false, properties:{
    operation:{type:'string',title:'Operation',enum:['text','number'],default:'text'},
    version:{type:'string',title:'Version',enum:['1.20.1','1.21.4'],default:'1.20.1'},
    text:{type:'string',title:'Text',maxLength:64,default:'hello'},
    amount:{type:'integer',title:'Amount',minimum:1,maximum:10,default:1},
  },required:['operation','version','text','amount'] },
  ui_schema:{type:'VerticalLayout',elements:[
    {type:'Control',scope:'#/properties/operation'}, {type:'Control',scope:'#/properties/version'},
    {type:'Control',scope:'#/properties/text',rule:{effect:RuleEffect.SHOW,condition:{scope:'#/properties/operation',schema:{const:'text'}}}},
    {type:'Control',scope:'#/properties/amount',rule:{effect:RuleEffect.SHOW,condition:{scope:'#/properties/operation',schema:{const:'number'}}}},
  ]} as ToolFormDefinition['ui_schema'],
};
const result = {title:'Result',blocks:[{label:'Command',text:'/say <script>no HTML</script>'}]};
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(writeText).mockReset(); });
afterEach(() => { setLocale('zh-CN'); });
it('uses enum context defaults only when the exact supported version exists', () => {
  expect(toolFormDefaults(form,'1.21.4').version).toBe('1.21.4');
  expect(toolFormDefaults(form,'1.21.3').version).toBe('1.20.1');
  expect(toolFormDefaults(form,'1.21.4')).not.toHaveProperty('context');
  expect(() => toolFormUiSchema({type:'Control',rule:{effect:RuleEffect.SHOW,condition:{scope:'#/properties/operation',schema:{enum:['text']}}}})).toThrow();
});
it('renders conditional standard forms without AJV compilation, generates only on submit, and copies plain text', async () => {
  const compile = vi.spyOn(Ajv.prototype,'compile').mockImplementation(() => { throw new Error('CSP blocked eval'); });
  vi.mocked(invoke).mockResolvedValue(result);
  const wrapper = mount(ToolFormPanel,{props:{pluginId:'pocket',form,contextVersion:'1.21.4'}});
  await vi.dynamicImportSettled(); await flushPromises();
  expect(invoke).not.toHaveBeenCalled();
  expect(wrapper.text()).toContain('Text'); expect(wrapper.text()).not.toContain('Amount');
  await wrapper.find('select').setValue('"number"'); await vi.dynamicImportSettled(); await flushPromises();
  expect(wrapper.text()).toContain('Amount'); expect(wrapper.find('input[type=text]').exists()).toBe(false);
  await wrapper.find('input[type=number]').setValue(7);
  await wrapper.find('form').trigger('submit'); await vi.dynamicImportSettled(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('run_plugin_tool_form',{pluginId:'pocket',input:{operation:'number',version:'1.21.4',text:'hello',amount:7},locale:'en'});
  expect(wrapper.find('textarea').attributes('readonly')).toBeDefined();
  expect(wrapper.find('script').exists()).toBe(false);
  await wrapper.find('button[aria-label="Copy Command"]').trigger('click'); await vi.dynamicImportSettled(); await flushPromises();
  expect(writeText).toHaveBeenCalledWith(result.blocks[0]!.text);
  await wrapper.find('input[type=number]').setValue(8); await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(compile).not.toHaveBeenCalled(); wrapper.unmount();
});
it('keeps a pending operation busy until acknowledged and suppresses cancelled late output', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(command => command === 'run_plugin_tool_form' ? new Promise(resolve => {finish=resolve;}) : Promise.resolve(undefined));
  const wrapper=mount(ToolFormPanel,{props:{pluginId:'pocket',form}}); await vi.dynamicImportSettled(); await flushPromises();
  await wrapper.find('form').trigger('submit'); await wrapper.find('form').trigger('submit');
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='run_plugin_tool_form')).toHaveLength(1);
  expect(wrapper.find('select').attributes('disabled')).toBeDefined();
  await wrapper.findAll('button').find(button=>button.text()==='Cancel')!.trigger('click'); await vi.dynamicImportSettled(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_plugin_tool_form',{pluginId:'pocket'});
  expect(wrapper.attributes('aria-busy')).toBe('true');
  finish(result); await vi.dynamicImportSettled(); await flushPromises(); expect(wrapper.find('textarea').exists()).toBe(false);
  expect(wrapper.attributes('aria-busy')).toBe('false'); wrapper.unmount();
});
it('preserves edited input across language changes and cancels its own plugin on unmount', async () => {
  vi.mocked(invoke).mockImplementation(()=>new Promise(()=>{}));
  const wrapper=mount(ToolFormPanel,{props:{pluginId:'pocket',form}}); await vi.dynamicImportSettled(); await flushPromises();
  await wrapper.find('input[type=text]').setValue('edited'); setLocale('zh-CN'); await vi.dynamicImportSettled(); await flushPromises();
  expect(wrapper.find<HTMLInputElement>('input[type=text]').element.value).toBe('edited');
  expect(invoke).not.toHaveBeenCalled(); await wrapper.find('form').trigger('submit'); wrapper.unmount();
  expect(invoke).toHaveBeenCalledWith('cancel_plugin_tool_form',{pluginId:'pocket'});
});
it('shows bounded guest validation as plain text and clears it only after an edit', async () => {
  vi.mocked(invoke).mockRejectedValue({code:'PLUGIN_TOOL_INPUT',message:'Enter a valid coordinate.'});
  const wrapper=mount(ToolFormPanel,{props:{pluginId:'pocket',form}}); await vi.dynamicImportSettled(); await flushPromises();
  await wrapper.find('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toBe('Enter a valid coordinate.');
  await wrapper.find('input[type=text]').setValue('fixed'); await flushPromises();
  expect(wrapper.find('[role=alert]').exists()).toBe(false); wrapper.unmount();
});
