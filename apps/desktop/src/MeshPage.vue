<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { Game } from './games';
import MeshSocialPanel from './MeshSocialPanel.vue';
import SelectField from './ui/SelectField.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import MeshConnectionDetails from './mesh-ui/MeshConnectionDetails.vue';
import MeshSessionResult from './mesh-ui/MeshSessionResult.vue';
import MeshNodeSettings from './mesh-ui/MeshNodeSettings.vue';
import MeshInspector from './MeshInspector.vue';
import AppIcon from './ui/AppIcon.vue';
import { t } from './i18n';
import { defaultMeshRoutePolicy, meshSessionActive, type MeshRoutePolicy, type MeshAvailability, type MeshOverview, type MeshHostOptions, type MeshHostResult, type MeshJoinResult, type MeshSession } from './mesh';

const props = defineProps<{ games: Game[]; initialGameId: string; active: boolean; initialMode?: 'host' | 'join' | 'contacts'; initialContactId?: string; contactRequestId?: number; networkRevision?: number }>();
const emit = defineEmits<{ busy: [value: boolean]; launch: [gameId: string]; contactOpened: [requestId: number] }>();
const mode = ref<'host' | 'server' | 'join' | 'contacts'>(props.initialMode ?? 'host');
const social = ref<InstanceType<typeof MeshSocialPanel> | null>(null);
const socialBusy = ref(false);
const gameId = ref(props.initialGameId || props.games.find(game => game.status === 'installed')?.id || '');
const portEdited = ref(false);
const port = ref(''); const relay = ref('');
const routePolicy = ref<MeshRoutePolicy>(defaultMeshRoutePolicy());
let policyEdited = false;
function changeRoutePolicy(value: MeshRoutePolicy) { routePolicy.value = value; policyEdited = true; }
const invitationInput = ref(''); const invitation = ref(''); const bridge = ref('');
const availability = ref<MeshAvailability | null>(null); const session = ref<MeshSession | null>(null);
const busy = ref(false); const checking = ref(false); const error = ref(''); const copied = ref(false);
const overview = ref<MeshOverview | null>(null); const hostOptions = ref<MeshHostOptions | null>(null);
const discovering = ref(false);
const inspectorOpen = ref(false);
const connectionDetailsOpen = ref(false);
const retryInvitation = ref('');
const retryExpiresAt = ref<number | null>(null);
const retryExpired = computed(() => retryExpiresAt.value !== null && retryExpiresAt.value * 1000 <= now.value);
const invalidInvitation = computed(() => error.value === '邀请无效或已过期');
const now = ref(Date.now());
const invitationExpired = computed(() => !!session.value && session.value.expires_at * 1000 <= now.value);
const interrupted = ref(false);
const sessionStatus = computed(() => busy.value ? 'busy' : connected.value ? session.value!.state : interrupted.value ? 'disconnected' : session.value?.state ?? 'ready');
const relayChoices = computed(() => [
  ...(overview.value?.configured_services ?? []).filter(service => service.kind === 'relay' && service.state === 'authenticated').map(service => ({ id: service.id, endpoint: service.endpoint, label: t('mesh.officialConnection') })),
  ...(overview.value?.nodes ?? []).filter(node => node.relay).map(node => ({ id: node.peer_id, endpoint: node.endpoint, label: node.peer_id })),
]);
const playableGames = computed(() => props.games.filter(game => game.status === 'installed' && !game.content_installing));
const runningGames = computed(() => playableGames.value.filter(game => game.launch_state === 'running'));
const selectedGame = computed(() => playableGames.value.find(game => game.id === gameId.value));
const selectedRunning = computed(() => runningGames.value.some(game => game.id === gameId.value));
const connected = computed(() => meshSessionActive(session.value));
const sessionGame = computed(() => props.games.find(game => game.id === session.value?.game_id)?.name ?? '');
const hostValid = computed(() => availability.value?.can_host && (mode.value === 'server' || selectedRunning.value) && /^\d+$/.test(port.value) && Number(port.value) >= 1024 && Number(port.value) <= 65535 && relay.value.trim().length > 0);
watch([busy, socialBusy], ([gameBusy, contactBusy]) => emit('busy', gameBusy || contactBusy), { flush: 'sync' });
async function changeMode(value: typeof mode.value) { if (busy.value || socialBusy.value || !await (social.value?.confirmLeave() ?? true)) return; mode.value = value; if (value === 'server' && !port.value) port.value = '25565'; }
function canLeave() { return !busy.value && !socialBusy.value && (social.value?.canLeave() ?? true); }
async function confirmLeave() { return !busy.value && !socialBusy.value && await (social.value?.confirmLeave() ?? true); }
async function openInspector() { if (await confirmLeave()) inspectorOpen.value = true; }
defineExpose({ canLeave, confirmLeave });
let timer: ReturnType<typeof setInterval> | undefined; let disposed = false;
let optionsEpoch = 0;
async function discover() {
  if (discovering.value || busy.value) return;
  discovering.value = true; error.value = '';
  try { const result = await invoke<MeshOverview>('mesh_discover'); if (!disposed) { overview.value = result; availability.value = result; acceptSession(result.session); chooseRelay(); } }
  catch (cause) { if (!disposed) error.value = String(cause); }
  finally { discovering.value = false; }
}
function chooseRelay() { if (!relay.value.trim()) relay.value = relayChoices.value[0]?.endpoint ?? ''; }
async function loadHostOptions() {
  const epoch = ++optionsEpoch; const selected = gameId.value;
  if (!props.active || mode.value === 'server' || !selectedRunning.value || connected.value) return;
  try {
    const result = await invoke<MeshHostOptions>('mesh_host_options', { gameId: selected });
    if (disposed || epoch !== optionsEpoch || selected !== gameId.value) return;
    hostOptions.value = result;
    if (!portEdited.value && result.lan.port !== null) port.value = String(result.lan.port);
    if (!relay.value.trim()) relay.value = result.relay.address ?? '';
  } catch (cause) { if (!disposed && epoch === optionsEpoch) error.value = String(cause); }
}
function acceptSession(value: MeshSession | null) {
  if (meshSessionActive(session.value) && !meshSessionActive(value)) {
    interrupted.value = value?.state !== 'ended';
    if (session.value?.role === 'guest') mode.value = 'join';
    else mode.value = session.value?.target === 'server' ? 'server' : 'host';
  }
  now.value = Date.now();
  if (value?.session_id !== session.value?.session_id || !meshSessionActive(value)) { invitation.value = ''; bridge.value = ''; }
  session.value = value;
}
async function refresh() {
  if (checking.value || busy.value || !props.active) return;
  checking.value = true;
  try {
    const available = await invoke<MeshOverview>('mesh_overview');
    if (disposed) return;
    availability.value = available;
    overview.value = available;
    if (!policyEdited && available.relay?.route_policy) routePolicy.value = available.relay.route_policy;
    chooseRelay();
    if (available.available) {
      acceptSession(available.session);
      await loadHostOptions();
    } else acceptSession(null);
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { checking.value = false; }
}
async function retry() {
  if (mode.value === 'join' && retryExpired.value) return;
  if (mode.value === 'join' && retryInvitation.value) invitationInput.value = retryInvitation.value;
  await connect();
}
async function connect() {
  if (busy.value || checking.value || connected.value || !availability.value?.available || (mode.value !== 'server' && !selectedRunning.value) || (mode.value === 'host' || mode.value === 'server' ? !hostValid.value : !invitationInput.value.trim())) return;
  busy.value = true;  error.value = ''; copied.value = false;
  try {
    if (mode.value === 'server') {
      const result = await invoke<MeshHostResult>('mesh_host_server', { localPort: Number(port.value), relay: relay.value.trim(), approveLocalPort: true, routePolicy: routePolicy.value });
      if (!disposed) { acceptSession(result.session); invitation.value = result.invitation; }
    } else if (mode.value === 'host') {
      const result = await invoke<MeshHostResult>('mesh_host', { gameId: gameId.value, localPort: Number(port.value), relay: relay.value.trim(), approveLocalPort: true, routePolicy: routePolicy.value });
      if (!disposed) { acceptSession(result.session); invitation.value = result.invitation; }
    } else {
      const secret = invitationInput.value.trim(); retryExpiresAt.value = null; retryInvitation.value = secret; invitationInput.value = '';
      const result = await invoke<MeshJoinResult>('mesh_join', { gameId: gameId.value, invitation: secret, routePolicy: routePolicy.value });
      if (!disposed) { acceptSession(result.session); bridge.value = result.bridge_address; retryExpiresAt.value = result.session.expires_at; }
    }
    if (!disposed) interrupted.value = false;
  } catch (cause) { if (!disposed) { error.value = String(cause); interrupted.value = true; } }
  finally { busy.value = false;  }
}
async function end() {
  if (!session.value || busy.value || checking.value) return;
  busy.value = true;  error.value = ''; copied.value = false;
  try {
    const result = await invoke<{ session: MeshSession | null }>('mesh_end', { sessionId: session.value.session_id });
    if (!disposed) { acceptSession(result.session); retryInvitation.value = ''; interrupted.value = false; }
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { busy.value = false;  }
}
async function copy(value: string) {
  try { await writeText(value); copied.value = true; }
  catch { error.value = t('mesh.copyError'); }
}
watch([() => props.active, () => props.initialGameId, () => props.initialMode, () => props.initialContactId, () => props.contactRequestId], ([active, value, entryMode, contactId, requestId], [wasActive, previous, previousMode, previousContact, previousRequest]) => {
  if (active && (!wasActive || value !== previous || entryMode !== previousMode || contactId !== previousContact || requestId !== previousRequest) && !busy.value && !socialBusy.value) {
    if (!connected.value && !interrupted.value) gameId.value = value;
    if (entryMode) mode.value = entryMode;
    else if (!connected.value && !interrupted.value) mode.value = 'host';
  }
});
watch(selectedRunning, () => { void loadHostOptions(); });
watch(gameId, () => { hostOptions.value = null; port.value = ''; portEdited.value = false; void loadHostOptions(); });
watch(() => props.active, active => { if (active) void refresh(); else invitationInput.value = ''; }, { immediate: true });
watch(() => props.networkRevision, () => { void refresh(); });
watch([() => props.active, connected, selectedRunning], ([active, live]) => {
  if (timer) clearInterval(timer); timer = undefined;
  if (active && (live || selectedRunning.value)) timer = setInterval(() => { now.value = Date.now(); void refresh(); }, 3000);
}, { immediate: true });
onUnmounted(() => { disposed = true; if (timer) clearInterval(timer); invitationInput.value = ''; retryInvitation.value = ''; invitation.value = ''; bridge.value = ''; });
</script>
<template>
  <section class="mesh-page">
    <MeshInspector
      v-if="inspectorOpen"
      :active="active"
      @close="inspectorOpen = false"
    />
    <template v-else>
      <MeshSocialPanel
        v-if="active && availability?.available"
        ref="social"
        :active="active"
        :show-contacts="mode === 'contacts'"
        :game-id="gameId || undefined"
        :initial-contact-id="initialContactId"
        :contact-request-id="contactRequestId"
        :games="games"
        @busy="socialBusy = $event"
        @contact-opened="emit('contactOpened', $event)"
      />
      <div class="mesh-page-toolbar">
        <div
          class="mesh-toolbar"
          role="group"
          :aria-label="t('mesh.mode')"
        >
          <button
            v-for="item in (['host', 'server', 'join'] as const)"
            :key="item"
            class="text-button"
            :aria-pressed="mode === item"
            :disabled="busy || socialBusy"
            @click="changeMode(item)"
          >
            <AppIcon :name="item === 'host' ? 'network' : item === 'server' ? 'monitor' : 'plus'" />{{ t(`mesh.service_${item}`) }}
          </button>
          <button
            class="text-button mesh-contacts"
            :aria-pressed="mode === 'contacts'"
            :disabled="busy || socialBusy"
            @click="changeMode('contacts')"
          >
            <AppIcon name="user" />{{ t('meshSocial.contacts') }}
          </button>
        </div>
        <button
          class="text-button mesh-inspector-button"
          @click="openInspector"
        >
          <AppIcon
            name="info"
            :size="16"
          />
          {{ t('meshInspector.title') }}
        </button>
      </div>
      <LoadingIndicator
        v-if="checking && !availability"
        :label="t('mesh.checking')"
        compact
      />
      <div
        v-if="availability && !availability.available"
        class="mesh-card"
      >
        <p>{{ t(availability.reason === 'MESH_OFFLINE' ? 'mesh.networkStatus.offlinePage' : 'mesh.unavailable') }}</p><p
          v-if="availability.reason && availability.reason !== 'MESH_OFFLINE'"
          class="setting-hint"
        >
          {{ availability.reason }}
        </p>
      </div>
      <template v-if="availability?.available && mode !== 'contacts'">
        <MeshSessionResult
          v-if="connected"
          :status="sessionStatus"
          :game-name="sessionGame"
          :invitation="invitation"
          :invitation-expired="invitationExpired"
          :bridge="bridge"
          :disabled="busy || checking"
          @copy="copy"
          @end="end"
        />
        <form
          v-else
          class="mesh-card mesh-form"
          @submit.prevent="connect"
        >
          <strong
            v-if="busy || interrupted || sessionStatus !== 'ready'"
            role="status"
          >{{ t(`mesh.${sessionStatus}`) }}</strong>
          <button
            v-if="interrupted && (mode !== 'join' || (retryInvitation && !retryExpired && !invalidInvitation))"
            type="button"
            class="text-button"
            :disabled="busy || checking || (mode !== 'server' && !selectedRunning)"
            @click="retry"
          >
            <AppIcon name="refresh-cw" />{{ t(mode === 'join' ? 'mesh.retry' : 'mesh.createAgain') }}
          </button>
          <p
            v-if="mode === 'join' && (retryExpired || invalidInvitation)"
            role="status"
          >
            {{ t('mesh.newInvitation') }}
          </p>
          <label
            v-if="mode !== 'server'"
            for="mesh-game"
          >{{ t('mesh.game') }}</label>
          <SelectField
            v-if="mode !== 'server'"
            id="mesh-game"
            v-model="gameId"
            :label="t('mesh.game')"
            :placeholder="t('mesh.chooseGame')"
            :options="playableGames.map(game => ({ value: game.id, label: game.name }))"
            :disabled="busy || socialBusy"
          />
          <button
            v-if="mode !== 'server' && selectedGame && !selectedRunning"
            type="button"
            class="primary-button"
            :disabled="selectedGame.launch_state === 'preparing'"
            @click="emit('launch', gameId)"
          >
            <AppIcon name="play" />{{ t('mesh.startGame') }}
          </button>
          <template v-if="mode === 'host'">
            <template v-if="selectedRunning">
              <div
                class="mesh-lan"
                :class="{ 'mesh-lan-detected': hostOptions?.lan.state === 'detected' }"
              >
                <AppIcon :name="hostOptions?.lan.state === 'detected' ? 'check' : 'network'" /><div>
                  <strong>{{ t(hostOptions?.lan.state === 'detected' ? 'mesh.portDetected' : 'mesh.openLan') }}</strong><p
                    v-if="hostOptions?.lan.state !== 'detected'"
                    class="setting-hint"
                  >
                    {{ t('mesh.worldGuide') }}
                  </p>
                </div>
              </div>
            </template>
            <label v-if="selectedRunning">{{ t('mesh.port') }}<input
              v-model="port"
              type="number"
              min="1024"
              max="65535"
              :disabled="busy"
              required
              @input="portEdited = true"
            ></label>
            <p
              v-if="!availability.can_host"
              class="setting-hint"
            >
              {{ t('mesh.hostUnavailable') }}
            </p>
          </template>
          <label v-else-if="mode === 'join'">{{ t('mesh.invitation') }}<textarea
            v-model="invitationInput"
            :disabled="busy || socialBusy"
            rows="4"
            spellcheck="false"
            autocomplete="off"
            required
          /></label>
          <label v-else>{{ t('mesh.serverPort') }}<input
            v-model="port"
            type="number"
            min="1024"
            max="65535"
            :disabled="busy || socialBusy"
            required
          ></label>
          <p
            v-if="mode === 'host' && selectedRunning && !relay"
            role="status"
          >
            {{ t(discovering ? 'mesh.findingConnection' : 'mesh.connectionUnavailable') }}
          </p>
          <button
            v-if="mode !== 'join' && availability.can_host && (mode === 'server' || selectedRunning) && !relay && !discovering"
            type="button"
            class="text-button"
            @click="connectionDetailsOpen = true"
          >
            <AppIcon name="settings" />{{ t('mesh.details') }}
          </button>
          <button
            v-if="(mode === 'server' || selectedRunning) && (mode === 'join' || availability.can_host)"
            class="primary-button"
            type="submit"
            :disabled="busy || checking || (mode !== 'server' && !selectedRunning) || (mode === 'host' || mode === 'server' ? !hostValid : !invitationInput.trim())"
          >
            <AppIcon :name="busy ? 'loader-circle' : mode === 'join' ? 'network' : 'plus'" />{{ busy ? t('mesh.busy') : mode === 'host' || mode === 'server' ? t('mesh.create') : t('mesh.joinSession') }}
          </button>
        </form>
        <MeshConnectionDetails
          v-if="connected || mode === 'join' || (availability.can_host && (mode === 'server' || selectedRunning))"
          :key="String(connectionDetailsOpen)"
          :default-open="connectionDetailsOpen"
          :overview="overview"
          :session="session"
          :relay="relay"
          :port="port"
          :busy="busy || socialBusy"
          :discovering="discovering"
          :route-policy="routePolicy"
          @update:route-policy="changeRoutePolicy"
          @update:relay="relay = $event"
          @update:port="port = $event; portEdited = true"
          @discover="discover"
          @refresh="refresh"
          @inspector="openInspector"
        />
        <MeshNodeSettings />
      </template>
      <p
        v-if="copied"
        role="status"
      >
        {{ t('mesh.copied') }}
      </p><p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ t(error.toLowerCase().includes('direct_path_required') ? 'mesh.routePolicy.directRequired' : 'mesh.connectionFailed') }}
      </p>
    </template>
  </section>
