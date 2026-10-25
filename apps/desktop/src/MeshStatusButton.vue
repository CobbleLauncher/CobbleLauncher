<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { newerNetworkSnapshot, type MeshNetworkSnapshot } from './mesh';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import MeshContactsButton from './MeshContactsButton.vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';

const props = withDefaults(defineProps<{ active?: boolean; beforeChange?: () => Promise<boolean>; snapshot?: MeshNetworkSnapshot | null }>(), { active: false, beforeChange: undefined, snapshot: null });
const emit = defineEmits<{ open: []; changed: []; contact: [peerId: string]; join: []; contacts: [] }>();
const current = ref<MeshNetworkSnapshot | null>(props.snapshot);
const busy = ref(false);
const menuOpen = ref(false);
const error = ref('');
let disposed = false;
let unlisten: UnlistenFn | undefined;
let readVersion = 0;
let eventSerial = 0;
const phase = computed(() => current.value?.phase ?? 'connecting');
const label = computed(() => t(`mesh.networkStatus.${phase.value}`));
const enabled = computed(() => current.value?.enabled ?? true);
const canRetry = computed(() => enabled.value && phase.value === 'failed');
watch(() => props.snapshot, value => { if (value) current.value = newerNetworkSnapshot(current.value, value); });
function apply(value: MeshNetworkSnapshot) {
  if (disposed) return;
  current.value = newerNetworkSnapshot(current.value, value);
}
async function read() {
  const version = ++readVersion;
  try {
    const value = await invoke<MeshNetworkSnapshot>('mesh_network_snapshot');
    if (!disposed && version === readVersion) apply(value);
  } catch {
    if (!disposed && version === readVersion) error.value = t('mesh.networkStatus.readFailed');
  }
}
async function change(action: 'enable' | 'disable' | 'retry') {
  if (busy.value || !await (props.beforeChange?.() ?? Promise.resolve(true)) || disposed || busy.value) return;
  busy.value = true;
  error.value = '';
  const serial = eventSerial;
  try {
    const value = action === 'retry'
      ? await invoke<MeshNetworkSnapshot>('mesh_network_retry')
      : await invoke<MeshNetworkSnapshot>('mesh_network_set_enabled', { enabled: action === 'enable' });
    if (serial === eventSerial || !current.value || value.generation > current.value.generation) apply(value);
    emit('changed');
  } catch {
    if (!disposed) { error.value = t('mesh.networkStatus.failedAction'); menuOpen.value = true; }
  } finally {
    if (!disposed) busy.value = false;
  }
}
onMounted(() => {
  void listen<MeshNetworkSnapshot>('mesh-network-changed', event => {
    eventSerial++;
    readVersion++;
    const before = current.value;
    apply(event.payload);
    if (current.value && (before?.generation !== current.value.generation || before?.phase !== current.value.phase)) emit('changed');
  }).then(stop => {
    if (disposed) stop(); else unlisten = stop;
  }).catch(() => {});
  if (!props.snapshot) void read();
});
onUnmounted(() => { disposed = true; ++readVersion; unlisten?.(); });
</script>

<template>
  <div class="mesh-status-actions">
    <DropdownMenuRoot v-model:open="menuOpen">
      <DropdownMenuTrigger
        type="button"
        class="mesh-status-button"
        :class="{ 'is-online': phase === 'online', 'is-busy': phase === 'connecting' || phase === 'reconnecting' }"
        :aria-label="label"
        :aria-current="active ? 'page' : undefined"
        :title="label"
      >
        <span
          class="mesh-status-button-dot"
          aria-hidden="true"
        />
        <span>{{ label }}</span>
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
          <DropdownMenuSeparator class="profile-menu-separator" />
          <DropdownMenuItem
            v-if="!enabled || phase === 'offline'"
            class="profile-menu-item"
            :disabled="busy"
            @select="change('enable')"
          >
            <AppIcon name="network" />{{ t('mesh.networkStatus.enable') }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-else-if="canRetry"
            class="profile-menu-item"
            :disabled="busy"
            @select="change('retry')"
          >
            <AppIcon name="refresh-cw" />{{ t('mesh.networkStatus.retry') }}
          </DropdownMenuItem>
          <DropdownMenuItem
            v-if="enabled"
            class="profile-menu-item"
            :disabled="busy"
            @select="change('disable')"
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
    <MeshContactsButton
      :available="enabled && phase === 'online'"
      @contact="emit('contact', $event)"
      @join="emit('join')"
      @open="emit('contacts')"
    />
  </div>
</template>

<style scoped>
.mesh-status-actions{display:inline-flex;align-items:center;gap:2px;margin-right:6px}
.mesh-status-button{display:inline-flex;min-height:28px;align-items:center;gap:6px;padding:0 8px;border:0;border-radius:4px;background:transparent;color:var(--ui-text-muted);font:inherit;font-size:12px;cursor:pointer}
.mesh-status-button:hover,.mesh-status-button[aria-current=page]{color:var(--ui-text)}
.mesh-status-button-dot{width:6px;height:6px;flex:0 0 6px;border-radius:50%;background:var(--ui-text-muted)}
.mesh-status-button.is-online .mesh-status-button-dot{background:var(--ui-success)}
.mesh-status-button.is-busy .mesh-status-button-dot{animation:mesh-status-pulse 1s ease-in-out infinite}
.mesh-status-error{margin:8px 12px;max-width:240px;color:var(--danger);font-size:13px}
@keyframes mesh-status-pulse{50%{opacity:.35}}
@media(prefers-reduced-motion:reduce){.mesh-status-button.is-busy .mesh-status-button-dot{animation:none}}
</style>
