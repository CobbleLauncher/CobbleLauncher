<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from '../i18n';
import type { MeshNodeSnapshot } from '../mesh';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import SelectField from '../ui/SelectField.vue';
import AppIcon from '../ui/AppIcon.vue';
const access = ref<'community' | 'private'>('private'), address = ref(''), port = ref(4001), peers = ref('');
const bandwidth = ref(1024), quota = ref(1024), connections = ref(8), duration = ref(60);
const node = ref<MeshNodeSnapshot | null>(null), busy = ref(false), error = ref(false), copied = ref(false), open = ref(false);
const running = computed(() => node.value?.state === 'running');
const valid = computed(() => /^(?:\d{1,3}\.){3}\d{1,3}$/.test(address.value) && [port.value,bandwidth.value,quota.value,connections.value,duration.value].every(Number.isInteger) && port.value >= 1024 && port.value <= 65535 && bandwidth.value >= 1 && bandwidth.value <= 102400 && quota.value >= 1 && quota.value <= 1048576 && connections.value >= 2 && connections.value <= 16 && duration.value >= 1 && duration.value <= 1440 && (access.value !== 'private' || peers.value.trim().split(/\s+/).length >= 2));
let disposed = false, timer: ReturnType<typeof setInterval> | undefined;
async function refresh() { if (busy.value || !open.value) return; try { const result = await invoke<{ node: MeshNodeSnapshot | null }>('mesh_node_status'); if (!disposed) node.value = result?.node ?? null; } catch { if (!disposed) error.value = true; } }
function expanded(value: boolean) { open.value = value; if (timer) clearInterval(timer); if (value) { void refresh(); timer = setInterval(() => { void refresh(); }, 3000); } }
async function start() {
  if (busy.value || running.value || !valid.value) return;
  busy.value = true; error.value = false; copied.value = false;
  const config = { advertise_ipv4:address.value, port:port.value, policy:access.value === 'community' ? { role:'community_relay' } : { role:'private_friends', allowed_peers:peers.value.trim().split(/\s+/) }, limits:{ bandwidth_bytes_per_second:bandwidth.value*1024, max_bytes:quota.value*1024*1024, max_connections:connections.value, duration_seconds:duration.value*60 } };
  try { const result = await invoke<{ node: MeshNodeSnapshot }>('mesh_node_start', { config }); if (!disposed) node.value = result.node; }
  catch { if (!disposed) error.value = true; }
  finally { if (!disposed) busy.value = false; }
}
async function stop() { if (busy.value) return; busy.value = true; error.value = false; try { const result = await invoke<{ node: MeshNodeSnapshot }>('mesh_node_stop'); if (!disposed) node.value = result.node; } catch { if (!disposed) error.value = true; } finally { if (!disposed) busy.value = false; } }
async function copy() { if (!node.value) return; try { await writeText(node.value.endpoint); copied.value = true; } catch { error.value = true; } }
onUnmounted(() => { disposed = true; if (timer) clearInterval(timer); });
</script>
<template>
  <SettingsDisclosure
    :title="t('mesh.node.title')"
    icon="network"
    :value="node ? t(`mesh.node.${node.state}`) : undefined"
    @update:open="expanded"
  >
    <p
      v-if="node"
      role="status"
    >
      {{ t(`mesh.node.${node.state}`) }}
    </p>
    <template v-if="running">
      <p>{{ t('mesh.node.usage', { used: ((node?.transport_bytes ?? 0)/1048576).toFixed(1), limit: ((node?.limits.max_bytes ?? 0)/1048576).toFixed(0) }) }}</p>
      <div class="mesh-node-actions">
        <button
          class="text-button"
          @click="copy"
        >
          <AppIcon :name="copied ? 'check' : 'copy'" />{{ t(copied ? 'mesh.node.copied' : 'mesh.node.copy') }}
        </button><button
          class="text-button"
          :disabled="busy"
          @click="stop"
        >
          <AppIcon name="x" />{{ t('mesh.node.stop') }}
        </button>
      </div>
    </template>
    <form
      v-else-if="open"
      class="mesh-node-form"
      @submit.prevent="start"
    >
      <label for="mesh-node-access">{{ t('mesh.node.access') }}</label>
      <SelectField
        id="mesh-node-access"
        v-model="access"
        :label="t('mesh.node.access')"
        :options="(['private','community'] as const).map(value => ({ value, label:t(`mesh.node.${value}`) }))"
        :disabled="busy"
      />
      <label>{{ t('mesh.node.address') }}<input
        v-model.trim="address"
        :disabled="busy"
        inputmode="decimal"
        required
      ></label>
      <label>{{ t('mesh.node.port') }}<input
        v-model.number="port"
        type="number"
        min="1024"
        max="65535"
        :disabled="busy"
        required
      ></label>
      <label v-if="access === 'private'">{{ t('mesh.node.peers') }}<textarea
        v-model="peers"
        :disabled="busy"
        rows="3"
        spellcheck="false"
        required
      /></label>
      <div class="mesh-node-grid">
        <label>{{ t('mesh.node.bandwidth') }}<input
          v-model.number="bandwidth"
          type="number"
          min="1"
          max="102400"
          :disabled="busy"
          required
        ></label>
        <label>{{ t('mesh.node.quota') }}<input
          v-model.number="quota"
          type="number"
          min="1"
          max="1048576"
          :disabled="busy"
          required
        ></label>
        <label>{{ t('mesh.node.connections') }}<input
          v-model.number="connections"
          type="number"
          min="2"
          max="16"
          :disabled="busy"
          required
        ></label>
        <label>{{ t('mesh.node.duration') }}<input
          v-model.number="duration"
          type="number"
          min="1"
          max="1440"
          :disabled="busy"
          required
        ></label>
      </div>
      <p class="setting-hint">
        {{ t('mesh.node.warning') }}
      </p>
      <button
        class="secondary-button"
        type="submit"
        :disabled="busy || !valid"
      >
        <AppIcon name="network" />{{ t('mesh.node.start') }}
      </button>
    </form>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ t('mesh.node.error') }}
    </p>
  </SettingsDisclosure>
</template>
<style scoped>
.mesh-node-form{display:grid;gap:12px;font-size:14px}.mesh-node-form label{display:grid;gap:8px}.mesh-node-form input,.mesh-node-form textarea{width:100%;min-width:0;box-sizing:border-box;padding:10px 12px;border:1px solid var(--line);border-radius:var(--ui-radius,12px);background:var(--bg);color:var(--ink);font:inherit}.mesh-node-form textarea{resize:vertical}.mesh-node-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}.mesh-node-form button{justify-self:start}.mesh-node-actions{display:flex;gap:8px;flex-wrap:wrap}.setting-hint{font-size:12px;color:var(--muted);margin:0}@media(max-width:560px){.mesh-node-grid{grid-template-columns:1fr}}
</style>
<style scoped>
.mesh-node-actions button,.mesh-node-form>button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.mesh-node-grid{padding-top:var(--ui-space-4);border-top:1px solid var(--ui-border)}.mesh-node-form>button{border:1px solid var(--ui-border);background:var(--ui-surface-1);border-radius:var(--ui-radius-md);padding:var(--ui-space-2) var(--ui-space-3)}
</style>
