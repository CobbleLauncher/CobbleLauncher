// SPDX-License-Identifier: MIT
// Synthetic native boundary: component behavior only; native Mesh transport is tested separately.
import { DOMWrapper, flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import MeshSocialPanel from './MeshSocialPanel.vue';
import { setLocale } from './i18n';
import type { MeshAppSnapshot, MeshMessagePage } from './mesh-social';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));

const localId = 'local-public-peer-id';
const snapshot: MeshAppSnapshot = {
  profile: { peer_id: localId, display_name: 'My Mesh', suffix: '00112233', revision: 1 },
  contacts: [
    { profile: { peer_id: 'peer-ada', display_name: 'Ada', suffix: '11223344', revision: 1 }, connected: true, last_seen: 1 },
    { profile: { peer_id: 'peer-ben', display_name: 'Ben', suffix: '55667788', revision: 1 }, connected: true, last_seen: 1 },
  ],
  capabilities: { contact_invites: true, direct_messages: true, lan_only: true, relay_messages: false, groups: false, vlan: false },
};
const emptyPage: MeshMessagePage = { messages: [], next_after: 0, oldest_sequence: null, has_more: false, generation: 'runtime-a' };
const wrappers: VueWrapper[] = [];
const mountPanel = (props = {}) => {
  const wrapper = mount(MeshSocialPanel, { props: { active: true, ...props } });
  wrappers.push(wrapper);
  return wrapper;
};
const button = (wrapper: VueWrapper, label: string) => wrapper.findAll('button').find(item => item.attributes('aria-label') === label || item.text() === label)!;
const field = (wrapper: VueWrapper, label: string) => {
  const direct = wrapper.find(`[aria-label="${label}"]`);
  if (direct.exists()) return direct;
  const matching = wrapper.findAll('label').find(item => item.text().trim() === label || item.text().trim().startsWith(label));
  const control = matching?.find('textarea,input,select');
  if (control?.exists()) return control;
  const id = matching?.attributes('for');
  if (id) return wrapper.get(`[id="${id}"]`);
  throw new Error(`No labelled control: ${label}`);
};
const chooseContact = async (wrapper: VueWrapper, name: string) => {
  const contact = wrapper.findAll('button').find(item => item.text().includes(`${name}#`));
  expect(contact, `Contact ${name} should be available`).toBeDefined();
  await contact!.trigger('click');
  await flushPromises();
};
function running(command: string) {
  if (command === 'mesh_app_snapshot') return snapshot;
  if (command === 'mesh_list_messages') return emptyPage;
  if (command === 'mesh_list_message_composers') return [{ plugin_id: 'fixture-composer', name: 'Fixture composer' }];
  if (command === 'mesh_create_contact_invitation') return { invitation: 'private-contact-invitation', expires_at: Math.floor(Date.now() / 1000) + 600 };
  if (command === 'mesh_cancel_message_composer') return true;
  return undefined;
}
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(writeText).mockReset(); vi.mocked(invoke).mockImplementation(async command => running(command)); });
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); vi.useRealTimers(); vi.clearAllMocks(); setLocale('zh-CN'); });

it('starts with Mesh off and starts the local runtime only after the explicit action', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_app_snapshot' ? null : command === 'mesh_start_app' ? snapshot : running(command));
  const wrapper = mountPanel({ active: false }); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_start_app')).toBe(false);
  await wrapper.setProps({ active: true }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_app_snapshot');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_start_app')).toBe(false);
  await field(wrapper, 'Mesh name').setValue('Explicit local name');
  await wrapper.get('form.mesh-start').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_start_app', { displayName: 'Explicit local name' });
});

it('copies the public Mesh ID rather than the private contact invitation', async () => {
  const wrapper = mountPanel(); await flushPromises();
  await button(wrapper, 'Add contact').trigger('click');
  await button(wrapper, 'Create contact invitation').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_create_contact_invitation');
  await button(wrapper, 'Copy Mesh ID').trigger('click'); await flushPromises();
  expect(writeText).toHaveBeenCalledWith(localId);
  expect(writeText).not.toHaveBeenCalledWith('private-contact-invitation');
});

it('shows the public-message pause only when the native snapshot explicitly says it is paused', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_app_snapshot' ? { ...snapshot, capabilities: { ...snapshot.capabilities, lan_only: false, events_public: true }, public_events_paused: true } : running(command));
  const paused = mountPanel(); await flushPromises();
  expect(paused.get('[role="status"]').text()).toBe('Public messages are paused during this game session.');
  expect(paused.text()).not.toContain('Local network');
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_app_snapshot' ? { ...snapshot, capabilities: { ...snapshot.capabilities, lan_only: false, events_public: true }, public_events_paused: false } : running(command));
  const available = mountPanel(); await flushPromises();
  expect(available.text()).not.toContain('Public messages are paused during this game session.');
});

