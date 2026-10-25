// SPDX-License-Identifier: MIT
/* eslint-disable vue/one-component-per-file -- Component doubles exercise separate page boundaries. */
import { defineComponent } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import MeshPage from './MeshPage.vue';
import type { Game } from './games';
import type { MeshSession } from './mesh';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const game = { id: 'owned-game', name: 'Creative', status: 'installed', launch_state: 'running' } as Game;
const overview = { available: true, reason: null, can_host: true, component_state: 'idle', mode: 'explicit_relay', discovery_state: 'inactive', neighbors: [], nodes: [{peer_id:'relay-id',endpoint:'/ip4/203.0.113.10/tcp/4001/p2p/fixture',relay:true,discovery:false,expires_at:2000000000,evidence:'signed_advertisement'}], capabilities: { relay: true, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' } };
const hostOptions = { game_id: game.id, lan: { state: 'not_open', port: null }, relay: overview.relay };
const session: MeshSession = { session_id: 7, target: 'game', local_port: null, game_id: game.id, role: 'host', state: 'listening', route: 'explicit_relay', scope: 'public', expires_at: 2000000000 };
beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_host_options' ? hostOptions : command === 'mesh_overview' ? { ...overview, session: null } : { session: null });
});
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.clearAllMocks(); setLocale('zh-CN'); });
function mountPage(options: Parameters<typeof mount<typeof MeshPage>>[1], additionalStubs: Record<string, unknown> = {}) {
  return mount(MeshPage, { ...options, global: { ...options?.global, stubs: {
    MeshSocialPanel: defineComponent({ setup(_, { expose }) { expose({ canLeave: () => true, confirmLeave: async () => true }); return () => null; } }),
    ...(options?.global?.stubs ?? {}),
    ...additionalStubs,
  } } });
}
const props = { games: [game], initialGameId: game.id, active: true };

it('keeps the Inspector action available offline with no games in Contacts mode', async () => {
  vi.mocked(invoke).mockResolvedValue({ available: false, reason: 'MESH_OFFLINE', can_host: false });
  const InspectorStub = defineComponent({ props: { active: Boolean }, template: '<section data-inspector />' });
  const wrapper = mountPage({ props: { games: [], initialGameId: '', active: true } }, { MeshInspector: InspectorStub });
  await flushPromises();
  await wrapper.findAll('button').find(item => item.text() === 'Contacts')!.trigger('click');
  await flushPromises();
  const button = wrapper.findAll('button').find(item => item.text().includes('Mesh Inspector'));
  expect(button).toBeDefined();
  expect(wrapper.text()).toContain('Mesh is offline');
  await button!.trigger('click');
  expect(wrapper.find('[data-inspector]').exists()).toBe(true);
  wrapper.unmount();
});

it('honors the contacts draft guard before opening the Inspector', async () => {
  const confirmLeave = vi.fn(async () => false);
  const SocialStub = defineComponent({ setup(_, { expose }) { expose({ canLeave: () => true, confirmLeave }); return () => null; } });
  const InspectorStub = defineComponent({ template: '<section data-inspector />' });
  const wrapper = mountPage({ props }, { MeshSocialPanel: SocialStub, MeshInspector: InspectorStub });
  await flushPromises();
  await wrapper.findAll('button').find(item => item.text() === 'Contacts')!.trigger('click');
  await flushPromises();
  confirmLeave.mockClear();
  const button = wrapper.findAll('button').find(item => item.text().includes('Mesh Inspector'));
  expect(button).toBeDefined();
  await button!.trigger('click');
  await flushPromises();
  expect(confirmLeave).toHaveBeenCalledOnce();
  expect(wrapper.find('[data-inspector]').exists()).toBe(false);
  wrapper.unmount();
});

