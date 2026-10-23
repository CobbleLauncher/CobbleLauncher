// SPDX-License-Identifier: MIT
import {mount,flushPromises} from '@vue/test-utils';
import {beforeEach,afterEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import ManagedConfigReportPanel from './ManagedConfigReportPanel.vue';
import {setLocale} from './i18n';
import type {Game} from './games';
import type {GameConfigFile} from './game-configs';
import type {PluginFolderReport} from './plugin-reports';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const games=[{id:'game-a',name:'Game A',status:'installed'},{id:'game-b',name:'Game B',status:'installed'}] as Game[];
const files:GameConfigFile[]=[{id:'opaque-a',name:'settings.json',size:24},{id:'opaque-b',name:'nested/other.toml',size:8}];
const result:PluginFolderReport={report:{schema_version:1,findings:[{code:'checked',severity:'info',title:'Checked',message:'No issues found',evidence:[],suggestions:[]}],tables:[]},files:2,skipped:1,truncated:true};
const props={pluginId:'community-config-reader',games,extensions:['json','toml']};
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockImplementation(async(command,args)=>{if(command==='list_game_configs')return files;if(command==='read_game_config')return{id:(args as {configId:string}).configId,name:'settings.json',text:'{"enabled":true}',revision:'current-revision',backups:[]};if(command==='analyze_game_config_report')return result.report;if(command==='analyze_game_config_batch_report')return result;return null;});});afterEach(()=>setLocale('zh-CN'));
const button=(wrapper:ReturnType<typeof mount<typeof ManagedConfigReportPanel>>,name:string)=>wrapper.findAll('button').find(value=>value.text()===name)!;
it('waits for an explicit game instead of silently selecting the first installed instance',async()=>{
 const wrapper=mount(ManagedConfigReportPanel,{props});await flushPromises();expect(invoke).not.toHaveBeenCalled();await wrapper.get('select').setValue('game-b');await flushPromises();expect(invoke).toHaveBeenCalledExactlyOnceWith('list_game_configs',{gameId:'game-b'});expect(wrapper.emitted('target')?.at(-1)).toEqual(['game-b']);expect(wrapper.findAll('.managed-config-files li')).toHaveLength(2);wrapper.unmount();
});
it('automatically lists the explicit current game but checks a selected file only after user action',async()=>{
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await flushPromises();expect(invoke).toHaveBeenCalledExactlyOnceWith('list_game_configs',{gameId:'game-a'});await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);await button(wrapper,'Check selected (1)').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('read_game_config',{gameId:'game-a',configId:'opaque-a'});expect(invoke).toHaveBeenCalledWith('analyze_game_config_report',{gameId:'game-a',configId:'opaque-a',expectedRevision:'current-revision',pluginId:props.pluginId,locale:'en'});expect(wrapper.text()).toContain('No issues found');wrapper.unmount();
});
it('opens original text only on the filename action and never writes configurations',async()=>{
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await flushPromises();expect(wrapper.find('textarea').exists()).toBe(false);await wrapper.get('.managed-config-name').trigger('click');await flushPromises();expect(wrapper.get<HTMLTextAreaElement>('textarea').element.value).toBe('{"enabled":true}');expect(wrapper.get('textarea').attributes('readonly')).toBeDefined();expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['list_game_configs','read_game_config']);wrapper.unmount();
});
it('checks all explicit eligible IDs and displays authoritative batch skip and limit counts',async()=>{
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await flushPromises();await wrapper.get('input[type="search"]').setValue('settings');await button(wrapper,'Check all (2)').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('analyze_game_config_batch_report',{gameId:'game-a',configIds:['opaque-a','opaque-b'],pluginId:props.pluginId,locale:'en'});expect(wrapper.text()).toContain('Checked 2 files · Skipped 1');expect(wrapper.text()).toContain('Scan limit reached');wrapper.unmount();
});
it('searches every config, preserves cross-page selection, and bounds batch actions',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='list_game_configs'?Array.from({length:75},(_,index)=>({id:`file-${index}`,name:`Config ${index}.json`,size:1})):result);
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await flushPromises();expect(wrapper.findAll('.managed-config-files li')).toHaveLength(50);expect(button(wrapper,'Check all (75)').attributes('disabled')).toBeDefined();await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);await button(wrapper,'Next').trigger('click');await wrapper.get('input[type="search"]').setValue('Config 74.json');expect(wrapper.findAll('.managed-config-files li')).toHaveLength(1);await wrapper.get('input[type="checkbox"]').setValue(true);await button(wrapper,'Check selected (2)').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('analyze_game_config_batch_report',expect.objectContaining({configIds:['file-0','file-74']}));wrapper.unmount();
});
it('discards late metadata from an old target and does not run its diagnostic',async()=>{
 let finish!:(value:GameConfigFile[])=>void;vi.mocked(invoke).mockImplementation(async(command,args)=>command==='list_game_configs'&&(args as {gameId:string}).gameId==='game-a'?new Promise<GameConfigFile[]>(resolve=>{finish=resolve;}):[]);
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await wrapper.get('select').setValue('game-b');await flushPromises();finish(files);await flushPromises();expect(wrapper.findAll('.managed-config-files li')).toHaveLength(0);expect(vi.mocked(invoke).mock.calls.every(([command])=>command==='list_game_configs')).toBe(true);wrapper.unmount();
});

it('cancels a pending revision read without starting a new plugin worker afterward',async()=>{
 let finish!:(value:unknown)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='list_game_configs'?files:command==='read_game_config'?new Promise(resolve=>{finish=resolve;}):null);
 const wrapper=mount(ManagedConfigReportPanel,{props:{...props,instanceId:'game-a'}});await flushPromises();await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);await button(wrapper,'Check selected (1)').trigger('click');await flushPromises();await button(wrapper,'Cancel').trigger('click');finish({id:'opaque-a',name:'settings.json',text:'{}',revision:'r1',backups:[]});await flushPromises();
 expect(vi.mocked(invoke).mock.calls.every(([command])=>['list_game_configs','read_game_config'].includes(command))).toBe(true);expect(wrapper.emitted('busy')).toEqual([[true],[false]]);expect(wrapper.find('.report-results').exists()).toBe(false);wrapper.unmount();
});
