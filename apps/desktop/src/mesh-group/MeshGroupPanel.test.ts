// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import MeshGroupPanel from './MeshGroupPanel.vue';
import { groupFreeSlots, type MeshGroupEnvelope, type MeshGroupSnapshot } from './types';
import { setLocale } from '../i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
const host = 'peer-host-actual', guest = 'peer-guest-actual';
const profile = { peer_id: host, display_name: 'Dylan', suffix: '1234', revision: 1 };
const hostOptions = { game_id: 'game-a', lan: { state: 'detected', port: 45678 }, relay: { address: '/dns4/relay.example.invalid/tcp/443/wss', source: 'last_successful', state: 'configured' } };
const hostGroup = (): MeshGroupSnapshot => ({ group_id: 12, game_id: 'game-a', game_name: 'Game A', role: 'host', state: 'hosting', host_peer_id: host, capacity: 3, members: [{ peer_id: host, state: 'connected', is_host: true }], membership_complete: true, invitation_expires_at: Math.floor(Date.now() / 1000) + 300, admission_open: true, expires_at: Math.floor(Date.now() / 1000) + 28_800, route: 'explicit_relay' });
let value: MeshGroupEnvelope, event: ((value: { payload: MeshGroupEnvelope }) => void) | undefined;
const unlisten = vi.fn(), wrappers: VueWrapper[] = [];
function mountPanel(props = {}) { const wrapper = mount(MeshGroupPanel, { props: { gameId: 'game-a', gameName: 'Game A', running: true, active: true, ...props }, attachTo: document.body }); wrappers.push(wrapper); return wrapper; }
function button(wrapper: VueWrapper, label: string) { return wrapper.findAll('button').find(button => button.text() === label)!; }
it('prefills a prepared invitation after explicit game launch and never auto joins', async () => {
  const wrapper = mountPanel({gameId:'prepared-game',gameName:'Prepared game',running:false,initialInvitation:'prepared-secret'});
  await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='mesh_group_join')).toBe(false);
  await wrapper.setProps({running:true}); await flushPromises();
  expect(wrapper.get('#mesh-group-invitation').element).toHaveProperty('value','prepared-secret');
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='mesh_group_join')).toBe(false);
  await button(wrapper,'Join group').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_join',{gameId:'prepared-game',invitation:'prepared-secret'});
});
async function menu(label: string) { await vi.waitFor(() => expect([...document.querySelectorAll('[role=menuitem]')].some(item => item.textContent?.includes(label))).toBe(true)); [...document.querySelectorAll<HTMLElement>('[role=menuitem]')].find(item => item.textContent?.includes(label))!.click(); await flushPromises(); }
beforeEach(() => {
  setLocale('en-US'); value = { generation: 1, group: null }; vi.mocked(invoke).mockReset(); vi.mocked(listen).mockReset(); vi.mocked(writeText).mockReset().mockResolvedValue(); unlisten.mockReset();
  vi.mocked(listen).mockImplementation(async (_name, handler) => { event = handler as typeof event; return unlisten; });
  vi.mocked(invoke).mockImplementation(async (command, params) => {
    if (command === 'mesh_group_snapshot') return value;
    if (command === 'mesh_host_options') return hostOptions;
    if (command === 'mesh_identity_state') return { profile, mode: 'persistent', active: true };
    if (command === 'mesh_app_snapshot') return { profile, contacts: [{ profile: { ...profile, peer_id: guest, display_name: 'River', suffix: '5678' } }], capabilities: {} };
    if (command === 'mesh_group_create') { value = { generation: 2, group: hostGroup() }; return { ...value, invitation: 'synthetic-private-invitation' }; }
    if (command === 'mesh_group_kick') { value = { generation: value.generation + 1, group: { ...value.group!, members: value.group!.members.filter(member => member.peer_id !== (params as { peerId: string }).peerId) } }; return value; }
    if (command === 'mesh_group_revoke') { value = { generation: value.generation + 1, group: { ...value.group!, admission_open: false } }; return value; }
    if (command === 'mesh_group_stop') { value = { generation: value.generation + 1, group: { ...value.group!, state: 'ended' } }; return value; }
    throw Error('Unexpected command ' + command);
  });
});
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); event = undefined; document.body.innerHTML = ''; vi.restoreAllMocks(); setLocale('zh-CN'); });
it('creates only from the actual detected game LAN port, copies the returned invitation and keeps group alive on unmount', async () => {
  const wrapper = mountPanel(); await flushPromises();
  expect(wrapper.classes()).toContain('page-content'); expect(wrapper.text()).not.toContain('Back');
  await button(wrapper, 'Create group').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_create', { gameId: 'game-a', localPort: 45678, approveLocalPort: true, relay: hostOptions.relay.address });
  expect(wrapper.text()).toContain('Hosted by Dylan#1234'); expect(wrapper.text()).toContain('2 open slots');
  await button(wrapper, 'Copy invitation').trigger('click'); await flushPromises(); expect(writeText).toHaveBeenCalledWith('synthetic-private-invitation');
  wrapper.unmount(); expect(unlisten).toHaveBeenCalled(); expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_group_stop')).toBe(false);
});
it('updates only from actual membership events, ignores older events and supports contextual kick and revoke', async () => {
  value = { generation: 2, group: hostGroup() }; const wrapper = mountPanel(); await flushPromises();
  value = { generation: 3, group: { ...value.group!, members: [...value.group!.members, { peer_id: guest, state: 'connected', is_host: false }] } }; event?.({ payload: value }); await flushPromises();
  expect(wrapper.text()).toContain('River#5678'); expect(wrapper.text()).toContain('1 open slot');
  event?.({ payload: { generation: 1, group: null } }); await flushPromises(); expect(wrapper.text()).toContain('River#5678');
  await wrapper.get('[aria-label="Actions for River#5678"]').trigger('click'); await menu('Remove from group');
  expect(invoke).toHaveBeenCalledWith('mesh_group_kick', { gameId: 'game-a', groupId: 12, peerId: guest }); expect(wrapper.text()).not.toContain('River#5678');
  await wrapper.get('[aria-label="Group actions"]').trigger('click'); await menu('Revoke invitation'); expect(wrapper.text()).toContain('New members cannot join.');
});
it('joins from a private invitation without inventing a complete guest roster or free slots', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_group_join') return { generation: 2, group: { ...hostGroup(), role: 'guest', state: 'bridge_ready', membership_complete: false, bridge_address: '127.0.0.1:34567', members: [{ peer_id: host, state: 'connected', is_host: true }, { peer_id: guest, state: 'connected', is_host: false }] }, bridge_address: '127.0.0.1:34567' };
    if (command === 'mesh_group_snapshot') return value; if (command === 'mesh_host_options') return hostOptions; return null;
  });
  const wrapper = mountPanel(); await flushPromises(); await button(wrapper, 'Join a friend').trigger('click'); await wrapper.get('textarea').setValue('  synthetic-group-invite  '); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_join', { gameId: 'game-a', invitation: 'synthetic-group-invite' }); expect(wrapper.text()).toContain('Visible connections'); expect(wrapper.text()).not.toContain('open slots'); expect(wrapper.text()).not.toContain('Group full');
  await button(wrapper, 'Copy address').trigger('click'); await flushPromises(); expect(writeText).toHaveBeenCalledWith('127.0.0.1:34567'); expect(wrapper.find('[aria-label="Group actions"]').exists()).toBe(false);
});
it('does not infer guest capacity, accepts unknown names as Mesh IDs and hides host-only actions', () => {
  const guestGroup = { ...hostGroup(), role: 'guest' as const, membership_complete: false }; expect(groupFreeSlots(guestGroup)).toBeNull();
});
it('keeps LAN creation disabled when undetected and exposes start-game instead of guessing a port', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_group_snapshot' ? value : command === 'mesh_host_options' ? { ...hostOptions, lan: { state: 'not_open', port: null } } : null);
  const wrapper = mountPanel(); await flushPromises(); expect(button(wrapper, 'Create group').attributes('disabled')).toBeDefined();
  await wrapper.setProps({ running: false }); await flushPromises(); await button(wrapper, 'Start game').trigger('click'); expect(wrapper.emitted('launch')).toEqual([['game-a']]); expect(wrapper.text()).not.toContain('25565');
});
it('clears invitation text when inactive and suppresses late results for another game', async () => {
  let finish!: (value: MeshGroupEnvelope & { invitation: string }) => void;
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, params) => command === 'mesh_group_create' ? new Promise(resolve => { finish = resolve; }) : original(command, params));
  const wrapper = mountPanel(); await flushPromises(); await button(wrapper, 'Create group').trigger('click'); await wrapper.setProps({ gameId: 'game-b', gameName: 'Game B' }); await flushPromises(); finish({ generation: 5, group: hostGroup(), invitation: 'must-not-copy' }); await flushPromises();
  expect(wrapper.text()).not.toContain('Hosted by'); expect(wrapper.text()).not.toContain('must-not-copy'); expect(writeText).not.toHaveBeenCalled();
  await button(wrapper, 'Join a friend').trigger('click'); await wrapper.get('textarea').setValue('private-draft'); await wrapper.setProps({ active: false }); await wrapper.setProps({ active: true }); await flushPromises(); expect(wrapper.find('textarea').exists()).toBe(false);
});
it('shows a bounded friendly join-denied error rather than guessing full or expired', async () => {
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, params) => command === 'mesh_group_join' ? Promise.reject(Error('MESH_GROUP_JOIN_DENIED')) : original(command, params));
  const wrapper = mountPanel(); await flushPromises(); await button(wrapper, 'Join a friend').trigger('click'); await wrapper.get('textarea').setValue('synthetic-invite'); await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('Ask the host to check access.'); expect(wrapper.get('[role=alert]').text()).not.toMatch(/full|expired/); expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('');
});
it('allows native relay selection when this game has no saved relay', async () => {
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, params) => command === 'mesh_host_options' ? Promise.resolve({ ...hostOptions, relay: { address: null, source: null, state: 'unconfigured' } }) : original(command, params));
  const wrapper = mountPanel(); await flushPromises(); await button(wrapper, 'Create group').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_create', { gameId: 'game-a', localPort: 45678, approveLocalPort: true });
  expect(wrapper.text()).toContain('Hosted by Dylan#1234');
});
it('keeps an observed membership event when a same-generation cached read completes later', async () => {
  let finish!: (value: MeshGroupEnvelope) => void;
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, params) => command === 'mesh_group_snapshot' ? new Promise(resolve => { finish = resolve; }) : original(command, params));
  const wrapper = mountPanel(); await flushPromises();
  event?.({ payload: { generation: 3, group: { ...hostGroup(), members: [...hostGroup().members, { peer_id: guest, state: 'connected', is_host: false }] } } });
  finish({ generation: 3, group: hostGroup() }); await flushPromises();
  expect(wrapper.text()).toContain('River#5678'); expect(wrapper.text()).toContain('1 open slot');
});
it('prefills the collapsed manual port from detection and forwards an explicit edit', async () => {
  const wrapper = mountPanel(); await flushPromises();
  const disclosure = wrapper.findAll('button').find(button => button.text().startsWith('Enter port'))!; expect(disclosure.attributes('aria-expanded')).toBe('false');
  await disclosure.trigger('click'); expect((wrapper.get('#mesh-group-port').element as HTMLInputElement).value).toBe('45678');
  await wrapper.get('#mesh-group-port').setValue('46789'); await button(wrapper, 'Create group').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_create', { gameId: 'game-a', localPort: 46789, approveLocalPort: true, relay: hostOptions.relay.address });
});
it('accepts a valid manual port without detection, rejects invalid input and reports ownership failures honestly', async () => {
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, params) => command === 'mesh_host_options' ? Promise.resolve({ ...hostOptions, lan: { state: 'unverified', port: null } }) : command === 'mesh_group_create' ? Promise.reject(Error('MESH_GROUP_PORT_NOT_OWNED')) : original(command, params));
  const wrapper = mountPanel(); await flushPromises(); await button(wrapper, 'Enter port').trigger('click');
  for (const invalid of ['1023', '65536', '30000.5', '']) { await wrapper.get('#mesh-group-port').setValue(invalid); expect(button(wrapper, 'Create group').attributes('disabled')).toBeDefined(); }
  await wrapper.get('#mesh-group-port').setValue('51234'); expect(button(wrapper, 'Create group').attributes('disabled')).toBeUndefined();
  await button(wrapper, 'Create group').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_group_create', { gameId: 'game-a', localPort: 51234, approveLocalPort: true, relay: hostOptions.relay.address });
  expect(wrapper.get('[role=alert]').text()).toContain('This game is not listening on that port.'); expect(wrapper.text()).not.toContain('Hosted by');
});