it('does no work before the optional page is opened and reports a missing companion honestly', async () => {
  const wrapper = mountPage({ props: { ...props, active: false } });
  await flushPromises(); expect(invoke).not.toHaveBeenCalled();
  vi.mocked(invoke).mockResolvedValue({ available: false, reason: 'Install the managed Mesh companion.', can_host: false });
  await wrapper.setProps({ active: true }); await flushPromises();
  expect(wrapper.text()).toContain('Mesh companion unavailable');
  expect(wrapper.text()).toContain('Install the managed Mesh companion.');
  expect(wrapper.find('form').exists()).toBe(false);
  expect(invoke).toHaveBeenCalledExactlyOnceWith('mesh_overview');
  wrapper.unmount();
});

it('starts a stopped selected game through the launcher', async () => {
  const wrapper = mountPage({props: {...props, games:[{...game, launch_state:'idle'}]}}); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'mesh_discover')).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Start game')!.trigger('click');
  expect(wrapper.emitted('launch')).toEqual([[game.id]]);
  expect(wrapper.find('button[type=submit]').exists()).toBe(false);
  expect(wrapper.text()).not.toContain('Ready');
  expect(wrapper.get('#mesh-game').attributes('role')).toBe('combobox');
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'mesh_host')).toBe(false);
  wrapper.unmount();
});
it('automatically chooses a discovered relay and uses the owned LAN port', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_host_options' ? {...hostOptions,lan:{state:'detected',port:54321}} : command === 'mesh_host' ? {session,invitation:'private-invite'} : {...overview,session:null});
  const wrapper = mountPage({props}); await flushPromises();
  expect(wrapper.text()).toContain('LAN port detected');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_host',{gameId:game.id,localPort:54321,relay:overview.nodes[0]!.endpoint,approveLocalPort:true,routePolicy:{authorization:'coordination',observer:null}});
  await wrapper.findAll('button').find(button => button.text() === 'Copy invitation')!.trigger('click'); await flushPromises();
  expect(writeText).toHaveBeenCalledWith('private-invite');
  expect(wrapper.text()).toContain('Waiting for a guest');
  wrapper.unmount();
});
it('shares a loopback server without requiring a running game', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_host_server' ? {session:{...session,game_id:''},invitation:'server-invite'} : {...overview,session:null});
  const wrapper=mountPage({props:{games:[],initialGameId:'',active:true}}); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Share server')!.trigger('click');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_host_server',{localPort:25565,relay:overview.nodes[0]!.endpoint,approveLocalPort:true,routePolicy:{authorization:'coordination',observer:null}});
  wrapper.unmount();
});
it('joins privately and only displays the returned local address', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_host_options' ? hostOptions : command === 'mesh_join' ? {session:{...session,role:'guest',state:'bridge_ready'},bridge_address:'127.0.0.1:40123'} : {...overview,session:null});
  const wrapper=mountPage({props}); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Join friend')!.trigger('click');
  await wrapper.get('textarea').setValue('secret'); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_join',{gameId:game.id,invitation:'secret',routePolicy:{authorization:'coordination',observer:null}});
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(wrapper.get('input[readonly]').element).toHaveProperty('value','127.0.0.1:40123');
  wrapper.unmount();
});


it('retains the invitation across navigation and stops hidden polling', async () => {
  vi.useFakeTimers(); let current: MeshSession | null = null;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_host_options') return {...hostOptions,lan:{state:'detected',port:54321}};
    if (command === 'mesh_host') { current=session; return {session,invitation:'retained-private'}; }
    if (command === 'mesh_end') { current={...session,state:'ended'}; return {session:current}; }
    return {...overview,session:current};
  });
  const wrapper=mountPage({props}); await flushPromises(); await wrapper.get('form').trigger('submit'); await flushPromises();
  await wrapper.setProps({active:false}); const count=vi.mocked(invoke).mock.calls.length;
  await vi.advanceTimersByTimeAsync(9000); expect(invoke).toHaveBeenCalledTimes(count);
  await wrapper.setProps({active:true}); await flushPromises();
  expect(wrapper.get('textarea[readonly]').element).toHaveProperty('value','retained-private');
  await wrapper.findAll('button').find(button => button.text()==='End session')!.trigger('click'); await flushPromises();
  expect(wrapper.find('textarea[readonly]').exists()).toBe(false);
  wrapper.unmount(); expect(vi.getTimerCount()).toBe(0);
});

