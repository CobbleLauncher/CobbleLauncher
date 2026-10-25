<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui';
import type { Game } from './games';
import { locale, t } from './i18n';
import { pluginLocale } from './extensions/contributions';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import MeshIdentityCard from './mesh-ui/MeshIdentityCard.vue';
import MeshIdentityAvatar from './mesh-ui/MeshIdentityAvatar.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { meshIdentityCard, type MeshIdentityState, meshProfileLabel, meshMessageBytes, MAX_MESH_MESSAGE_BYTES, type MeshAppSnapshot, type MeshPublicProfile, type MeshContact, type MeshContactInvitation, type MeshMessageReceipt, type MeshMessage, type MeshMessagePage, type MeshMessageComposer } from './mesh-social';
const props = withDefaults(defineProps<{ active: boolean; showContacts?: boolean; gameId?: string; games?: Game[]; initialContactId?: string; contactRequestId?: number }>(), { showContacts: true, gameId: undefined, games: () => [], initialContactId: undefined, contactRequestId: undefined });
const emit = defineEmits<{ busy: [value: boolean]; contactOpened: [requestId: number] }>();
const identityState = ref<MeshIdentityState>({ profile: null, mode: 'persistent', active: false });
const identityProfile = computed(() => snapshot.value?.profile ?? identityState.value.profile);
const identitySwitchPrompt = ref(false);
const snapshot = ref<MeshAppSnapshot | null>(null); const checked = ref(false); const checking = ref(false); const error = ref('');
const operation = ref(''); const busy = computed(() => Boolean(operation.value));
const startName = ref(''); const nameDraft = ref(''); const editingName = ref(false); const copied = ref('');
const query = ref(''); const selectedPeer = ref(''); const drafts = ref<Record<string, string>>({});
let handledContactRequest = 0;
const invitation = ref<MeshContactInvitation | null>(null); const invitationInput = ref('');
const messages = ref<MeshMessage[]>([]); const cursor = ref(0); const inboxGeneration = ref(''); const hasMore = ref(false); const readingMessages = ref(false); const receipt = ref<MeshMessageReceipt | null>(null);
const contextGame = ref(props.gameId ?? '');
const composers = ref<MeshMessageComposer[]>([]); const composerId = ref(''); const note = ref(''); const suggestion = ref<{ peerId: string; text: string } | null>(null);
let disposed = false; let version = 0; let messageRequest = 0; let composeVersion = 0; let cancelRequested = false; let timer: ReturnType<typeof setInterval> | undefined;
const contacts = computed(() => snapshot.value?.contacts ?? []);
const filteredContacts = computed(() => contacts.value.filter(contact => `${meshProfileLabel(contact.profile)} ${contact.profile.peer_id}`.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())));
const selected = computed(() => contacts.value.find(contact => contact.profile.peer_id === selectedPeer.value));
const currentMessages = computed(() => messages.value.filter(message => message.peer_id === selectedPeer.value));
const draft = computed({ get: () => drafts.value[selectedPeer.value] ?? '', set: (value: string) => { drafts.value = { ...drafts.value, [selectedPeer.value]: value }; receipt.value = null; } });
const nameDirty = computed(() => Boolean(snapshot.value && nameDraft.value !== snapshot.value.profile.display_name));
const messageDirty = computed(() => Object.values(drafts.value).some(value => value.length > 0));
const nameValid = computed(() => Boolean(nameDraft.value.trim()) && validName(nameDraft.value));
const publicRouteErrorKeys: Record<string, string> = {
  MESH_EVENT_ROUTE_NOT_READY: 'meshSocial.eventRouteNotReady',
  MESH_EVENT_ROUTE_GAME_ACTIVE: 'meshSocial.eventRouteGameActive',
  MESH_EVENT_ROUTE_ACTIVE: 'meshSocial.eventRouteActive',
  MESH_EVENT_ROUTE_NOT_AUTHORIZED: 'meshSocial.eventRouteNotAuthorized',
  MESH_EVENT_DIRECT_REQUIRED: 'meshSocial.eventDirectRequired',
  MESH_EVENT_ROUTE_FAILED: 'meshSocial.eventRouteFailed',
};
function contactError(cause: unknown, fallback: string) {
  const code = typeof cause === 'string' ? cause : cause instanceof Error ? cause.message : '';
  return t(publicRouteErrorKeys[code] ?? fallback);
}
function validName(value: string) { return [...value.trim()].length <= 32 && meshMessageBytes(value.trim()) <= 128 && ![...value].some(character => character.codePointAt(0)! < 32 || character.codePointAt(0) === 127); }
const startValid = computed(() => Boolean(startName.value.trim()) && validName(startName.value));
const messageTooLong = computed(() => meshMessageBytes(draft.value) > MAX_MESH_MESSAGE_BYTES);
const canSend = computed(() => Boolean(selected.value && snapshot.value?.capabilities.direct_messages && draft.value.trim() && !messageTooLong.value && !busy.value));
const removePrompt = ref(false);
const leavePrompt = ref(false); let leaveResolver: ((value: boolean) => void) | null = null;
watch(busy, value => emit('busy', value), { flush: 'sync' });
function applySnapshot(value: MeshAppSnapshot | null) {
  const previous = snapshot.value; const dirty = nameDirty.value;
  snapshot.value = value;
  identityState.value = { ...identityState.value, profile: value?.profile ?? identityState.value.profile, active: Boolean(value) };
  if (value && (!previous || previous.profile.peer_id !== value.profile.peer_id || !dirty || !editingName.value)) nameDraft.value = value.profile.display_name;
  if (!value) { messages.value = []; cursor.value = 0; inboxGeneration.value = ''; selectedPeer.value = ''; receipt.value = null; composers.value = []; }
}
async function refreshSnapshot() {
  if (!props.active || checking.value || busy.value) return;
  const current = version; checking.value = true;
  try { const [value, identity] = await Promise.all([invoke<MeshAppSnapshot | null>('mesh_app_snapshot'), invoke<MeshIdentityState | null>('mesh_identity_state')]); if (!disposed && current === version && props.active) { if (identity) identityState.value = identity; applySnapshot(value); if (!value && identity?.profile) startName.value = identity.profile.display_name; checked.value = true; } }
  catch { if (!disposed && current === version) error.value = t('meshSocial.loadFailed'); }
  finally { if (!disposed && current === version) checking.value = false; }
}
async function start() {
  if (!startValid.value || busy.value) return;
  const current = version; operation.value = 'start'; error.value = '';
  try { const value = await invoke<MeshAppSnapshot>('mesh_start_app', { displayName: startName.value.trim() }); if (!disposed && current === version) { applySnapshot(value); checked.value = true; } }
  catch { if (!disposed && current === version) error.value = t('meshSocial.startFailed'); }
  finally { if (!disposed) operation.value = ''; }
}
async function saveName() {
  if (!nameValid.value || busy.value || !snapshot.value) return;
  const current = version; operation.value = 'rename'; error.value = '';
  try { const profile = await invoke<MeshPublicProfile>('mesh_set_display_name', { displayName: nameDraft.value.trim() }); if (!disposed && current === version && snapshot.value) { snapshot.value = { ...snapshot.value, profile }; identityState.value = { ...identityState.value, profile }; nameDraft.value = profile.display_name; editingName.value = false; } }
  catch { if (!disposed && current === version) error.value = t('meshSocial.nameFailed'); }
  finally { if (!disposed) operation.value = ''; }
}
async function switchIdentity() {
  if (busy.value || !await confirmLeave()) return;
  identitySwitchPrompt.value = false;
  const current = version; operation.value = 'identity'; error.value = '';
  const temporary = identityState.value.mode !== 'temporary';
  try {
    const value = await invoke<MeshAppSnapshot>('mesh_set_identity_mode', { temporary, displayName: identityProfile.value?.display_name ?? startName.value.trim() });
    if (!disposed && current === version) {
      identityState.value = { profile: value.profile, mode: temporary ? 'temporary' : 'persistent', active: true };
      drafts.value = {}; suggestion.value = null; invitation.value = null; messages.value = []; cursor.value = 0; inboxGeneration.value = ''; selectedPeer.value = '';
      applySnapshot(value); editingName.value = false; copied.value = '';
    }
  } catch { if (!disposed && current === version) { operation.value = ''; await refreshSnapshot(); error.value = t('meshSocial.identitySwitchFailed'); } }
  finally { if (!disposed) operation.value = ''; }
}
function copyIdentity(kind: 'id' | 'card') { if (identityProfile.value) void copy(kind === 'id' ? identityProfile.value.peer_id : meshIdentityCard(identityProfile.value), kind); }
function cancelName() { nameDraft.value = snapshot.value?.profile.display_name ?? ''; editingName.value = false; }
async function copy(value: string, kind: string) { try { await writeText(value); copied.value = kind; } catch { error.value = t('mesh.copyError'); } }
async function createInvitation() {
  if (busy.value || !snapshot.value?.capabilities.contact_invites) return;
  const current = version; operation.value = 'invite'; error.value = ''; invitation.value = null;
  try { const value = await invoke<MeshContactInvitation>('mesh_create_contact_invitation'); if (!disposed && current === version) invitation.value = value; }
  catch (cause) { if (!disposed && current === version) error.value = contactError(cause, 'meshSocial.inviteFailed'); }
  finally { if (!disposed) operation.value = ''; }
}
async function acceptInvitation() {
  if (busy.value || !invitationInput.value.trim() || !snapshot.value?.capabilities.contact_invites) return;
  const current = version; const secret = invitationInput.value.trim(); invitationInput.value = ''; operation.value = 'accept'; error.value = '';
  try { const contact = await invoke<MeshContact>('mesh_accept_contact_invitation', { invitation: secret }); if (!disposed && current === version && snapshot.value) { snapshot.value = { ...snapshot.value, contacts: [...snapshot.value.contacts.filter(item => item.profile.peer_id !== contact.profile.peer_id), contact] }; selectedPeer.value = contact.profile.peer_id; } }
  catch (cause) { if (!disposed && current === version) error.value = contactError(cause, 'meshSocial.acceptFailed'); }
  finally { if (!disposed) operation.value = ''; }
}
async function removeContact() {
  if (busy.value || !selected.value) return;
  const peerId = selectedPeer.value; const current = version; operation.value = 'remove'; error.value = ''; removePrompt.value = false;
  try { const value = await invoke<MeshAppSnapshot>('mesh_remove_contact', { peerId }); if (!disposed && current === version) { applySnapshot(value); selectedPeer.value = ''; suggestion.value = null; drafts.value = Object.fromEntries(Object.entries(drafts.value).filter(([id]) => id !== peerId)); } }
  catch { if (!disposed && current === version) error.value = t('meshSocial.removeFailed'); }
  finally { if (!disposed) operation.value = ''; }
}
async function loadMessages() {
  if (!props.active || !props.showContacts || !snapshot.value?.capabilities.direct_messages || readingMessages.value || busy.value) return;
  const current = version; const request = ++messageRequest; readingMessages.value = true;
  let after = cursor.value;
  try {
    for (let attempt = 0; attempt < 2; attempt++) {
      const page = await invoke<MeshMessagePage>('mesh_list_messages', { after, limit: 40 });
      if (disposed || current !== version || request !== messageRequest || !props.active) return;
      const reset = Boolean(inboxGeneration.value && inboxGeneration.value !== page.generation);
      if (reset) { messages.value = []; cursor.value = 0; receipt.value = null; }
      inboxGeneration.value = page.generation;
      if (reset && after > 0) { after = 0; continue; }
      messages.value = [...new Map([...messages.value, ...page.messages].map(message => [message.sequence, message])).values()].sort((left, right) => left.sequence - right.sequence).slice(-256);
      cursor.value = page.next_after; hasMore.value = page.has_more; break;
    }
  } catch { if (!disposed && current === version) error.value = t('meshSocial.messagesFailed'); }
  finally { if (!disposed && current === version && request === messageRequest) readingMessages.value = false; }
}
async function send() {
  if (!canSend.value) return;
  const peerId = selectedPeer.value; const text = draft.value; const current = version; operation.value = 'send'; error.value = ''; receipt.value = null;
  try {
    const value = await invoke<MeshMessageReceipt>('mesh_send_message', { peerId, text });
    if (disposed || current !== version) return;
    if (!value.received) throw new Error('Message not acknowledged');
    receipt.value = value; if (drafts.value[peerId] === text) drafts.value = { ...drafts.value, [peerId]: '' };
  } catch (cause) { if (!disposed && current === version) error.value = contactError(cause, 'meshSocial.sendFailed'); }
  finally { if (!disposed) { operation.value = ''; if (current === version) void loadMessages(); } }
}
async function loadComposers() {
  if (!props.active || !props.showContacts || !snapshot.value?.capabilities.direct_messages) return;
  const current = version;
  try { const value = await invoke<MeshMessageComposer[]>('mesh_list_message_composers'); if (!disposed && current === version) { composers.value = value; if (!value.some(item => item.plugin_id === composerId.value)) composerId.value = value[0]?.plugin_id ?? ''; } }
  catch { if (!disposed && current === version) composers.value = []; }
}
async function generateDraft() {
  if (!selected.value || !composerId.value || busy.value) return;
  const current = version; const request = ++composeVersion; const peerId = selectedPeer.value; const pluginId = composerId.value; cancelRequested = false; operation.value = 'compose'; error.value = ''; suggestion.value = null;
  try {
    const value = await invoke<{ text: string }>('mesh_compose_message', { pluginId, peerId, gameId: contextGame.value || null, note: note.value, locale: pluginLocale() });
    if (!disposed && current === version && request === composeVersion && peerId === selectedPeer.value) suggestion.value = { peerId, text: value.text };
  } catch { if (!disposed && current === version && request === composeVersion && !cancelRequested) error.value = t(contextGame.value ? 'meshSocial.composeFailed' : 'meshSocial.composeNoGameFailed'); }
  finally { if (!disposed && operation.value === 'compose') operation.value = ''; }
}
async function cancelComposer() { if (operation.value !== 'compose') return; cancelRequested = true; ++composeVersion; await invoke('mesh_cancel_message_composer').catch(() => {}); }
function useDraft() { if (suggestion.value?.peerId === selectedPeer.value) { draft.value = suggestion.value.text; suggestion.value = null; } }
function selectContact(peerId: string) { if (busy.value) return; selectedPeer.value = peerId; suggestion.value = null; receipt.value = null; void loadMessages(); }
watch([contacts, () => props.initialContactId, () => props.contactRequestId, () => props.active, () => props.showContacts], ([items, peerId, requestId, active, visible]) => {
  if (!peerId || !requestId || requestId === handledContactRequest || !active || !visible || !items.some(contact => contact.profile.peer_id === peerId)) return;
  handledContactRequest = requestId;
  selectContact(peerId);
  emit('contactOpened', requestId);
}, { immediate: true });
function canLeave() { return !busy.value && !nameDirty.value && !messageDirty.value && !suggestion.value; }
async function confirmLeave(): Promise<boolean> { if (busy.value) return false; if (canLeave()) return true; leavePrompt.value = true; return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; }); }
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
function discard() { drafts.value = {}; suggestion.value = null; cancelName(); finishLeave(true); }
function time(value: number) { return new Date(value * 1000).toLocaleString(locale.value); }
watch(() => props.active, active => { if (active) void refreshSnapshot(); else { ++version; ++messageRequest; checking.value = false; readingMessages.value = false; invitation.value = null; invitationInput.value = ''; void cancelComposer(); } }, { immediate: true });
watch(() => props.gameId, value => { contextGame.value = value ?? ''; });
watch(contextGame, () => { suggestion.value = null; void cancelComposer(); });
watch([() => props.active, () => props.showContacts, () => snapshot.value?.profile.peer_id], ([active, visible, peer]) => {
  if (timer) clearInterval(timer); timer = undefined;
  if (active && visible && peer) { void loadMessages(); void loadComposers(); timer = setInterval(() => { if (!busy.value) { void refreshSnapshot(); void loadMessages(); } }, 3000); }
});
onUnmounted(() => { void cancelComposer(); disposed = true; ++version; ++messageRequest; if (timer) clearInterval(timer); leaveResolver?.(false); if (busy.value) emit('busy', false); });
defineExpose({ canLeave, confirmLeave });
</script>
<template>
  <section
    class="mesh-social"
    :aria-busy="busy || checking"
  >
    <LoadingIndicator
      v-if="checking && !checked"
      :label="t('meshSocial.loading')"
      compact
    />
    <form
      v-else-if="checked && !identityProfile"
      class="mesh-social-card mesh-start"
      @submit.prevent="start"
    >
      <strong>{{ t('meshSocial.myIdentity') }}</strong><label>{{ t('meshSocial.name') }}<input
        v-model="startName"
        :disabled="busy"
        autocomplete="off"
      ></label><button
        class="secondary-button"
        type="submit"
        :disabled="!startValid || busy"
      >
        <AppIcon
          v-if="busy"
          name="loader-circle"
          class="ui-busy-icon"
        />{{ t('meshSocial.start') }}
      </button>
    </form>
    <MeshIdentityCard
      v-if="identityProfile"
      :profile="identityProfile"
      :temporary="identityState.mode === 'temporary'"
      :active="Boolean(snapshot)"
      :busy="busy"
      :editing="editingName"
      :copied="copied"
      @edit="editingName = true"
      @copy="copyIdentity"
      @switch="identitySwitchPrompt = true"
    >
      <button
        v-if="!snapshot"
        class="secondary-button"
        :disabled="busy"
        @click="startName = identityProfile.display_name; start()"
      >
        <AppIcon name="network" />{{ t('meshSocial.start') }}
      </button>
      <form
        v-if="editingName"
        class="mesh-name-form"
        @submit.prevent="saveName"
      >
        <label>{{ t('meshSocial.name') }}<input
          v-model="nameDraft"
          :disabled="busy"
          autocomplete="off"
        ></label><button
          class="secondary-button"
          type="submit"
          :disabled="!nameDirty || !nameValid || busy"
        >
          <AppIcon
            v-if="operation === 'rename'"
            name="loader-circle"
            class="ui-busy-icon"
          />{{ t('gameConfigs.save') }}
        </button><button
          class="text-button"
          type="button"
          :disabled="busy"
          @click="cancelName"
        >
          {{ t('settingsLayout.cancel') }}
        </button>
      </form>
    </MeshIdentityCard>
    <template v-if="snapshot">
      <p
        v-if="snapshot.public_events_paused === true"
        class="setting-hint"
        role="status"
      >
        {{ t('meshSocial.publicEventsPaused') }}
      </p>
      <div
        v-if="showContacts"
        class="mesh-contacts-layout"
      >
        <aside class="mesh-social-card mesh-contact-list">
          <div class="mesh-contact-title">
            <h2>{{ t('meshSocial.contacts') }}</h2><span
              v-if="snapshot.capabilities.lan_only"
              class="mesh-network-badge"
            >{{ t('meshSocial.localNetwork') }}</span>
          </div>
          <label class="mesh-contact-search"><AppIcon name="search" /><input
            v-model="query"
            type="search"
            :aria-label="t('meshSocial.search')"
            :placeholder="t('meshSocial.search')"
          ></label>
          <p
            v-if="!contacts.length"
            class="setting-hint"
          >
            {{ t('meshSocial.empty') }}
          </p>
          <ul>
            <li
              v-for="contact in filteredContacts"
              :key="contact.profile.peer_id"
            >
              <button
                class="mesh-contact"
                :aria-pressed="selectedPeer === contact.profile.peer_id"
                :disabled="busy"
                @click="selectContact(contact.profile.peer_id)"
              >
                <MeshIdentityAvatar :peer-id="contact.profile.peer_id" /><strong>{{ meshProfileLabel(contact.profile) }}</strong><small>{{ t(contact.connected ? 'meshSocial.connected' : 'meshSocial.notConnected') }}</small>
              </button>
            </li>
          </ul>
          <SettingsDisclosure
            v-if="snapshot.capabilities.contact_invites"
            :title="t('meshSocial.addContact')"
          >
            <button
              class="text-button"
              :disabled="busy"
              @click="createInvitation"
            >
              {{ t('meshSocial.createInvite') }}
            </button>
            <template v-if="invitation">
              <textarea
                readonly
                :value="invitation.invitation"
                :aria-label="t('meshSocial.privateInvite')"
                rows="3"
              /><p class="setting-hint">
                {{ t('mesh.privateHint') }}
              </p><button
                class="text-button"
                @click="copy(invitation.invitation, 'invite')"
              >
                {{ t('meshSocial.copyInvite') }}
              </button>
            </template>
            <form
              class="mesh-accept-form"
              @submit.prevent="acceptInvitation"
            >
              <label>{{ t('meshSocial.privateInvite') }}<textarea
                v-model="invitationInput"
                :disabled="busy"
                rows="3"
                autocomplete="off"
                spellcheck="false"
              /></label><button
                class="text-button"
                type="submit"
                :disabled="busy || !invitationInput.trim()"
              >
                {{ t('meshSocial.addContact') }}
              </button>
            </form>
          </SettingsDisclosure>
        </aside>
        <section class="mesh-social-card mesh-conversation">
          <template v-if="selected">
            <div class="mesh-contact-title">
              <h2>{{ meshProfileLabel(selected.profile) }}</h2><span>{{ t(selected.connected ? 'meshSocial.connected' : 'meshSocial.notConnected') }}</span>
            </div>
            <div class="mesh-public-id">
              <span :title="selected.profile.peer_id">{{ selected.profile.peer_id }}</span><AppIconButton
                icon="copy"
                :label="t('meshSocial.copyContactId')"
                @click="copy(selected.profile.peer_id, 'contact')"
              />
            </div>
            <p class="setting-hint">
              {{ t('meshSocial.keyBoundContact') }}
            </p>
            <button
              class="text-button"
              :disabled="busy"
              @click="removePrompt = true"
            >
              {{ t('meshSocial.removeContact') }}
            </button>
            <template v-if="snapshot.capabilities.direct_messages">
              <p class="setting-hint">
                {{ t('meshSocial.history') }}
              </p>
              <LoadingIndicator
                v-if="readingMessages && !messages.length"
                :label="t('meshSocial.loadingMessages')"
                compact
              />
              <p
                v-else-if="!currentMessages.length && !hasMore"
                class="setting-hint"
              >
                {{ t('meshSocial.noMessages') }}
              </p>
              <ol class="mesh-messages">
                <li
                  v-for="message in currentMessages"
                  :key="`${inboxGeneration}:${message.id}`"
                  :class="{ outgoing: message.direction === 'outgoing' }"
                >
                  <small>{{ message.direction === 'outgoing' ? t('meshSocial.you') : meshProfileLabel(selected.profile) }} · {{ time(message.at) }}</small><p>{{ message.text }}</p>
                </li>
              </ol>
              <button
                v-if="hasMore"
                class="text-button"
                :disabled="readingMessages"
                @click="loadMessages"
              >
                {{ t('meshSocial.loadMore') }}
              </button>
              <form
                class="mesh-message-form"
                @submit.prevent="send"
              >
                <label>{{ t('meshSocial.message') }}<textarea
                  v-model="draft"
                  :readonly="busy"
                  rows="3"
                  spellcheck="false"
                /></label><p
                  v-if="messageTooLong"
                  class="settings-error"
                >
                  {{ t('meshSocial.tooLong') }}
                </p><button
                  class="secondary-button"
                  type="submit"
                  :disabled="!canSend"
                >
                  <AppIcon
                    v-if="operation === 'send'"
                    name="loader-circle"
                    class="ui-busy-icon"
                  />{{ t('meshSocial.send') }}
                </button><span
                  v-if="receipt?.received"
                  role="status"
                >{{ t('meshSocial.received') }}</span>
              </form>
              <SettingsDisclosure
                v-if="composers.length"
                :title="t('meshSocial.composer')"
              >
                <label v-if="games.some(game => game.status === 'installed')">{{ t('meshSocial.game') }}<select
                  v-model="contextGame"
                  :disabled="busy"
                ><option value="">{{ t('meshSocial.noGame') }}</option><option
                  v-for="game in games.filter(game => game.status === 'installed')"
                  :key="game.id"
                  :value="game.id"
                >{{ game.name }}</option></select></label><label>{{ t('meshSocial.extension') }}<select
                  v-model="composerId"
                  :disabled="busy"
                ><option
                  v-for="composer in composers"
                  :key="composer.plugin_id"
                  :value="composer.plugin_id"
                >{{ composer.name }}</option></select></label><label>{{ t('meshSocial.instruction') }}<textarea
                  v-model="note"
                  :disabled="busy"
                  rows="2"
                /></label><div class="mesh-social-actions">
                  <button
                    class="text-button"
                    :disabled="busy || !composerId"
                    @click="generateDraft"
                  >
                    {{ t('meshSocial.generate') }}
                  </button><button
                    v-if="operation === 'compose'"
                    class="text-button"
                    @click="cancelComposer"
                  >
                    {{ t('settingsLayout.cancel') }}
                  </button>
                </div><div
                  v-if="suggestion"
                  class="mesh-extension-draft"
                >
                  <strong>{{ t('meshSocial.draft') }}</strong><p>{{ suggestion.text }}</p><button
                    class="text-button"
                    :disabled="busy"
                    @click="useDraft"
                  >
                    {{ t('meshSocial.useDraft') }}
                  </button><button
                    class="text-button"
                    :disabled="busy"
                    @click="suggestion = null"
                  >
                    {{ t('settingsLayout.discard') }}
                  </button>
                </div>
              </SettingsDisclosure>
            </template>
            <p
              v-else
              class="setting-hint"
            >
              {{ t('meshSocial.messagesUnavailable') }}
            </p>
          </template>
          <p
            v-else
            class="setting-hint"
          >
            {{ t('meshSocial.chooseContact') }}
          </p>
        </section>
      </div>
    </template>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p><button
      v-if="error && !snapshot"
      class="text-button"
      :disabled="busy || checking"
      @click="refreshSnapshot"
    >
      {{ t('plugins.retry') }}
    </button>
    <DialogRoot
      :open="identitySwitchPrompt"
      @update:open="identitySwitchPrompt = $event"
    >
      <DialogPortal>
        <DialogOverlay class="mesh-social-overlay" />
        <DialogContent
          class="mesh-social-dialog"
          :aria-describedby="'mesh-identity-switch-description'"
        >
          <DialogTitle>{{ t(identityState.mode === 'temporary' ? 'meshSocial.returnSaved' : 'meshSocial.useTemporary') }}</DialogTitle>
          <p id="mesh-identity-switch-description">
            {{ t(identityState.mode === 'temporary' ? 'meshSocial.returnSavedEffect' : 'meshSocial.temporaryEffect') }}
          </p>
          <div class="mesh-social-actions">
            <button
              class="text-button"
              @click="identitySwitchPrompt = false"
            >
              {{ t('settingsLayout.cancel') }}
            </button><button
              class="secondary-button"
              :disabled="busy"
              @click="switchIdentity"
            >
              {{ t(identityState.mode === 'temporary' ? 'meshSocial.returnSaved' : 'meshSocial.useTemporary') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
    <DialogRoot
      :open="removePrompt"
      @update:open="removePrompt = $event"
    >
      <DialogPortal>
        <DialogOverlay class="mesh-social-overlay" /><DialogContent
          class="mesh-social-dialog"
          :aria-describedby="undefined"
        >
          <DialogTitle>{{ t('meshSocial.removeTitle') }}</DialogTitle><div class="mesh-social-actions">
            <button
              class="text-button"
              @click="removePrompt = false"
            >
              {{ t('settingsLayout.cancel') }}
            </button><button
              class="secondary-button"
              @click="removeContact"
            >
              {{ t('meshSocial.removeContact') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="mesh-social-overlay" /><DialogContent
          class="mesh-social-dialog"
          :aria-describedby="undefined"
        >
          <DialogTitle>{{ t('meshSocial.leaveTitle') }}</DialogTitle><div class="mesh-social-actions">
            <button
              class="text-button"
              @click="discard"
            >
              {{ t('settingsLayout.discard') }}
            </button><button
              class="secondary-button"
              @click="finishLeave(false)"
            >
              {{ t('settingsLayout.continueEditing') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
  </section>
</template>
<style scoped>
.mesh-social{min-width:0;margin:12px 0 20px}.mesh-social-card{min-width:0;border:1px solid var(--line);border-radius:12px;padding:16px;background:var(--panel)}.mesh-social input,.mesh-social textarea,.mesh-social select{width:100%;min-width:0;box-sizing:border-box;min-height:40px;border:1px solid var(--line);border-radius:8px;padding:9px 11px;background:var(--bg);color:var(--ink);font:inherit;font-size:12px}.mesh-social textarea{resize:vertical}.mesh-social label{display:grid;gap:7px;font-size:12px;margin:10px 0}.mesh-social button{display:inline-flex;align-items:center;justify-content:center;gap:7px}.mesh-social button>svg{width:15px;height:15px;flex:none}.mesh-start,.mesh-name-form{display:flex;gap:12px;align-items:end;flex-wrap:wrap}.mesh-start>strong{flex-basis:100%;font-size:13px}.mesh-start label,.mesh-name-form label{flex:1;min-width:160px;margin:0}.mesh-identity{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.mesh-identity>strong{font-size:14px}.mesh-name-form{flex-basis:100%}.mesh-public-id{display:flex;align-items:center;gap:6px;min-width:0;flex:1}.mesh-public-id>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:11px;color:var(--muted);min-width:0}.mesh-stop{margin-left:auto}.mesh-contacts-layout{display:grid;grid-template-columns:minmax(200px,280px) minmax(0,1fr);gap:16px;margin-top:16px}.mesh-contact-title{display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap}.mesh-identity>strong,.mesh-contact-title h2{overflow-wrap:anywhere;font-size:14px;margin:0}.mesh-contact-title>span{font-size:11px;color:var(--muted)}.mesh-network-badge{border:1px solid var(--line);border-radius:999px;padding:3px 7px}.mesh-contact-search{display:flex!important;align-items:center;gap:7px;padding:0 9px;border:1px solid var(--line);border-radius:8px}.mesh-contact-search>svg{width:15px;height:15px;flex:none}.mesh-contact-search input{border:0;padding:8px 0;outline:0;background:transparent}.mesh-contact-search input::-webkit-search-decoration,.mesh-contact-search input::-webkit-search-cancel-button{appearance:none}.mesh-contact-list ul{list-style:none;margin:10px 0;padding:0}.mesh-contact{display:grid!important;justify-content:start!important;gap:5px!important;width:100%;text-align:left;border:1px solid transparent;border-radius:8px;padding:10px;background:transparent;color:var(--ink)}.mesh-contact[aria-pressed=true]{border-color:var(--accent);background:var(--tint)}.mesh-contact :deep(.mesh-identity-avatar){width:32px;height:32px;grid-row:1 / 3}.mesh-contact{grid-template-columns:32px minmax(0,1fr)}.mesh-contact strong{font-size:12px;overflow-wrap:anywhere}.mesh-contact small{font-size:11px;color:var(--muted)}.mesh-accept-form{margin-top:16px}.mesh-conversation>.mesh-public-id{margin:12px 0}.mesh-messages{list-style:none;margin:14px 0;padding:0}.mesh-messages li{padding:12px;border:1px solid var(--line);border-radius:10px;margin:8px 0;max-width:90%;background:var(--bg)}.mesh-messages li.outgoing{margin-left:auto;background:var(--tint)}.mesh-messages small{display:block;font-size:10px;color:var(--muted)}.mesh-messages p,.mesh-extension-draft p{white-space:pre-wrap;overflow-wrap:anywhere;font-size:13px;line-height:1.6;margin:6px 0 0}.mesh-message-form>span{margin-left:12px;font-size:12px;color:var(--muted)}.mesh-social-actions{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}.mesh-extension-draft{padding:12px;border:1px solid var(--line);border-radius:8px;margin-top:12px}.mesh-extension-draft>strong{font-size:12px}.mesh-extension-draft>button{margin:10px 8px 0 0}.mesh-social-overlay{position:fixed;inset:0;background:#0006;z-index:100}.mesh-social-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));padding:24px;box-sizing:border-box;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}.mesh-social-dialog h2{font-size:16px;margin:0}@media(max-width:760px){.mesh-contacts-layout{grid-template-columns:minmax(0,1fr)}.mesh-public-id{flex-basis:100%}.mesh-identity>.mesh-public-id{order:3}.mesh-stop{margin-left:auto}}
</style>
