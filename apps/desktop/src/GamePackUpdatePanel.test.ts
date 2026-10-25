// SPDX-License-Identifier: MIT
import {flushPromises, mount} from '@vue/test-utils';
import {afterEach, beforeEach, expect, it, vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import GamePackUpdatePanel from './GamePackUpdatePanel.vue';
import SelectField from './ui/SelectField.vue';
import {setLocale} from './i18n';
import {packUpdateGroups, type PackUpdateContext, type PackUpdatePreview, type PackFileChange} from './pack-update';
vi.mock('@tauri-apps/api/core', () => ({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/event', () => ({listen:vi.fn().mockResolvedValue(()=>{})}));
const context: PackUpdateContext = {supported:true,reason:null,provider:'mrpack',name:'Fixture pack',installed_version:'v1',online_versions:false,identity_relation:'unknown'};
function change(path:string,local_state:PackFileChange['local_state']='clean',preserve=false):PackFileChange {return {before:{identity:path,version_id:'old',path,size_bytes:1,sha1:'a',sha512:null},after:{identity:path,version_id:'new',path,size_bytes:2,sha1:'b',sha512:null},local_state,preserve};}
function preview():PackUpdatePreview {return {plan_id:'plan',player_added:[],migratable_paths:[],baseline_available:true,baseline_status:'verified',target_name:'Target fixture pack',excluded_optional_files:0,observed_file_count:0,unobserved_file_count:0,current_version:'v1',target_version:'v2',identity_relation:'unknown',runtime_changed:false,added:[],removed:[],version_changed:[],unchanged:[]};}
beforeEach(() => {setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockImplementation(async command => command === 'get_game_pack_update_context' ? context : preview());});
afterEach(() => setLocale('zh-CN'));
it('loads native eligibility without opening a picker and prevents unsupported comparisons',async()=>{
 vi.mocked(invoke).mockResolvedValue({...context,supported:false,reason:'source_unavailable'});
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();
 expect(wrapper.vm.supported).toBe(false);await wrapper.vm.choose();
 expect(invoke).toHaveBeenCalledExactlyOnceWith('get_game_pack_update_context',{gameId:'game'});expect(wrapper.find('section').exists()).toBe(false);wrapper.unmount();
});
it('treats picker cancellation as cancellation with no result or success',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:null);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 expect(wrapper.find('section').exists()).toBe(false);expect(wrapper.emitted('busy')).toEqual([[true],[false]]);expect(wrapper.find('[role="alert"]').exists()).toBe(false);wrapper.unmount();
});
it('separates actual-change conflicts from protected clean, missing and unchanged files',()=>{
 const result=preview();const modified=change('mods/edited.jar','modified',true);const unknown=change('mods/unverified.jar','unknown',true);
 result.added=[unknown];result.version_changed=[modified,change('config/options.toml','clean',true),change('mods/missing.jar','missing')];result.unchanged=[change('mods/kept.jar','modified',true)];
 expect(packUpdateGroups(result).conflicts).toEqual([unknown,modified]);
});
it('paginates fifty rows and searches the full preview, while preserving accurate group totals',async()=>{
 const result=preview();result.version_changed=Array.from({length:121},(_,index)=>change(`mods/file-${index}.jar`));result.unchanged=[change('config/untouched.toml','clean',true)];
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:result);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 expect(wrapper.findAll('.pack-update-files li')).toHaveLength(50);expect(wrapper.text()).toContain('1–50 / 121');expect(wrapper.text()).not.toContain('config/untouched.toml');
 await wrapper.findAll('button').find(button=>button.text()==='Next')!.trigger('click');expect(wrapper.text()).toContain('51–100 / 121');
 await wrapper.get('input[type="search"]').setValue('file-120.jar');expect(wrapper.findAll('.pack-update-files li')).toHaveLength(1);expect(wrapper.text()).toContain('mods/file-120.jar');expect(wrapper.findAll('.pack-update-groups button').find(button=>button.text().startsWith('Changed'))!.text()).toContain('121');
 expect(wrapper.text()).toContain('Create update copy');wrapper.unmount();
});
it('ignores a completed preview from the previous game context',async()=>{
 let complete!:(value:PackUpdatePreview)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:new Promise<PackUpdatePreview>(resolve=>{complete=resolve;}));
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'one'}});await flushPromises();const pending=wrapper.vm.choose();await wrapper.setProps({gameId:'two'});await flushPromises();complete(preview());await pending;await flushPromises();
 expect(wrapper.find('section').exists()).toBe(false);expect(wrapper.vm.pending).toBe(false);wrapper.unmount();
});
it('reports a real failed comparison without displaying the previous file result or applying changes',async()=>{
 let fail=false;vi.mocked(invoke).mockImplementation(async command=>{if(command==='get_game_pack_update_context')return context;if(fail)throw 'PACK_UPDATE_SOURCE_CHANGED';return preview();});
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();fail=true;await wrapper.vm.choose();await flushPromises();
 expect(wrapper.get('[role="alert"]').text()).toContain('installed pack changed');expect(wrapper.find('.pack-update-versions').exists()).toBe(false);expect(wrapper.vm.pending).toBe(false);expect(vi.mocked(invoke).mock.calls.every(([command])=>['get_game_pack_update_context','preview_game_pack_update'].includes(command))).toBe(true);wrapper.unmount();
});

