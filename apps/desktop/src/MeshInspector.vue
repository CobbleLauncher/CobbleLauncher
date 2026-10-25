<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import AppIcon from './ui/AppIcon.vue';
import MeshInspectorContent from './mesh-ui/MeshInspectorContent.vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import type { MeshNetworkSnapshot, MeshOverview, MeshNodeSnapshot } from './mesh';
import type { MeshAppSnapshot, MeshMessagePage } from './mesh-social';
import { meshDiagnostics, messageMetadata, type MeshEventMetadata } from './mesh-inspector';
const props = defineProps<{ active: boolean }>();
defineEmits<{ close: [] }>();
const overview = ref<MeshOverview | null>(null); const app = ref<MeshAppSnapshot | null>(null); const network = ref<MeshNetworkSnapshot | null>(null);
const localNode = ref<MeshNodeSnapshot | null>(null);
const events = ref<MeshEventMetadata[]>([]); const readAt = ref<number | null>(null);
const busy = ref(false); const error = ref(''); const copied = ref(false); const hasMore = ref(false);
const restarted = ref(false); let generation = ''; let cursor = 0; let disposed = false;
let eventSerial = 0; let listenToken = 0; let unlisten: (() => void) | null = null;
const aliases = new Map<string, number>();
function time(value?: number | null) { return value ? new Date(value * 1000).toLocaleString() : t('meshInspector.unknown'); }
async function loadEvents(append: boolean) {
  const page = await invoke<MeshMessagePage>('mesh_list_messages', { after: append ? cursor : 0, limit: 40 });
  if (disposed) return;
  const changed = generation !== '' && generation !== page.generation;
  if (changed) { aliases.clear(); events.value = []; restarted.value = true; }
  generation = page.generation;
  if (append && changed) { cursor = 0; await loadEvents(false); return; }
  const metadata = messageMetadata(page.messages, aliases);
  events.value = (append ? [...events.value, ...metadata] : metadata).slice(-160);
  cursor = page.next_after; hasMore.value = page.has_more;
}
async function refresh() {
  if (busy.value || !props.active) return;
  busy.value = true; error.value = ''; copied.value = false; restarted.value = false;
  const readSerial = eventSerial;
  const results = await Promise.allSettled([invoke<MeshOverview>('mesh_overview'), invoke<MeshAppSnapshot | null>('mesh_app_snapshot'), invoke<MeshNetworkSnapshot>('mesh_network_snapshot'), invoke<{node:MeshNodeSnapshot | null}>('mesh_node_status')]);
  if (disposed || !props.active) return;
  if (results[0].status === 'fulfilled') overview.value = results[0].value; else { overview.value = null; error.value = t('meshInspector.refreshError'); }
  if (results[1].status === 'fulfilled') app.value = results[1].value; else { app.value = null; error.value = t('meshInspector.refreshError'); }
  if (results[3].status === 'fulfilled') localNode.value = results[3].value?.node ?? null;
  if (readSerial === eventSerial) {
    if (results[2].status === 'fulfilled' && results[2].value.generation >= (network.value?.generation ?? 0)) network.value = results[2].value;
    else if (results[2].status === 'rejected') { network.value = null; error.value = t('meshInspector.refreshError'); }
  }
  try {
    if (app.value) await loadEvents(false); else { events.value = []; aliases.clear(); generation = ''; cursor = 0; hasMore.value = false; }
  } catch { error.value = t('meshInspector.refreshError'); events.value = []; hasMore.value = false; }
  if (!disposed) { readAt.value = Math.floor(Date.now() / 1000); busy.value = false; }
}
async function more() {
  if (busy.value || !props.active || !app.value) return;
  busy.value = true;
  try { await loadEvents(true); } catch { error.value = t('meshInspector.refreshError'); }
  finally { busy.value = false; }
}
async function copy() {
  try { await writeText(JSON.stringify(meshDiagnostics(overview.value, app.value, events.value, readAt.value, network.value, localNode.value), null, 2)); copied.value = true; }
  catch { error.value = t('meshInspector.copyError'); }
}
watch(() => props.active, active => {
  const token = ++listenToken;
  unlisten?.(); unlisten = null;
  if (!active) { busy.value = false; return; }
  void listen<MeshNetworkSnapshot>('mesh-network-changed', event => {
    const next = event.payload;
    if (disposed || !props.active || token !== listenToken || next.generation < (network.value?.generation ?? 0)) return;
    eventSerial += 1; network.value = next; readAt.value = Math.floor(Date.now() / 1000);
  }).then(stop => { if (disposed || !props.active || token !== listenToken) stop(); else unlisten = stop; }).catch(() => {});
  void refresh();
}, { immediate: true });
onUnmounted(() => { disposed = true; listenToken += 1; unlisten?.(); aliases.clear(); events.value = []; });
</script>
<template>
  <section
    class="mesh-inspector"
    :aria-label="t('meshInspector.title')"
  >
    <header class="inspector-toolbar">
      <button
        class="text-button"
        @click="$emit('close')"
      >
        <AppIcon name="arrow-left" />{{ t('meshInspector.back') }}
      </button>
      <h2>{{ t('meshInspector.title') }}</h2>
      <button
        class="text-button"
        :disabled="busy"
        @click="refresh"
      >
        <AppIcon :name="busy ? 'loader-circle' : 'refresh-cw'" />{{ t(busy ? 'meshInspector.loading' : 'meshInspector.refresh') }}
      </button>
      <button
        class="text-button"
        :disabled="busy || !readAt"
        @click="copy"
      >
        <AppIcon name="copy" />{{ t('meshInspector.copy') }}
      </button>
    </header>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ error }}
    </p>
    <p
      v-if="copied"
      role="status"
    >
      {{ t('meshInspector.copied') }}
    </p>
    <p
      v-if="readAt"
      class="setting-hint"
    >
      {{ t('meshInspector.snapshotAt') }} {{ time(readAt) }}
    </p>
    <MeshInspectorContent
      :overview="overview"
      :network="network"
      :app="app"
      :local-node="localNode"
      :events="events"
      :read-at="readAt"
      :busy="busy"
      :has-more="hasMore"
      :restarted="restarted"
      @more="more"
    />
  </section>
</template>
<style scoped>
.mesh-inspector{display:grid;gap:var(--ui-space-5);min-width:0}.inspector-toolbar{display:flex;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap}.inspector-toolbar h2{margin:0 auto 0 0;font-size:var(--ui-text-section);line-height:var(--ui-leading-section)}.inspector-toolbar button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.setting-hint{margin:0;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}
</style>
