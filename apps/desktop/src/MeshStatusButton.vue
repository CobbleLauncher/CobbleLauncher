<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { MeshIdentityState } from './mesh-social';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';

const props = withDefaults(defineProps<{ active?: boolean; beforeChange?: () => Promise<boolean> }>(), { active: false, beforeChange: undefined });
const emit = defineEmits<{ open: []; changed: [] }>();

const connected = ref(false);
const enabled = ref(true);
const relayAvailable = ref(false);
const discoveryChecked = ref(false);
const checked = ref(false);
const statusFailed = ref(false);
const hasIdentity = ref<boolean | null>(null);
const busy = ref(false);
const enabling = ref(false);
const menuOpen = ref(false);
const error = ref('');
let disposed = false;
let refreshing = false;
let revision = 0;
let interval: ReturnType<typeof setInterval> | undefined;
let initialRefresh: ReturnType<typeof setTimeout> | undefined;
type NetworkStatus = { enabled?: boolean; connected: boolean; relay_available?: boolean; discovery_checked?: boolean };
const state = computed(() => busy.value ? (enabling.value ? 'connecting' : 'switching') : !checked.value ? 'checking' : statusFailed.value ? 'unknown' : !enabled.value ? 'offline' : connected.value ? 'online' : relayAvailable.value ? 'ready' : discoveryChecked.value ? 'unconnected' : 'idle');
const statusTitle = computed(() => t(state.value === 'online' ? 'mesh.networkStatus.connected' : state.value === 'ready' ? 'mesh.networkStatus.readyHint' : state.value === 'offline' ? 'mesh.networkStatus.offlinePage' : state.value === 'idle' || state.value === 'unconnected' ? 'mesh.networkStatus.disconnected' : `mesh.networkStatus.${state.value}`));
function applyStatus(status: NetworkStatus | null) {
  enabled.value = status?.enabled !== false;
  connected.value = enabled.value && status?.connected === true;
  relayAvailable.value = enabled.value && status?.relay_available === true;
  discoveryChecked.value = status?.discovery_checked === true;
  checked.value = true; statusFailed.value = false;
}

watch(menuOpen, async open => {
  if (!open) return;
  void refresh();
  try {
    const identity = await invoke<MeshIdentityState | null>('mesh_identity_state');
    if (!disposed) hasIdentity.value = identity ? identity.profile !== null : null;
  } catch { if (!disposed) hasIdentity.value = null; }
});

async function refresh() {
  if (disposed || refreshing || busy.value || document.visibilityState !== 'visible') return;
  refreshing = true;
  const current = revision;
  try {
    const status = await invoke<NetworkStatus | null>('mesh_network_status');
    if (!disposed && current === revision) applyStatus(status);
  } catch {
    if (!disposed && current === revision) { connected.value = false; relayAvailable.value = false; checked.value = true; statusFailed.value = true; }
  } finally {
    refreshing = false;
    if (!disposed && current !== revision && !busy.value) void refresh();
  }
}

async function setEnabled(value: boolean) {
  if (busy.value || !await (props.beforeChange?.() ?? Promise.resolve(true)) || disposed || busy.value) return;
  ++revision;
  busy.value = true; enabling.value = value; error.value = '';
  try {
    const status = await invoke<NetworkStatus>('mesh_set_enabled', { enabled: value });
    if (disposed) return;
    applyStatus(status);
    if (value) await invoke('mesh_discover');
  } catch { if (!disposed) { error.value = t('mesh.networkStatus.failed'); menuOpen.value = true; } }
  finally { if (!disposed) { busy.value = false; emit('changed'); void refresh(); } }
}

function startPolling() {
  if (disposed || document.visibilityState !== 'visible' || interval) return;
  void refresh();
  interval = setInterval(() => { void refresh(); }, 15_000);
}

function updateVisibility() {
  if (document.visibilityState === 'visible') startPolling();
  else if (interval) { clearInterval(interval); interval = undefined; }
}

onMounted(() => {
  document.addEventListener('visibilitychange', updateVisibility);
  initialRefresh = setTimeout(() => {
    initialRefresh = undefined;
    startPolling();
  }, 0);
});

onUnmounted(() => {
  disposed = true;
  if (interval) clearInterval(interval);
  if (initialRefresh) clearTimeout(initialRefresh);
  document.removeEventListener('visibilitychange', updateVisibility);
});
</script>

<template>
  <DropdownMenuRoot v-model:open="menuOpen">
    <DropdownMenuTrigger
      type="button"
      class="mesh-status-button"
      :class="{ 'is-connected': connected, 'is-ready': relayAvailable && !connected }"
      :aria-label="statusTitle"
      :aria-current="active ? 'page' : undefined"
      :title="statusTitle"
      :disabled="busy"
    >
      <span
        class="mesh-status-button-dot"
        :class="{ 'is-busy': busy }"
        aria-hidden="true"
      />
      <span>{{ t(`mesh.networkStatus.${state}`) }}</span>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        class="profile-menu mesh-status-menu"
        align="end"
        :side-offset="8"
        :collision-padding="12"
      >
        <DropdownMenuItem
          class="profile-menu-item"
          @select="emit('open')"
        >
          <AppIcon name="network" />{{ t('mesh.networkStatus.open') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          v-if="hasIdentity === false && enabled"
          class="profile-menu-item"
          @select="emit('open')"
        >
          <AppIcon name="user" />{{ t('mesh.networkStatus.createIdentity') }}
        </DropdownMenuItem>
        <DropdownMenuSeparator class="profile-menu-separator" />
        <DropdownMenuItem
          v-if="!connected"
          class="profile-menu-item"
          :disabled="busy"
          @select="setEnabled(true)"
        >
          <AppIcon name="refresh-cw" />{{ t(enabled ? 'mesh.networkStatus.retry' : 'mesh.networkStatus.enable') }}
        </DropdownMenuItem>
        <DropdownMenuItem
          v-if="enabled"
          class="profile-menu-item"
          :disabled="busy"
          @select="setEnabled(false)"
        >
          <AppIcon name="x" />{{ t('mesh.networkStatus.disable') }}
        </DropdownMenuItem>
        <p
          v-if="error"
          class="mesh-status-error"
          role="alert"
        >
          {{ error }}
        </p>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>

<style scoped>
.mesh-status-button {
  display: inline-flex;
  width: auto;
  height: 28px;
  min-height: 28px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 0 8px;
  margin-right: 6px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--ui-text-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.mesh-status-button :deep(.app-icon) { width: 16px; height: 16px; }
.mesh-status-button:hover, .mesh-status-button[aria-current=page] { color: var(--ui-text); }
.mesh-status-button-dot {
  width: 6px;
  height: 6px;
  flex: 0 0 6px;
  border-radius: 50%;
  background: var(--ui-text-muted);
}
.mesh-status-button.is-connected .mesh-status-button-dot { background: var(--ui-success); }
.mesh-status-button.is-ready .mesh-status-button-dot { background: var(--ui-success); }
.mesh-status-error{margin:8px 12px;max-width:240px;color:var(--danger);font-size:13px}
.mesh-status-button-dot.is-busy{animation:mesh-status-pulse 1s ease-in-out infinite}
@keyframes mesh-status-pulse{50%{opacity:.35}}
@media(prefers-reduced-motion:reduce){.mesh-status-button-dot.is-busy{animation:none}}
</style>
