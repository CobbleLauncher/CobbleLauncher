// SPDX-License-Identifier: MIT
import {mount,flushPromises} from '@vue/test-utils';
import {beforeEach,afterEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import PluginReportPanel from './PluginReportPanel.vue';
import {setLocale} from './i18n';
import type {Game} from './games';
import type {PluginReport,PluginFolderReport} from './plugin-reports';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const manifest={api_version:'0.2.0',report_protocols:['cobble.config-folder@1'],permissions:[{permission:'launcher:resource/selected-folder/read'}]};
const report:PluginReport={schema_version:1,findings:[{code:'config_syntax_checked',severity:'info',title:'Configuration checked',message:'Syntax check completed.',evidence:[],suggestions:[]}],tables:[]};
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockResolvedValue(null);});afterEach(()=>setLocale('zh-CN'));
const find=(wrapper:ReturnType<typeof mount<typeof PluginReportPanel>>,name:string)=>wrapper.findAll('button').find(button=>button.text()===name);
it('offers folder checks only for the declared protocol and explicit folder access',async()=>{
 const wrapper=mount(PluginReportPanel,{props:{pluginId:'ordinary-report',manifest}});expect(find(wrapper,'Check folder…')).toBeDefined();expect(wrapper.find('input').exists()).toBe(false);
 await wrapper.setProps({manifest:{...manifest,permissions:[]}});expect(find(wrapper,'Check folder…')).toBeUndefined();await wrapper.setProps({manifest:{...manifest,report_protocols:[]}});expect(find(wrapper,'Check folder…')).toBeUndefined();await wrapper.setProps({manifest:{...manifest,api_version:'0.1.0'}});expect(find(wrapper,'Check folder…')).toBeUndefined();expect(find(wrapper,'Analyze files…')).toBeDefined();wrapper.unmount();
});
it('uses the native folder picker command and renders its checked/skipped structured summary',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='list_game_configs'?[]:{report,files:2,skipped:3,truncated:true});const wrapper=mount(PluginReportPanel,{props:{pluginId:'third-party-config-checker',instanceId:'owned-game',manifest,games:[{id:'owned-game',name:'Owned',status:'installed'} as Game]}});await find(wrapper,'Check folder…')!.trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('analyze_plugin_folder_report',{pluginId:'third-party-config-checker',instanceId:'owned-game',locale:'en'});expect(wrapper.text()).toContain('Checked 2 files · Skipped 3');expect(wrapper.text()).toContain('Scan limit reached');expect(wrapper.find('.report-results').exists()).toBe(true);wrapper.unmount();
});
it('keeps the ordinary file action and treats canceled native folder selection as no report',async()=>{
 const wrapper=mount(PluginReportPanel,{props:{pluginId:'config-checker',manifest}});await find(wrapper,'Analyze files…')!.trigger('click');await flushPromises();expect(invoke).toHaveBeenLastCalledWith('analyze_plugin_report',{pluginId:'config-checker',locale:'en'});await find(wrapper,'Check folder…')!.trigger('click');await flushPromises();expect(wrapper.find('.report-results').exists()).toBe(false);expect(wrapper.find('[role="alert"]').exists()).toBe(false);wrapper.unmount();
});
it('shares pending and cancellation with the current report action until the worker finishes',async()=>{
 let finish!:(value:null)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='analyze_plugin_folder_report'?new Promise<null>(resolve=>{finish=resolve;}):undefined);
 const wrapper=mount(PluginReportPanel,{props:{pluginId:'config-checker',manifest}});await find(wrapper,'Check folder…')!.trigger('click');await flushPromises();expect(find(wrapper,'Analyze files…')!.attributes('disabled')).toBeDefined();expect(find(wrapper,'Checking folder…')!.find('.ui-busy-icon').exists()).toBe(true);
 await find(wrapper,'Cancel')!.trigger('click');expect(invoke).toHaveBeenCalledWith('cancel_plugin_report');expect(find(wrapper,'Cancel')!.attributes('disabled')).toBeDefined();finish(null);await flushPromises();expect(find(wrapper,'Check folder…')!.attributes('disabled')).toBeUndefined();expect(wrapper.find('.report-results').exists()).toBe(false);wrapper.unmount();
});
it('ignores a stale folder result after the instance context changes',async()=>{
 let finish!:(value:PluginFolderReport)=>void;vi.mocked(invoke).mockImplementation(async()=>new Promise<PluginFolderReport>(resolve=>{finish=resolve;}));const wrapper=mount(PluginReportPanel,{props:{pluginId:'config-checker',instanceId:'one',manifest}});await find(wrapper,'Check folder…')!.trigger('click');await wrapper.setProps({instanceId:'two'});finish({report,files:2,skipped:3,truncated:false});await flushPromises();expect(wrapper.find('.report-results').exists()).toBe(false);wrapper.unmount();
});

it('keeps external pickers secondary and uses the explicitly selected managed game as their context',async()=>{
 const games=[{id:'one',name:'One',status:'installed'},{id:'two',name:'Two',status:'installed'}] as Game[];
 vi.mocked(invoke).mockImplementation(async command=>command==='list_game_configs'?[]:null);
 const wrapper=mount(PluginReportPanel,{props:{pluginId:'community-checker',manifest:{...manifest,file_extensions:['json']},games,instanceId:'one'}});await flushPromises();expect(invoke).toHaveBeenCalledWith('list_game_configs',{gameId:'one'});expect(find(wrapper,'Check folder…')!.isVisible()).toBe(false);
 await wrapper.get('select').setValue('two');await flushPromises();await find(wrapper,'Other files or folders')!.trigger('click');await flushPromises();expect(find(wrapper,'Other files or folders')!.attributes('aria-expanded')).toBe('true');await find(wrapper,'Check folder…')!.trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('analyze_plugin_folder_report',{pluginId:'community-checker',instanceId:'two',locale:'en'});wrapper.unmount();
});
