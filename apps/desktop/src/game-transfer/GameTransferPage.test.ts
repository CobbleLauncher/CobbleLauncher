// SPDX-License-Identifier: MIT
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {flushPromises,mount,type VueWrapper} from '@vue/test-utils';
import {invoke} from '@tauri-apps/api/core';
import {setLocale} from '../i18n';
import type {TaskSnapshot} from '../tasks';
import GameTransferPage from './GameTransferPage.vue';
import TransferWorldScene from '../ui/TransferWorldScene.vue';
const events=vi.hoisted(()=>({update:null as null|((event:{payload:TaskSnapshot})=>void),stop:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async(_name,callback)=>{events.update=callback;return events.stop;})}));
const peer={profile:{peer_id:'peer-two',display_name:'Other computer',suffix:'2345',revision:1},connected:true,last_seen:0};
const source={gameId:'source',name:'Mountain home',version:'1.21.1',loaderKind:'fabric',loaderVersion:'0.16.0',availableScopes:['saves','mods','config','options'],meshAvailable:true,stopped:true};
const preview={offerId:'offer-one',gameName:source.name,version:source.version,loaderKind:'fabric',loaderVersion:'0.16.0',fileCount:15,totalBytes:1048576,sharedFileNames:true};
const offer={peerId:'peer-two',manifestId:'remote-one',gameName:source.name,version:source.version,loaderKind:'fabric',loaderVersion:'0.16.0',fileCount:15,totalBytes:1048576,scopes:['saves','mods']};
let rows:TaskSnapshot[],outgoing:unknown[],stopped:boolean,snapshot:unknown;
const wrappers:VueWrapper[]=[];
function task(state:TaskSnapshot['state'],phase='receiving',total=100):TaskSnapshot{return {id:'receive-task',game_id:'new-game',game_name:source.name,title:'Receive game',kind:'game_install',state,progress:{phase,completed:25,total,reused:0},error:null,can_retry:false,created_at:1,updated_at:state==='succeeded'?3:2,has_result:state==='succeeded'};}
beforeEach(()=>{
  setLocale('en-US');rows=[];outgoing=[];stopped=true;snapshot={profile:peer.profile,contacts:[peer]};events.update=null;events.stop.mockReset();
  vi.mocked(invoke).mockImplementation(async(command)=>{
    if(command==='mesh_app_snapshot')return snapshot;
    if(command==='game_transfer_context')return {...source,stopped};
    if(command==='game_transfer_list_outgoing')return outgoing;
    if(command==='game_transfer_prepare')return preview;
    if(command==='game_transfer_send')return {manifestId:'offer-one',totalBytes:1048576,totalFiles:15,state:'waiting'};
    if(command==='game_transfer_offers')return [offer];
    if(command==='game_transfer_accept'){rows=[task('queued')];return {taskId:'receive-task',gameId:'new-game'};}
    if(command==='list_tasks')return rows;
    if(command==='cancel_task')return task('cancelled');
    return undefined;
  });
});
afterEach(()=>{for(const wrapper of wrappers.splice(0))wrapper.unmount();vi.useRealTimers();vi.mocked(invoke).mockReset();setLocale('zh-CN');document.body.innerHTML='';});
async function open(props={}){const wrapper=mount(GameTransferPage,{props,attachTo:document.body});wrappers.push(wrapper);await flushPromises();return wrapper;}
function button(wrapper:VueWrapper,label:string){return wrapper.findAll('button').find(item=>item.text()===label)!;}
async function review(wrapper:VueWrapper){await wrapper.get('input[type=radio]').setValue();await button(wrapper,'Continue').trigger('click');await button(wrapper,'Continue').trigger('click');await flushPromises();}