</template>
<style scoped>
.mesh-page{width:100%;min-width:0;padding-top:12px}.mesh-page-toolbar{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:20px}.mesh-toolbar{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin:0}.mesh-contacts{margin-left:auto}.mesh-toolbar [aria-pressed=true]{background:var(--panel);box-shadow:inset 0 -2px var(--accent)}.mesh-inspector-button{display:inline-flex;align-items:center;gap:7px;white-space:nowrap}.mesh-card{border:1px solid var(--line);border-radius:14px;background:var(--panel);padding:24px}.mesh-form,.mesh-result{display:grid;gap:18px}.mesh-page label{display:grid;gap:8px}.mesh-page input,.mesh-page textarea,.mesh-page select{width:100%;min-width:0;box-sizing:border-box;min-height:42px;padding:10px 12px;border:1px solid var(--line);border-radius:12px;background:var(--bg);color:var(--ink);font:inherit}.mesh-page textarea{resize:vertical;overflow-wrap:anywhere}.setting-hint{margin:0}.mesh-form>button,.mesh-result>button{justify-self:start}.mesh-lan{display:grid;gap:6px}.mesh-toolbar button{border:0;border-radius:8px;min-height:40px;padding:10px 14px}@media(max-width:600px){.mesh-card{padding:18px}.mesh-page-toolbar{align-items:flex-start;flex-wrap:wrap}.mesh-toolbar{gap:2px}.mesh-toolbar button{padding:8px}}
</style>
<style scoped>
.mesh-page-toolbar{padding-bottom:var(--ui-space-4);border-bottom:1px solid var(--ui-border)}.mesh-toolbar button,.mesh-form>button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.mesh-toolbar button svg{width:16px;height:16px}.mesh-form{max-width:760px;border:0;border-radius:0;background:transparent;padding:var(--ui-space-2) 0 var(--ui-space-5);gap:var(--ui-space-3)}.mesh-form>label{font-weight:var(--ui-weight-medium);font-size:var(--ui-text-label)}.mesh-form>label:has(textarea){margin-top:var(--ui-space-3)}.mesh-form>button[type=submit]{margin-top:var(--ui-space-2)}.mesh-lan{display:flex;align-items:flex-start;gap:var(--ui-space-3);border-top:1px solid var(--ui-border);padding-top:var(--ui-space-5);margin-top:var(--ui-space-2)}.mesh-lan>svg{width:20px;height:20px;color:var(--ui-text-muted);flex:none}.mesh-lan>div{display:grid;gap:var(--ui-space-1)}.mesh-lan strong{font-size:var(--ui-text-label);font-weight:var(--ui-weight-medium)}.mesh-lan .setting-hint{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.mesh-lan-detected>svg{color:var(--ui-success)}.mesh-form input[type=number]{max-width:240px}.mesh-form textarea{font-family:var(--ui-font-mono);font-size:var(--ui-text-caption)}
</style>

