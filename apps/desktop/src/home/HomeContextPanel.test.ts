// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenuContent, ContextMenuItem } from 'reka-ui';
import HomeContextPanel from './HomeContextPanel.vue';
import { setLocale } from '../i18n';
import { readHomeLayout, writeHomeLayout } from '../home-layout';
import { homeWorldResources } from './world-resources';
import { toolPresentation } from './tool-presentation';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
vi.mock('../after-home-paint',()=>({afterHomePaint:(run:()=>void)=>{run();return ()=>{};}}));
const game={id:'a',name:'A',version:'1.20.1',directory:'fixture',status:'installed' as const,error:null,progress:null};
const world={folder_name:'world-a',name:'Current world',last_played:100,minecraft_version:'1.20.1',game_mode:0,hardcore:false,metadata_error:false};
const png='data:image/png;base64,aWNvbg==';
afterEach(()=>{vi.mocked(invoke).mockReset();setLocale('zh-CN');localStorage.clear();vi.useRealTimers();});
it('ignores a late world response after changing games and defers reads while hidden',async()=>{
 setLocale('en-US');let old!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=> command==='get_home_world_summary'?Promise.resolve({thumbnail:null,last_backup:null}):(args as {id:string}).id==='a'?new Promise(resolve=>old=resolve):Promise.resolve([{...world,name:'New world',last_played:20}]));
 const wrapper=mount(HomeContextPanel,{props:{game,active:false,commands:[]}});
 expect(invoke).not.toHaveBeenCalled();await wrapper.setProps({active:true});
 await wrapper.setProps({game:{...game,id:'b'}});await flushPromises();old([{...world,name:'Old world',last_played:100}]);await flushPromises();
 expect(wrapper.text()).toContain('New world');expect(wrapper.text()).not.toContain('Old world');
 await wrapper.get('.home-context-worlds header button').trigger('click');expect(wrapper.emitted('section')).toEqual([['worlds']]);
 await wrapper.setProps({active:false});const reads=vi.mocked(invoke).mock.calls.length;await wrapper.setProps({game:{...game,id:'c'}});expect(invoke).toHaveBeenCalledTimes(reads);wrapper.unmount();
});
it('loads only the latest world summary, opens that exact world and reuses its image on return',async()=>{
 setLocale('en-US');
 vi.mocked(invoke).mockImplementation(async command=>command==='list_game_worlds'?[{...world,folder_name:'old',last_played:10},world]:{thumbnail:png,last_backup:'2026-10-26T12:00:00Z[UTC]'});
 const wrapper=mount(HomeContextPanel,{props:{game,active:true,commands:[]}});await flushPromises();
 expect(invoke).toHaveBeenCalledWith('get_home_world_summary',{id:'a',folderName:'world-a'});
 expect(wrapper.get('.home-world-art img').attributes('src')).toBe(png);
 expect(wrapper.get('.home-backup-state').text()).toContain('Last backup');
 await wrapper.get('.home-world-name').trigger('click');expect(wrapper.emitted('section')).toEqual([['worlds','world-a']]);
 await wrapper.setProps({active:false});await wrapper.setProps({active:true});await flushPromises();
 expect(vi.mocked(invoke).mock.calls.filter(([name])=>name==='get_home_world_summary')).toHaveLength(1);wrapper.unmount();
});
it('discards a stale image and backup result after switching games',async()=>{
 let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='list_game_worlds'?Promise.resolve([world]):(args as {id:string}).id==='a'?new Promise(resolve=>finish=resolve):Promise.resolve({thumbnail:null,last_backup:null}));
 const wrapper=mount(HomeContextPanel,{props:{game,active:true,commands:[]}});await flushPromises();
 await wrapper.setProps({game:{...game,id:'b'}});await flushPromises();finish({thumbnail:png,last_backup:'2026-10-26T12:00:00Z'});await flushPromises();
 expect(wrapper.find('.home-world-art img').exists()).toBe(false);expect(wrapper.get('.home-backup-state').text()).toBe('尚无备份');wrapper.unmount();
});
it('keeps a quiet placeholder for fast loads and delays both loading spinners',async()=>{
 vi.useFakeTimers();let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockReturnValue(new Promise(resolve=>finish=resolve));
 const wrapper=mount(HomeContextPanel,{props:{game,active:true,commands:[],loading:true}});
 expect(wrapper.findAll('.ui-busy-icon')).toHaveLength(0);expect(wrapper.text()).not.toContain('暂无');
 await vi.advanceTimersByTimeAsync(179);expect(wrapper.findAll('.ui-busy-icon')).toHaveLength(0);
 await vi.advanceTimersByTimeAsync(1);expect(wrapper.findAll('.ui-busy-icon')).toHaveLength(2);
 finish([]);await flushPromises();await wrapper.setProps({loading:false});expect(wrapper.findAll('.ui-busy-icon')).toHaveLength(0);wrapper.unmount();
});
it('retains the world thumbnail when backup metadata is unavailable',async()=>{
 setLocale('en-US');
 vi.mocked(invoke).mockImplementation(async command=>command==='list_game_worlds'?[world]:{thumbnail:png,last_backup:null,backup_unavailable:true});
 const wrapper=mount(HomeContextPanel,{props:{game,active:true,commands:[]}});await flushPromises();
 expect(wrapper.get('.home-world-art img').attributes('src')).toBe(png);
 expect(wrapper.get('.home-backup-state').text()).toBe('Backup status unavailable');wrapper.unmount();
});
it('uses the existing invitation modes and never launches a game',async()=>{
 vi.mocked(invoke).mockResolvedValue([]);const wrapper=mount(HomeContextPanel,{props:{game,active:true,commands:[]}});await flushPromises();
 await wrapper.get('.home-invite-actions .secondary-button').trigger('click');await wrapper.get('.home-invite-actions .text-button').trigger('click');
 expect(wrapper.emitted('mesh')).toEqual([['host'],['join']]);expect(vi.mocked(invoke).mock.calls.every(([command])=>command==='list_game_worlds')).toBe(true);wrapper.unmount();
});
it('pins and unpins through local home preferences and exposes unavailable action settings',async()=>{
 setLocale('en-US');vi.mocked(invoke).mockResolvedValue([]);
 const run=vi.fn(),command={id:'plugin:coordinate-book:coordinate-book.open',label:'Open Coordinate Book',run};
 const wrapper=mount(HomeContextPanel,{attachTo:document.body,props:{game,active:false,commands:[command],unavailableTools:1}});
 expect(wrapper.get('.home-context-tools button').text()).toBe('Coordinate Book');expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(0);
 await wrapper.get('.home-context-tools button').trigger('contextmenu',{button:2,clientX:80,clientY:80});await flushPromises();
 wrapper.findAllComponents(ContextMenuItem)[0]!.vm.$emit('select');await flushPromises();expect(readHomeLayout().quickIds).toEqual([command.id]);
 wrapper.findAllComponents(ContextMenuItem)[0]!.vm.$emit('select');await flushPromises();expect(readHomeLayout().quickIds).toEqual([]);expect(run).not.toHaveBeenCalled();
 writeHomeLayout({...readHomeLayout(),quickIds:Array.from({length:6},(_,i)=>`saved-${i}`)});
 wrapper.findAllComponents(ContextMenuItem)[0]!.vm.$emit('select');await flushPromises();expect(readHomeLayout().quickIds).toHaveLength(6);expect(wrapper.emitted('configureTools')).toHaveLength(1);
 await wrapper.get('.home-unavailable-tools').trigger('click');expect(wrapper.emitted('configureTools')).toHaveLength(2);wrapper.unmount();
});
it('shares only current bounded summary reads, refreshes expired metadata and retries failures',async()=>{
 vi.useFakeTimers();const load=homeWorldResources();vi.mocked(invoke).mockResolvedValue({thumbnail:'https://untrusted.invalid/icon.png',last_backup:null});
 const [one,two]=await Promise.all([load('a',world),load('a',world)]);expect(one.thumbnail).toBeNull();expect(two).toEqual(one);expect(invoke).toHaveBeenCalledTimes(1);
 await vi.advanceTimersByTimeAsync(15_001);await load('a',world);expect(invoke).toHaveBeenCalledTimes(2);
 await load('a',{...world,last_played:101});expect(invoke).toHaveBeenCalledTimes(3);
 vi.mocked(invoke).mockRejectedValueOnce(new Error('read failed'));await expect(load('b',world)).rejects.toThrow('read failed');await load('b',world);expect(invoke).toHaveBeenCalledTimes(5);
});
it('uses first-party IDs for distinct icons and preserves unknown manifest names',()=>{
 const action=(id:string,label:string)=>({id,label,run:vi.fn()});
 expect(toolPresentation(action('plugin:seed-compass:open','Open Seed Compass'))).toEqual({label:'Seed Compass',icon:'compass'});
 expect(toolPresentation(action('plugin:coordinate-book:open','打开坐标簿'))).toEqual({label:'坐标簿',icon:'pin'});
 expect(toolPresentation(action('plugin:unknown:open','Custom world tool'))).toEqual({label:'Custom world tool',icon:'puzzle'});
});