it('requires an explicit reconnect after a guest disconnect and reuses only its invitation', async () => {
  vi.useFakeTimers(); let current: MeshSession | null = null;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_host_options') return hostOptions;
    if (command === 'mesh_join') { current={...session,role:'guest',state:'bridge_ready'}; return {session:current,bridge_address:'127.0.0.1:40123'}; }
    return {...overview,session:current};
  });
  const wrapper=mountPage({props}); await flushPromises();
  await wrapper.findAll('button').find(button=>button.text()==='Join friend')!.trigger('click');
  await wrapper.get('textarea').setValue('owned-invitation'); await wrapper.get('form').trigger('submit'); await flushPromises();
  current={...current!,state:'failed'}; await vi.advanceTimersByTimeAsync(3000); await flushPromises();
  expect(wrapper.text()).toContain('Connection lost');
  expect(wrapper.find('input[readonly]').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.filter(([name])=>name==='mesh_join')).toHaveLength(1);
  await wrapper.findAll('button').find(button=>button.text()==='Reconnect')!.trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([name])=>name==='mesh_join')).toHaveLength(2);
  expect(invoke).toHaveBeenLastCalledWith('mesh_join',{gameId:game.id,invitation:'owned-invitation',routePolicy:{authorization:'coordination',observer:null}});
  wrapper.unmount();
});
it('hides an expired host invitation using the companion deadline', async () => {
  vi.mocked(invoke).mockImplementation(async command => command==='mesh_host_options' ? {...hostOptions,lan:{state:'detected',port:54321}} : command==='mesh_host' ? {session:{...session,expires_at:1},invitation:'expired-secret'} : {...overview,session:null});
  const wrapper=mountPage({props}); await flushPromises(); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.text()).toContain('Invitation expired'); expect(wrapper.find('textarea[readonly]').exists()).toBe(false);
  expect(wrapper.findAll('button').some(button=>button.text()==='Copy invitation')).toBe(false); wrapper.unmount();
});

it('asks for a new invitation when the real protocol rejects an invalid or expired invite', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command==='mesh_host_options') return hostOptions;
    if (command==='mesh_join') throw '邀请无效或已过期';
    return {...overview,session:null};
  });
  const wrapper=mountPage({props}); await flushPromises();
  await wrapper.findAll('button').find(button=>button.text()==='Join friend')!.trigger('click');
  await wrapper.get('textarea').setValue('expired'); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.text()).toContain('Ask your friend for a new invitation.');
  expect(wrapper.findAll('button').some(button=>button.text()==='Reconnect')).toBe(false);
  wrapper.unmount();
});

it('opens join directly from a game entry', async () => {
  const wrapper = mountPage({ props: { ...props, initialMode: 'join' } });
  await flushPromises();
  expect(wrapper.find('form textarea').exists()).toBe(true);
  wrapper.unmount();
});

it('opens contacts directly from a titlebar entry', async () => {
  const wrapper = mountPage({ props: { ...props, initialMode: 'contacts' } });
  await flushPromises();
  expect(wrapper.get('.mesh-contacts').attributes('aria-pressed')).toBe('true');
  wrapper.unmount();
});

it('keeps a manually entered LAN port during status refresh', async () => {
  vi.useFakeTimers();
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_host_options' ? { ...hostOptions, lan: { state: 'detected', port: 54321 } } : { ...overview, session: null });
  const wrapper = mountPage({ props });
  await flushPromises();
  await wrapper.get('form input[type="number"]').setValue('54322');
  await vi.advanceTimersByTimeAsync(3000);
  await flushPromises();
  expect((wrapper.get('form input[type="number"]').element as HTMLInputElement).value).toBe('54322');
  wrapper.unmount();
});
