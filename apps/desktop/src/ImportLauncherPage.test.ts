// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import ImportLauncherPage from './ImportLauncherPage.vue';
import TransferWorldScene from './ui/TransferWorldScene.vue';
import { setLocale } from './i18n';
import type { Game, LauncherImportPreview } from './games';
import type { TaskSnapshot } from './tasks';
import { clearPageWorkspacesForTests } from './use-page-workspace';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
const preview:LauncherImportPreview={preview_id:'host-preview',source_display:'PCL / Minecraft',candidates:[{candidate_id:'host-candidate',name:'PCL pack',minecraft_version:'1.21.1',loader_kind:'fabric',loader_version:'0.16.14',supported:true,unsupported_reason:null,content_roots:[{content_id:'isolated',label:'Instance folder',has_saves:true},{content_id:'shared',label:'Shared game folder',has_saves:false}],warnings:[]}]};
const game:Game={id:'imported',name:'PCL pack',version:'1.21.1',directory:'D:/Cobble/imported',status:'installing',error:null,progress:{phase:'import-copy',completed:2,total:4,reused:0}};
function task(state:TaskSnapshot['state']):TaskSnapshot{return{id:'task',game_id:game.id,game_name:game.name,title:'Import',kind:'game_install',state,progress:game.progress,error:null,can_retry:true,created_at:1,updated_at:2,has_result:false};}
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockImplementation(async command=>command==='get_download_settings'?{source:'automatic',mirror:''}:command==='default_game_directory'?'D:/Cobble':command==='discover_launcher_import'||command==='preview_launcher_import'?preview:command==='create_launcher_import'||command==='retry_game'?game:null);});
afterEach(()=>{clearPageWorkspacesForTests();setLocale('zh-CN');});
async function discover(wrapper:ReturnType<typeof mount>,root='isolated'){await wrapper.get('.launcher-import-discover').trigger('click');await flushPromises();await wrapper.findAll('.import-root-option').find(option=>option.attributes('value')===root)!.trigger('click');await wrapper.findAll('button').find(button=>button.text()==='Continue')!.trigger('click');await flushPromises();}
it('separates discovery, game selection and confirmation with paired minimal copy',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();expect(wrapper.get('.pack-picker').text()).toContain('Choose game folder');expect(wrapper.find('form').exists()).toBe(false);
 await wrapper.get('.pack-picker').trigger('click');await flushPromises();expect(wrapper.get('.import-candidate').text()).toContain('PCL pack');expect(wrapper.find('#launcher-import-name').exists()).toBe(false);
 await wrapper.findAll('.import-root-option')[0]!.trigger('click');await wrapper.findAll('button').find(button=>button.text()==='Continue')!.trigger('click');
 expect(wrapper.get('.done-button').text()).toBe('Import as separate game');expect(invoke).not.toHaveBeenCalledWith('create_launcher_import',expect.anything());
 setLocale('zh-CN');await flushPromises();expect(wrapper.get('label[for=launcher-import-name]').text()).toBe('游戏名称');expect(wrapper.get('.done-button').text()).toBe('导入为独立游戏');wrapper.unmount();
});
it('starts the exact isolated move and remains on real host progress instead of navigating',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);
 await wrapper.findAll('.import-mode-options [role=radio]').find(option=>option.attributes('value')==='move')!.trigger('click');expect(wrapper.text()).toContain('Recycle Bin');
 await wrapper.get('form').trigger('submit');await flushPromises();expect(invoke).toHaveBeenCalledWith('create_launcher_import',expect.objectContaining({previewId:'host-preview',candidateId:'host-candidate',contentId:'isolated',moveSource:true,includeSaves:false}));
 expect(wrapper.emitted('started')).toEqual([[game]]);expect(wrapper.emitted('open')).toBeUndefined();expect(wrapper.emitted('created')).toBeUndefined();expect(wrapper.find('.import-progress-step').exists()).toBe(true);
 expect(wrapper.findComponent(TransferWorldScene).props('progress')).toBe(.5);expect(wrapper.emitted('busy')?.at(-1)).toEqual([false]);wrapper.unmount();
});
it('clears move and save choices when switching to the shared root',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);
 await wrapper.findAll('.import-mode-options [role=radio]').find(option=>option.attributes('value')==='move')!.trigger('click');await wrapper.get('[role=switch]').trigger('click');
 await wrapper.get('.import-step-actions .secondary-button').trigger('click');await wrapper.findAll('.import-root-option').find(option=>option.attributes('value')==='shared')!.trigger('click');
 await wrapper.findAll('button').find(button=>button.text()==='Continue')!.trigger('click');expect(wrapper.find('.import-mode-options').exists()).toBe(false);expect(wrapper.find('[role=switch]').exists()).toBe(false);
 await wrapper.get('form').trigger('submit');await flushPromises();expect(invoke).toHaveBeenCalledWith('create_launcher_import',expect.objectContaining({contentId:'shared',moveSource:false,includeSaves:false}));wrapper.unmount();
});
it('does not invent aggregate progress and opens only after native installation completes',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);await wrapper.get('form').trigger('submit');await flushPromises();
 await wrapper.setProps({games:[{...game,progress:{phase:'files',completed:1,total:10,reused:0}}]});expect(wrapper.findComponent(TransferWorldScene).props('progress')).toBe(.5);expect(wrapper.text()).not.toContain('Open game');
 const complete={...game,status:'installed' as const,progress:null};await wrapper.setProps({games:[complete],tasks:[task('succeeded')]});expect(wrapper.findComponent(TransferWorldScene).props('state')).toBe('complete');
 expect(wrapper.emitted('open')).toBeUndefined();await wrapper.findAll('button').find(button=>button.text()==='Open game')!.trigger('click');expect(wrapper.emitted('open')).toEqual([[complete]]);wrapper.unmount();
});
it('restores the selected host game after navigation without cancelling or restarting its task',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);await wrapper.get('form').trigger('submit');await flushPromises();wrapper.unmount();
 const restored=mount(ImportLauncherPage,{props:{games:[game],tasks:[task('running')]}});await flushPromises();expect(restored.find('.import-progress-step').exists()).toBe(true);expect(restored.findComponent(TransferWorldScene).props('progress')).toBe(.5);
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='create_launcher_import')).toHaveLength(1);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='cancel_task')).toBe(false);restored.unmount();
});
it('shows real cancellation and retries through the existing native game pipeline',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);await wrapper.get('form').trigger('submit');await flushPromises();
 await wrapper.setProps({games:[{...game,status:'failed',error:'Stopped'}],tasks:[task('cancelled')]});expect(wrapper.findComponent(TransferWorldScene).props('state')).toBe('cancelled');
 await wrapper.findAll('button').find(button=>button.text()==='Retry task')!.trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('retry_game',{id:game.id});expect(wrapper.emitted('open')).toBeUndefined();wrapper.unmount();
});
it('cancels only through the host task and shows its returned cancelling state',async()=>{
 const wrapper=mount(ImportLauncherPage);await flushPromises();await discover(wrapper);await wrapper.get('form').trigger('submit');await flushPromises();
 await wrapper.setProps({games:[game],tasks:[task('running')]});
 vi.mocked(invoke).mockResolvedValue({...task('cancelling'),updated_at:3});
 await wrapper.findAll('button').find(button=>button.text()==='Cancel task')!.trigger('click');await flushPromises();
 expect(invoke).toHaveBeenCalledWith('cancel_task',{taskId:'task'});
 expect(wrapper.findComponent(TransferWorldScene).props('state')).toBe('waiting');
 expect(wrapper.findAll('button').find(button=>button.text().startsWith('Cancelling'))!.attributes('disabled')).toBeDefined();
 wrapper.unmount();
});
it('keeps unsupported games unimportable and ignores a discovery result after leaving',async()=>{
 let finish!:(value:unknown)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='get_download_settings'?{source:'automatic',mirror:''}:command==='default_game_directory'?'D:/Cobble':new Promise(resolve=>{finish=resolve;}));
 const wrapper=mount(ImportLauncherPage);await flushPromises();await wrapper.get('.launcher-import-discover').trigger('click');wrapper.unmount();finish(preview);await flushPromises();expect(invoke).not.toHaveBeenCalledWith('create_launcher_import',expect.anything());
 vi.mocked(invoke).mockImplementation(async command=>command==='get_download_settings'?{source:'automatic',mirror:''}:command==='default_game_directory'?'D:/Cobble':{...preview,candidates:[{...preview.candidates[0]!,supported:false,unsupported_reason:'Unsupported loader'}]});
 const next=mount(ImportLauncherPage);await flushPromises();await next.get('.launcher-import-discover').trigger('click');await flushPromises();expect(next.text()).toContain('Unsupported loader');expect(next.findAll('button').find(button=>button.text()==='Continue')!.attributes('disabled')).toBeDefined();next.unmount();
});