it('requires a reviewed selection and explicit Send, then waits for actual acceptance',async()=>{
  const wrapper=await open({gameId:'source'});expect(wrapper.get('h2').text()).toBe('Choose a computer');
  await review(wrapper);expect(wrapper.get('h2').text()).toBe('Ready to send');expect(wrapper.text()).toContain('Selected files and their names');
  expect(invoke).toHaveBeenCalledWith('game_transfer_prepare',{gameId:'source',selection:{saves:true,mods:true,resourcepacks:false,shaderpacks:false,config:true,options:true}});
  expect(vi.mocked(invoke).mock.calls.some(([name])=>name==='game_transfer_send')).toBe(false);
  await button(wrapper,'Send game').trigger('click');await flushPromises();
  expect(invoke).toHaveBeenCalledWith('game_transfer_send',{offerId:'offer-one',peerId:'peer-two'});
  expect(wrapper.text()).toContain('Waiting for the other computer');expect(wrapper.text()).not.toContain('Game sent');expect(wrapper.getComponent(TransferWorldScene).props()).toMatchObject({state:'waiting',progress:null});expect(wrapper.get('progress').attributes('value')).toBeUndefined();
  await button(wrapper,'Continue in background').trigger('click');expect(wrapper.emitted('back')).toHaveLength(1);wrapper.unmount();expect(invoke).not.toHaveBeenCalledWith('game_transfer_cancel',expect.anything());
});
it('blocks a running source and gives a real connection entry when Mesh is absent',async()=>{
  stopped=false;const running=await open({gameId:'source'});await running.get('input[type=radio]').setValue();expect(button(running,'Continue').attributes('disabled')).toBeDefined();expect(running.text()).toContain('Close the source game');
  snapshot=null;const disconnected=await open({gameId:'source'});expect(disconnected.text()).toContain('Connect with Mesh');await button(disconnected,'Open connections').trigger('click');expect(disconnected.emitted('open-mesh')).toHaveLength(1);expect(vi.mocked(invoke).mock.calls.some(([name])=>name==='game_transfer_prepare')).toBe(false);
});
it('reviews source/version/content and receives into a new named game without launching it',async()=>{
  const wrapper=await open();await wrapper.get('.transfer-offer-list button').trigger('click');expect(wrapper.text()).toContain('Minecraft 1.21.1');expect(wrapper.text()).toContain('Mods contain executable code');expect(wrapper.text()).toContain('Default games folder');
  await wrapper.get('input[type=text]').setValue('  New home  ');await button(wrapper,'Receive game').trigger('click');await flushPromises();
  expect(invoke).toHaveBeenCalledWith('game_transfer_accept',{peerId:'peer-two',manifestId:'remote-one',name:'New home',useDefaultDirectory:true});expect(wrapper.emitted('task')).toEqual([['receive-task']]);
  events.update?.({payload:task('running')});await flushPromises();expect(wrapper.get('progress').attributes('value')).toBe('0.25');
  events.update?.({payload:task('succeeded')});await flushPromises();await button(wrapper,'Open new game').trigger('click');expect(wrapper.emitted('open-game')).toEqual([['new-game']]);expect(vi.mocked(invoke).mock.calls.some(([name])=>/launch_game|start_game/u.test(name))).toBe(false);
});
it('restores native receive tasks without requiring Mesh and ignores stale/unrelated updates',async()=>{
  snapshot=null;rows=[task('running','verifying',100)];const wrapper=await open({initialTaskId:'receive-task'});expect(wrapper.get('progress').attributes('value')).toBeUndefined();expect(wrapper.getComponent(TransferWorldScene).props()).toMatchObject({progress:null,state:'waiting'});expect(invoke).not.toHaveBeenCalledWith('mesh_app_snapshot');
  events.update?.({payload:{...task('succeeded'),id:'unrelated'}});events.update?.({payload:{...task('queued'),updated_at:1}});await flushPromises();expect(button(wrapper,'Open new game')).toBeUndefined();wrapper.unmount();expect(invoke).not.toHaveBeenCalledWith('cancel_task',expect.anything());
});
it('restores the exact source offer and cancels only from the explicit action',async()=>{
  outgoing=[{offerId:'offer-one',gameId:'source',gameName:source.name,peerId:'peer-two',state:'transferring',bytesDone:25,bytesTotal:100}];
  const wrapper=await open({initialOfferId:'offer-one'});expect(wrapper.get('progress').attributes('value')).toBe('0.25');await button(wrapper,'Cancel transfer').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('game_transfer_cancel',{offerId:'offer-one'});expect(wrapper.text()).toContain('Transfer cancelled');
});
it('releases an unsent prepared draft on exit, and supports explicitly declining an incoming offer',async()=>{
  const sender=await open({gameId:'source'});await review(sender);sender.unmount();expect(invoke).toHaveBeenCalledWith('game_transfer_cancel',{offerId:'offer-one'});
  const receiver=await open();await receiver.get('.transfer-offer-list button').trigger('click');await button(receiver,'Decline').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('game_transfer_reject',{peerId:'peer-two',manifestId:'remote-one'});expect(receiver.text()).toContain('No games waiting');
});
it('releases a late prepared offer after unmount instead of keeping a hidden source lock',async()=>{
  const wrapper=await open({gameId:'source'});await wrapper.get('input[type=radio]').setValue();await button(wrapper,'Continue').trigger('click');
  let resolve:((value:typeof preview)=>void)|undefined;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
  await button(wrapper,'Continue').trigger('click');wrapper.unmount();resolve?.(preview);await flushPromises();expect(invoke).toHaveBeenCalledWith('game_transfer_cancel',{offerId:'offer-one'});
});
it('stops reading an expired source offer and never retries the send automatically',async()=>{
  vi.useFakeTimers();outgoing=[{offerId:'offer-one',state:'waiting',bytesDone:0,bytesTotal:100}];const wrapper=await open({initialOfferId:'offer-one'});
  vi.mocked(invoke).mockRejectedValueOnce('TRANSFER_OFFER_EXPIRED');await vi.advanceTimersByTimeAsync(1001);await flushPromises();expect(wrapper.text()).toContain('Transfer interrupted');expect(wrapper.find('progress').exists()).toBe(false);
  const count=vi.mocked(invoke).mock.calls.length;await vi.advanceTimersByTimeAsync(4000);expect(vi.mocked(invoke).mock.calls).toHaveLength(count);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='game_transfer_send')).toBe(false);
});
it('deletes an unfinished received copy only after dangerous confirmation and returns to the inbox',async()=>{
  rows=[{...task('failed'),error:'TRANSFER_INTERRUPTED'}];const wrapper=await open({initialTaskId:'receive-task'});
  expect(invoke).not.toHaveBeenCalledWith('game_transfer_discard',expect.anything());await button(wrapper,'Delete unfinished copy').trigger('click');await flushPromises();
  expect(document.querySelector('[role=alertdialog]')?.textContent).toContain('The source game is kept');expect(invoke).not.toHaveBeenCalledWith('game_transfer_discard',expect.anything());
  (document.querySelector('.confirmation-cancel') as HTMLButtonElement).click();await flushPromises();expect(invoke).not.toHaveBeenCalledWith('game_transfer_discard',expect.anything());
  await button(wrapper,'Delete unfinished copy').trigger('click');await flushPromises();(document.querySelector('.confirmation-confirm') as HTMLButtonElement).click();await flushPromises();
  expect(invoke).toHaveBeenCalledWith('game_transfer_discard',{gameId:'new-game'});expect(wrapper.emitted('discarded')).toEqual([['new-game']]);expect(wrapper.emitted('task')).toEqual([['receive-task']]);expect(wrapper.get('h2').text()).toBe('Games sent to you');expect(wrapper.find('.transfer-status').exists()).toBe(false);
});
