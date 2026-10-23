// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import MeshInspector from './MeshInspector.vue';
import { setLocale } from './i18n';
import type { MeshOverview } from './mesh';
import { messageMetadata } from './mesh-inspector';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const overview: MeshOverview = { available: true, reason: null, can_host: true, component_state: 'running', mode: 'explicit_relay', discovery_state: 'published', discovery_source: 'https://fixture.invalid', discovery_total: 1, nodes: [{ peer_id: 'PRIVATE_PEER', endpoint: '/dns4/relay.invalid/tcp/4001/p2p/PRIVATE_PEER', relay: true, discovery: true, expires_at: 2000000000, evidence: 'signed_advertisement' }], configured_services: [{ id: 'official',kind: 'relay', endpoint:'/dns4/fixture.invalid/tcp/4001/p2p/PRIVATE_PEER',state:'authenticated',discovery:'unavailable',game_relay:false }],neighbors:[],capabilities:{relay:true,direct:false,hole_punch:false},relay:{address:null,source:null,state:'unconfigured'},session:null };
const app = { profile: { peer_id:'SECRET_ID',display_name:'Private Name',suffix:'ABCD',revision:1 },contacts:[{ profile:{peer_id:'PRIVATE_PEER',display_name:'Contact Name',suffix:'EFGH',revision:1},connected:true,last_seen:42 }],capabilities:{} };
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockImplementation(async command => command === 'mesh_overview' ? overview : command === 'mesh_app_snapshot' ? app : { messages:[{sequence:1,id:'SECRET_MESSAGE_ID',peer_id:'PRIVATE_PEER',direction:'incoming',text:'PRIVATE BODY 🔒',at:42}],next_after:1,oldest_sequence:1,has_more:false,generation:'generation1' }); });
afterEach(() => { vi.clearAllMocks(); setLocale('zh-CN'); });
it('does not inspect until active and never starts or probes services', async () => {
  const wrapper=mount(MeshInspector,{props:{active:false}}); await flushPromises(); expect(invoke).not.toHaveBeenCalled();
  await wrapper.setProps({active:true}); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['mesh_overview','mesh_app_snapshot','mesh_list_messages']);
  expect(wrapper.text()).toContain('Transport neighbors'); expect(wrapper.text()).toContain('Authenticated'); expect(wrapper.text()).toContain('Unavailable');
  expect(wrapper.text()).not.toContain('PRIVATE BODY'); expect(wrapper.text()).not.toContain('Contact Name');
  await wrapper.findAll('button').find(button=>button.text()==='Copy diagnostics')!.trigger('click'); await flushPromises();
  const copy=String(vi.mocked(writeText).mock.calls[0]![0]);
  for(const secret of ['PRIVATE BODY','PRIVATE_PEER','SECRET_ID','SECRET_MESSAGE_ID','Private Name','fixture.invalid']) expect(copy).not.toContain(secret);
  expect(JSON.parse(copy).local).toMatchObject({transportNeighbors:0,connectedContacts:1});
  wrapper.unmount();
});
it('maps UTF-8 message size and ephemeral aliases without retaining text',()=>{
  const result=messageMetadata([{sequence:1,id:'id',peer_id:'peer',direction:'outgoing',text:'中',at:3}],new Map());
  expect(result).toEqual([{sequence:1,direction:'outgoing',at:3,bytes:3,contact:1}]);
});