it('maps a game-session public-route conflict to a brief retry action without exposing its code', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_create_contact_invitation') throw 'MESH_EVENT_ROUTE_GAME_ACTIVE';
    return running(command);
  });
  const wrapper = mountPanel(); await flushPromises();
  await button(wrapper, 'Add contact').trigger('click');
  await button(wrapper, 'Create contact invitation').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toBe('End the current game connection, then try again.');
  expect(wrapper.text()).not.toContain('MESH_EVENT_ROUTE_GAME_ACTIVE');
});

it('maps a starting public route to a brief retry message without exposing its code', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_create_contact_invitation') throw 'MESH_EVENT_ROUTE_NOT_READY';
    return running(command);
  });
  const wrapper = mountPanel(); await flushPromises();
  await button(wrapper, 'Add contact').trigger('click');
  await button(wrapper, 'Create contact invitation').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toBe('The connection is getting ready. Try again shortly.');
  expect(wrapper.text()).not.toContain('MESH_EVENT_ROUTE_NOT_READY');
});

it('opens the exact requested contact after the current contact list loads', async () => {
  const wrapper = mountPanel({ initialContactId: 'peer-ben', contactRequestId: 4 });
  await flushPromises();
  expect(wrapper.get('.mesh-contact[aria-pressed="true"]').text()).toContain('Ben#55667788');
  expect(wrapper.emitted('contactOpened')).toEqual([[4]]);
  expect(invoke).toHaveBeenCalledWith('mesh_list_messages', { after: 0, limit: 40 });
});

it('keeps separate contact drafts and preserves the selected draft when sending fails', async () => {
  vi.mocked(invoke).mockImplementation(async command => { if (command === 'mesh_send_message') throw new Error('Remote acknowledgement unavailable'); return running(command); });
  const wrapper = mountPanel(); await flushPromises();
  await chooseContact(wrapper, 'Ada');
  await field(wrapper, 'Message').setValue('Ada draft remains private until sent');
  await chooseContact(wrapper, 'Ben');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('');
  await field(wrapper, 'Message').setValue('Ben separate draft');
  await chooseContact(wrapper, 'Ada');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Ada draft remains private until sent');
  await wrapper.get('form.mesh-message-form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_send_message', { peerId: 'peer-ada', text: 'Ada draft remains private until sent' });
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Ada draft remains private until sent');
  expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  expect(wrapper.text()).not.toContain('Delivered');
  expect(wrapper.find('.mesh-message-form [role="status"]').exists()).toBe(false);
  expect(wrapper.findAll('.mesh-messages .outgoing')).toHaveLength(0);
  await chooseContact(wrapper, 'Ben');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Ben separate draft');
});

it('advances the global inbox cursor across other contacts and clears old runtime history on generation reset', async () => {
  vi.useFakeTimers();
  let generationReset = false;
  const inboxArgs: number[] = [];
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command !== 'mesh_list_messages') return running(command);
    const after = (args as { after: number }).after; inboxArgs.push(after);
    if (!generationReset) return { ...emptyPage, messages: after > 0 ? [] : [{ sequence: 1, id: 'old-a', peer_id: 'peer-ada', direction: 'incoming', text: 'Old runtime history', at: 1 }, { sequence: 3, id: 'other-b', peer_id: 'peer-ben', direction: 'incoming', text: 'Other contact frame', at: 1 }], next_after: 3, oldest_sequence: 1 };
    return after > 0 ? { ...emptyPage, generation: 'runtime-b', next_after: 1 } : { ...emptyPage, generation: 'runtime-b', messages: [{ sequence: 1, id: 'new-a', peer_id: 'peer-ada', direction: 'incoming', text: 'New runtime history', at: 2 }], next_after: 1, oldest_sequence: 1 };
  });
  const wrapper = mountPanel(); await flushPromises();
  await chooseContact(wrapper, 'Ada');
  expect(wrapper.text()).toContain('Old runtime history');
  expect(wrapper.text()).not.toContain('Other contact frame');
  await field(wrapper, 'Message').setValue('Unsent across runtime restart');
  generationReset = true;
  await vi.advanceTimersByTimeAsync(3000); await flushPromises();
  expect(inboxArgs).toContain(3);
  expect(inboxArgs.slice(-2)).toEqual([3, 0]);
  expect(wrapper.text()).not.toContain('Old runtime history');
  expect(wrapper.text()).toContain('New runtime history');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Unsent across runtime restart');
  await vi.advanceTimersByTimeAsync(3000); await flushPromises();
  expect(inboxArgs.at(-1)).toBe(1);
});