it('shows the actual target pack name and handles native file-count limits clearly',async()=>{
 let fail=false;vi.mocked(invoke).mockImplementation(async command=>{if(command==='get_game_pack_update_context')return context;if(fail)throw 'PACK_UPDATE_TOO_MANY_FILES';return preview();});
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 expect(wrapper.get('.pack-update-versions').text()).toContain('Fixture pack · v1');expect(wrapper.get('.pack-update-versions').text()).toContain('Target fixture pack · v2');
 fail=true;await wrapper.vm.choose();await flushPromises();expect(wrapper.get('[role="alert"]').text()).toContain('too many files');wrapper.unmount();
});
it('requires a conflict choice and selects player-added files individually when baseline is missing',async()=>{
 const value=preview();value.baseline_available=false;value.baseline_status='missing';value.player_added=['mods/player.jar','config/notes.txt'];value.migratable_paths=['config/settings.txt','mods/player.jar','config/notes.txt'];value.version_changed=[change('config/settings.txt','modified',true)];
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:command==='preview_game_pack_update'?value:command==='start_game_pack_update_copy'?'task-1':[]);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 expect(wrapper.text()).toContain('original pack file is missing');
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeDefined();
 expect(wrapper.get('.pack-update-added').text()).toContain('0 selected');
 await wrapper.get('.pack-update-choice [role="radio"]').trigger('click');
 await wrapper.findAll('.pack-update-added li input')[0]!.setValue(true);
 await wrapper.get('.pack-update-copy-form button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('start_game_pack_update_copy',{request:{planId:'plan',name:'Target fixture pack v2',preservePaths:['mods/player.jar','config/settings.txt'],replaceNewMods:[],includeWorlds:false}});
 expect(listen).toHaveBeenCalled();wrapper.unmount();
});
it('requires an explicit replacement decision when retaining a removed local Mod',async()=>{
 const value=preview();const old=change('mods/old.jar','modified',true).before!;const next=change('mods/new.jar').after!;
 value.removed=[{before:old,after:null,local_state:'modified',preserve:true}];value.added=[{before:null,after:next,local_state:'missing',preserve:false}];value.migratable_paths=['mods/old.jar'];
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:command==='preview_game_pack_update'?value:command==='start_game_pack_update_copy'?'task-2':[]);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 await wrapper.get('.pack-update-choice [role="radio"]').trigger('click');
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeDefined();
 expect(wrapper.text()).toContain('Choose for this Mod');
 wrapper.findComponent(SelectField).vm.$emit('update:modelValue','mods/new.jar');await flushPromises();
 await wrapper.get('.pack-update-copy-form button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('start_game_pack_update_copy',{request:{planId:'plan',name:'Target fixture pack v2',preservePaths:['mods/old.jar'],replaceNewMods:[{oldPath:'mods/old.jar',newPath:'mods/new.jar'}],includeWorlds:false}});
 wrapper.unmount();
});
it('treats a selected player-added Mod as an explicit decision beside new Mods',async()=>{
 const value=preview();value.baseline_available=false;value.baseline_status='missing';value.player_added=['mods/player.jar'];value.migratable_paths=['mods/player.jar'];
 value.added=[{before:null,after:change('mods/new.jar').after!,local_state:'missing',preserve:false}];
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:command==='preview_game_pack_update'?value:command==='start_game_pack_update_copy'?'task-3':[]);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeUndefined();
 await wrapper.get('.pack-update-added li input').setValue(true);await flushPromises();
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeDefined();
 wrapper.findComponent(SelectField).vm.$emit('update:modelValue','both');await flushPromises();
 await wrapper.get('.pack-update-copy-form button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('start_game_pack_update_copy',{request:{planId:'plan',name:'Target fixture pack v2',preservePaths:['mods/player.jar'],replaceNewMods:[{oldPath:'mods/player.jar',newPath:null}],includeWorlds:false}});
 wrapper.unmount();
});
it('previews three JSON sides and requires an explicit conflicting field choice',async()=>{
 const value=preview();value.migratable_paths=['config/settings.json'];value.version_changed=[change('config/settings.json','modified',true)];
 value.json_configs=[{path:'config/settings.json',fields:[{pointer:'/difficulty',base:{exists:true,value:1},local:{exists:true,value:2},upstream:{exists:true,value:3},conflict:true,suggested:'local'},{pointer:'/localOnly',base:{exists:false,value:null},local:{exists:true,value:true},upstream:{exists:false,value:null},conflict:false,suggested:'local'}]}];
 vi.mocked(invoke).mockImplementation(async command=>command==='get_game_pack_update_context'?context:command==='preview_game_pack_update'?value:command==='start_game_pack_update_copy'?'task-json':[]);
 const wrapper=mount(GamePackUpdatePanel,{props:{gameId:'game'}});await flushPromises();await wrapper.vm.choose();await flushPromises();
 await wrapper.findAll('.pack-update-choice-option').find(button=>button.text()==='Merge fields')!.trigger('click');
 expect(wrapper.get('.pack-update-json').text()).toContain('Original pack');
 expect(wrapper.get('.pack-update-json').text()).toContain('/difficulty');
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeDefined();
 const conflict=wrapper.findAll('.pack-update-json tbody tr')[0]!;
 expect(conflict.findAll('[role="radio"]').every(button=>button.attributes('aria-checked')!=='true')).toBe(true);
 await conflict.findAll('[role="radio"]')[0]!.trigger('click');
 expect(wrapper.get('.pack-update-copy-form button').attributes('disabled')).toBeUndefined();
 await wrapper.get('.pack-update-copy-form button').trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('start_game_pack_update_copy',{request:{planId:'plan',name:'Target fixture pack v2',preservePaths:[],replaceNewMods:[],includeWorlds:false,jsonMerges:[{path:'config/settings.json',fields:[{pointer:'/localOnly',side:'local'},{pointer:'/difficulty',side:'local'}]}]}});
 wrapper.unmount();
});
