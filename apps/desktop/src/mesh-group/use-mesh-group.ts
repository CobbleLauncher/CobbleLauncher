// SPDX-License-Identifier: MIT
import { computed, onScopeDispose, ref, shallowRef, toValue, watch, type MaybeRefOrGetter } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { MeshHostOptions } from '../mesh';
import { meshProfileLabel, type MeshAppSnapshot, type MeshIdentityState } from '../mesh-social';
import { groupActive, validGroupPort, type MeshGroupEnvelope, type MeshGroupSnapshot } from './types';
import type { GroupTextKey } from './labels';
type ActionResult = MeshGroupEnvelope & { invitation?: string; bridge_address?: string };
type Options = { gameId: MaybeRefOrGetter<string>; running: MaybeRefOrGetter<boolean>; active: MaybeRefOrGetter<boolean> };
const errorKeys: Record<string, GroupTextKey> = { MESH_DISABLED: 'offline', MESH_GROUP_ACTIVE: 'alreadyActive', MESH_GROUP_BUSY: 'busy', MESH_GROUP_NOT_ACTIVE: 'notActive', MESH_GROUP_NOT_HOST: 'notHost', MESH_GROUP_FULL: 'full', MESH_GROUP_ADMISSION_CLOSED: 'admissionClosed', MESH_GROUP_INVALID_INVITATION: 'invalidInvite', MESH_GROUP_GAME_EXITED: 'gameExited', MESH_GROUP_LAN_NOT_OPEN: 'waitingLan', MESH_GROUP_RELAY_REQUIRED: 'relayRequired', MESH_GROUP_INVALID_RELAY: 'relayRequired', MESH_GROUP_UNAVAILABLE: 'unavailable', MESH_GROUP_IO_FAILED: 'unavailable', MESH_GROUP_INVALID_RESPONSE: 'actionFailed', MESH_GROUP_JOIN_DENIED: 'joinDenied', MESH_GROUP_UNKNOWN_MEMBER: 'unknownMember', MESH_GROUP_TIMEOUT: 'timeout' };
const portErrorKeys: Record<string, GroupTextKey> = { MESH_GROUP_INVALID_PORT: 'invalidPort', MESH_GROUP_PORT_NOT_OWNED: 'portNotOwned' };
const problem = (cause: unknown): GroupTextKey => String(cause).includes('MESH_GROUP_DIRECT_REQUIRED') ? 'directRequired' : Object.entries({ ...errorKeys, ...portErrorKeys }).find(([code]) => String(cause).includes(code))?.[1] ?? 'actionFailed';
export function useMeshGroup(options: Options) {
  const group = shallowRef<MeshGroupSnapshot | null>(null), hostOptions = shallowRef<MeshHostOptions | null>(null), names = shallowRef<Record<string, string>>({});
  const selfPeerId = ref(''), invitation = ref(''), address = ref(''), copied = ref(''), loading = ref(false), operation = ref(''), error = ref<GroupTextKey | null>(null);
  const busy = computed(() => !!operation.value);
  let disposed = false, contextRevision = 0, readRevision = 0, generation = -1, eventGeneration = -1, unlisten: UnlistenFn | undefined;
  const gameId = () => toValue(options.gameId);
  const current = (revision: number) => !disposed && revision === contextRevision && toValue(options.active);
  function applyNames(identity: MeshIdentityState | null, snapshot: MeshAppSnapshot | null) {
    const known: Record<string, string> = {}; selfPeerId.value = '';
    if (identity?.profile) { known[identity.profile.peer_id] = meshProfileLabel(identity.profile); selfPeerId.value = identity.profile.peer_id; }
    if (snapshot) { known[snapshot.profile.peer_id] = meshProfileLabel(snapshot.profile); selfPeerId.value = snapshot.profile.peer_id; for (const contact of snapshot.contacts) known[contact.profile.peer_id] = meshProfileLabel(contact.profile); }
    names.value = known;
  }
  async function refreshNames(revision: number) {
    const [identity, snapshot] = await Promise.allSettled([invoke<MeshIdentityState | null>('mesh_identity_state'), invoke<MeshAppSnapshot | null>('mesh_app_snapshot')]);
    if (current(revision)) applyNames(identity.status === 'fulfilled' ? identity.value : null, snapshot.status === 'fulfilled' ? snapshot.value : null);
  }
  function accept(value: MeshGroupEnvelope, source: 'read' | 'event' | 'action'): boolean {
    if (!Number.isSafeInteger(value.generation) || value.generation < generation || source === 'read' && value.generation <= eventGeneration) return false;
    if (value.group && value.group.game_id !== gameId()) { if (source === 'read') error.value = 'otherGame'; return false; }
    if (source === 'event') eventGeneration = Math.max(eventGeneration, value.generation);
    generation = value.generation;
    if (group.value?.group_id !== value.group?.group_id || !groupActive(value.group)) { invitation.value = ''; address.value = ''; copied.value = ''; }
    group.value = value.group;
    address.value = value.group?.bridge_address ?? (value.group?.role === 'guest' ? address.value : '');
    if (!value.group?.admission_open) invitation.value = '';
    return true;
  }
  async function refresh(): Promise<void> {
    if (!toValue(options.active) || !gameId() || busy.value || disposed) return;
    const revision = contextRevision, request = ++readRevision, selected = gameId(); loading.value = true; error.value = null;
    const result = await Promise.allSettled([
      invoke<MeshGroupEnvelope>('mesh_group_snapshot', { gameId: selected }),
      toValue(options.running) ? invoke<MeshHostOptions>('mesh_host_options', { gameId: selected }) : Promise.resolve(null),
      invoke<MeshIdentityState | null>('mesh_identity_state'),
      invoke<MeshAppSnapshot | null>('mesh_app_snapshot'),
    ]);
    if (!current(revision) || request !== readRevision || selected !== gameId()) return;
    if (result[0].status === 'fulfilled') accept(result[0].value, 'read'); else error.value = 'loadFailed';
    if (result[1].status === 'fulfilled') hostOptions.value = result[1].value; else if (!groupActive(group.value)) error.value = 'loadFailed';
    applyNames(result[2].status === 'fulfilled' ? result[2].value : null, result[3].status === 'fulfilled' ? result[3].value : null); loading.value = false;
  }
  async function act(name: string, params: Record<string, unknown>): Promise<void> {
    if (!toValue(options.active) || !gameId() || busy.value || disposed) return;
    const revision = contextRevision; operation.value = name; error.value = null; copied.value = '';
    try {
      const result = await invoke<ActionResult>(name, { gameId: gameId(), ...params });
      if (!current(revision)) return;
      const accepted = accept(result, 'action');
      if (accepted && result.group && groupActive(result.group)) { if (result.invitation) invitation.value = result.invitation; if (result.bridge_address) address.value = result.bridge_address; void refreshNames(revision); }
    } catch (cause) { if (current(revision)) error.value = problem(cause); }
    finally { if (current(revision)) operation.value = ''; }
  }
  function create(manualPort?: number): Promise<void> {
    const lan = hostOptions.value?.lan;
    const port = manualPort ?? (lan?.state === 'detected' ? lan.port : undefined);
    if (!toValue(options.running) || groupActive(group.value) || !validGroupPort(port)) return Promise.resolve();
    return act('mesh_group_create', { localPort: port, approveLocalPort: true, ...(hostOptions.value?.relay.address ? { relay: hostOptions.value.relay.address } : {}) });
  }
  function join(secret: string): Promise<void> {
    if (!toValue(options.running) || groupActive(group.value) || !secret.trim() || secret.length > 8192) return Promise.resolve();
    return act('mesh_group_join', { invitation: secret.trim() });
  }
  function change(name: 'mesh_group_invite' | 'mesh_group_revoke' | 'mesh_group_kick' | 'mesh_group_stop', peerId?: string): Promise<void> {
    const selected = group.value;
    if (!selected || !groupActive(selected) || selected.game_id !== gameId() || name !== 'mesh_group_stop' && selected.role !== 'host') return Promise.resolve();
    if (name === 'mesh_group_kick' && !selected.members.some(member => member.peer_id === peerId && !member.is_host && member.peer_id !== selfPeerId.value)) return Promise.resolve();
    return act(name, { groupId: selected.group_id, ...(peerId ? { peerId } : {}) });
  }
  async function copy(kind: 'invitation' | 'address'): Promise<void> {
    const selected = group.value, revision = contextRevision;
    if (!toValue(options.active) || !selected || !groupActive(selected) || busy.value) return;
    if (kind === 'invitation' && (selected.role !== 'host' || !selected.admission_open)) return;
    if (kind === 'invitation' && selected.invitation_expires_at !== null && selected.invitation_expires_at * 1000 <= Date.now()) { invitation.value = ''; error.value = 'inviteExpired'; return; }
    const value = kind === 'invitation' ? invitation.value : address.value;
    if (!value || kind === 'address' && selected.role !== 'guest') return;
    try { await writeText(value); if (current(revision)) copied.value = kind; }
    catch { if (current(revision)) error.value = 'copyFailed'; }
  }
  async function subscribe(revision: number) {
    try {
      const stop = await listen<MeshGroupEnvelope>('mesh-group-changed', event => { if (current(revision)) accept(event.payload, 'event'); });
      if (current(revision)) unlisten = stop; else stop();
    } catch { if (current(revision)) error.value = 'loadFailed'; }
  }
  watch(() => [toValue(options.gameId), toValue(options.active), toValue(options.running)], () => {
    const revision = ++contextRevision; ++readRevision; unlisten?.(); unlisten = undefined; generation = -1; eventGeneration = -1;
    group.value = null; hostOptions.value = null; invitation.value = ''; address.value = ''; copied.value = ''; error.value = null; operation.value = ''; loading.value = false;
    if (toValue(options.active) && gameId()) { void subscribe(revision); void refresh(); }
  }, { immediate: true, flush: 'sync' });
  onScopeDispose(() => { disposed = true; ++contextRevision; ++readRevision; unlisten?.(); invitation.value = ''; address.value = ''; });
  return { group, hostOptions, names, selfPeerId, invitation, address, copied, loading, busy, error, refresh, create, join, copy, invite: () => change('mesh_group_invite'), revoke: () => change('mesh_group_revoke'), kick: (peerId: string) => change('mesh_group_kick', peerId), stop: () => change('mesh_group_stop') };
}