it('previews an extension draft without replacing the message or sending it until both explicit actions', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_compose_message' ? { text: 'Extension proposed message' } : command === 'mesh_send_message' ? { id: 'ack-message', received: true } : running(command));
  const wrapper = mountPanel({ gameId: 'owned-game' }); await flushPromises();
  await chooseContact(wrapper, 'Ada');
  await field(wrapper, 'Message').setValue('Existing unsent message');
  await button(wrapper, 'Draft with an extension').trigger('click');
  await field(wrapper, 'Extension').setValue('fixture-composer');
  await field(wrapper, 'Instruction').setValue('Invite Ada to build');
  await button(wrapper, 'Generate draft').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_compose_message', expect.objectContaining({ pluginId: 'fixture-composer', peerId: 'peer-ada', gameId: 'owned-game', note: 'Invite Ada to build' }));
  expect(wrapper.get('.mesh-extension-draft').text()).toContain('Extension proposed message');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Existing unsent message');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_send_message')).toBe(false);
  await button(wrapper, 'Use draft').trigger('click');
  expect((field(wrapper, 'Message').element as HTMLTextAreaElement).value).toBe('Extension proposed message');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_send_message')).toBe(false);
  await wrapper.get('form.mesh-message-form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_send_message', { peerId: 'peer-ada', text: 'Extension proposed message' });
});

it('removes a contact only after the explicit confirmation, and cancellation changes nothing', async () => {
  let current = snapshot;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'mesh_app_snapshot') return current;
    if (command === 'mesh_remove_contact') { current = { ...snapshot, contacts: snapshot.contacts.filter(contact => contact.profile.peer_id !== 'peer-ada') }; return current; }
    return running(command);
  });
  const wrapper = mount(MeshSocialPanel, { props: { active: true }, attachTo: document.body }); wrappers.push(wrapper);
  await flushPromises(); await chooseContact(wrapper, 'Ada');
  await button(wrapper, 'Remove contact').trigger('click'); await flushPromises();
  const firstDialog = new DOMWrapper(document.body.querySelector('[role="dialog"]')!);
  expect(firstDialog.text()).toContain('Remove and disconnect this contact?');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_remove_contact')).toBe(false);
  await firstDialog.findAll('button').find(control => control.text() === 'Cancel')!.trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'mesh_remove_contact')).toBe(false);
  expect(wrapper.findAll('.mesh-contact').some(contact => contact.text().includes('Ada#'))).toBe(true);
  await button(wrapper, 'Remove contact').trigger('click'); await flushPromises();
  const confirmation = new DOMWrapper(document.body.querySelector('[role="dialog"]')!);
  await confirmation.findAll('button').find(control => control.text() === 'Remove contact')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_remove_contact', { peerId: 'peer-ada' });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'mesh_remove_contact')).toHaveLength(1);
  expect(wrapper.findAll('.mesh-contact').some(contact => contact.text().includes('Ada#'))).toBe(false);
  expect(wrapper.findAll('.mesh-contact').some(contact => contact.text().includes('Ben#'))).toBe(true);
});

it('shows the saved identity while contact networking is stopped without starting it', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_identity_state' ? { profile: snapshot.profile, mode: 'persistent', active: false } : command === 'mesh_app_snapshot' ? null : running(command));
  const wrapper = mountPanel({ showContacts: false }); await flushPromises();
  expect(wrapper.get('.mesh-identity-card').text()).toContain('My Mesh identity');
  expect(wrapper.get('.mesh-identity-card').text()).toContain('Saved on this device');
  expect(wrapper.find('.mesh-contacts-layout').exists()).toBe(false);
  await button(wrapper, 'Copy identity card').trigger('click');
  expect(JSON.parse(vi.mocked(writeText).mock.calls[0]![0])).toMatchObject({ peerId: localId, displayName: 'My Mesh' });
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'mesh_start_app')).toBe(false);
});
it('changes contact identity only after the explicit lifecycle confirmation', async () => {
  const temporary = { ...snapshot, profile: { ...snapshot.profile, peer_id: 'temporary-full-peer-id' }, contacts: [] };
  vi.mocked(invoke).mockImplementation(async command => command === 'mesh_identity_state' ? { profile: snapshot.profile, mode: 'persistent', active: true } : command === 'mesh_set_identity_mode' ? temporary : running(command));
  const wrapper = mountPanel({ showContacts: false }); await flushPromises();
  await button(wrapper, 'Use temporary identity').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([name]) => name === 'mesh_set_identity_mode')).toBe(false);
  const confirm = new DOMWrapper(document.body).findAll('button').find(item => item.text() === 'Use temporary identity');
  await confirm!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('mesh_set_identity_mode', { temporary: true, displayName: 'My Mesh' });
  expect(wrapper.get('.mesh-identity-card').text()).toContain('Temporary contact identity');
  await button(wrapper, 'Copy Mesh ID').trigger('click');
  expect(writeText).toHaveBeenCalledWith('temporary-full-peer-id');
});
