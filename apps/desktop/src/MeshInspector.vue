<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { t } from './i18n';
import type { MeshOverview } from './mesh';
import type { MeshAppSnapshot, MeshMessagePage } from './mesh-social';
import { meshDiagnostics, messageMetadata, type MeshEventMetadata } from './mesh-inspector';
const props = defineProps<{ active: boolean }>();
defineEmits<{ close: [] }>();
const overview = ref<MeshOverview | null>(null); const app = ref<MeshAppSnapshot | null>(null);
const events = ref<MeshEventMetadata[]>([]); const readAt = ref<number | null>(null);
const busy = ref(false); const error = ref(''); const copied = ref(false); const hasMore = ref(false);
const restarted = ref(false); let generation = ''; let cursor = 0; let disposed = false;
const aliases = new Map<string, number>();
const connectedContacts = computed(() => app.value?.contacts.filter(contact => contact.connected).length ?? 0);
const session = computed(() => overview.value?.session);
function state(value?: string) { return value ? t(`meshInspector.state_${value}`) : t('meshInspector.unknown'); }
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
  const results = await Promise.allSettled([invoke<MeshOverview>('mesh_overview'), invoke<MeshAppSnapshot | null>('mesh_app_snapshot')]);
  if (disposed) return;
  if (results[0].status === 'fulfilled') overview.value = results[0].value; else { overview.value = null; error.value = t('meshInspector.refreshError'); }
  if (results[1].status === 'fulfilled') app.value = results[1].value; else { app.value = null; error.value = t('meshInspector.refreshError'); }
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
  try { await writeText(JSON.stringify(meshDiagnostics(overview.value, app.value, events.value, readAt.value), null, 2)); copied.value = true; }
  catch { error.value = t('meshInspector.copyError'); }
}
watch(() => props.active, active => { if (active) void refresh(); }, { immediate: true });
onUnmounted(() => { disposed = true; aliases.clear(); events.value = []; });
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
        {{ t('meshInspector.back') }}
      </button>
      <h2>{{ t('meshInspector.title') }}</h2>
      <button
        class="text-button"
        :disabled="busy"
        @click="refresh"
      >
        {{ t(busy ? 'meshInspector.loading' : 'meshInspector.refresh') }}
      </button>
      <button
        class="text-button"
        :disabled="busy || !readAt"
        @click="copy"
      >
        {{ t('meshInspector.copy') }}
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
    <section>
      <h3>{{ t('meshInspector.local') }}</h3>
      <dl class="inspector-metrics">
        <div><dt>{{ t('meshInspector.component') }}</dt><dd>{{ state(overview?.component_state) }}</dd></div>
        <div><dt>{{ t('meshInspector.contactsService') }}</dt><dd>{{ t(app ? 'meshInspector.active' : 'meshInspector.inactive') }}</dd></div>
        <div><dt>{{ t('meshInspector.neighbors') }}</dt><dd>{{ overview?.neighbors.length ?? 0 }}</dd></div>
        <div><dt>{{ t('meshInspector.contacts') }}</dt><dd>{{ connectedContacts }}</dd></div>
        <div><dt>{{ t('meshInspector.advertisements') }}</dt><dd>{{ overview?.nodes.length ?? 0 }}</dd></div>
        <div><dt>{{ t('meshInspector.discovery') }}</dt><dd>{{ state(overview?.discovery_state) }}</dd></div>
      </dl>
    </section>
    <section>
      <h3>{{ t('meshInspector.route') }}</h3>
      <template v-if="session">
        <p>{{ session.state === 'listening' ? t('meshInspector.reserved') : session.state === 'bridge_ready' ? t('meshInspector.bridgeReady') : state(session.state) }} · <code>{{ session.route }}</code></p>
        <dl class="inspector-metrics">
          <div><dt>{{ t('meshInspector.role') }}</dt><dd>{{ state(session.role) }}</dd></div>
          <div><dt>{{ t('meshInspector.target') }}</dt><dd>{{ state(session.target) }}</dd></div>
          <div v-if="session.local_port">
            <dt>{{ t('meshInspector.port') }}</dt><dd>{{ session.local_port }}</dd>
          </div>
          <div><dt>{{ t('meshInspector.expires') }}</dt><dd>{{ time(session.expires_at) }}</dd></div>
        </dl>
        <p class="setting-hint">
          {{ t('meshInspector.noTrafficClaim') }}
        </p>
      </template>
      <p v-else>
        {{ t('meshInspector.noSession') }}
      </p>
      <dl class="inspector-capabilities">
        <div
          v-for="[name, value] in [['relay',overview?.capabilities.relay],['direct',overview?.capabilities.direct],['holePunch',overview?.capabilities.hole_punch]]"
          :key="String(name)"
        >
          <dt>{{ t(`meshInspector.${name}`) }}</dt><dd>{{ t(value ? 'meshInspector.yes' : 'meshInspector.no') }}</dd>
        </div>
      </dl>
    </section>
    <section>
      <h3>{{ t('meshInspector.services') }}</h3>
      <div
        v-if="overview?.configured_services?.length"
        class="inspector-table-scroll inspector-services"
      >
        <table>
          <thead><tr><th>{{ t('meshInspector.service') }}</th><th>{{ t('meshInspector.connection') }}</th><th>{{ t('meshInspector.discovery') }}</th><th>{{ t('meshInspector.gameRelay') }}</th></tr></thead><tbody>
            <tr
              v-for="service in overview.configured_services"
              :key="service.id"
            >
              <td>{{ service.id }}<code>{{ service.endpoint }}</code></td><td>{{ state(service.state) }}</td><td>{{ state(service.discovery) }}</td><td>{{ t(service.game_relay ? 'meshInspector.yes' : 'meshInspector.no') }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else>
        {{ t('meshInspector.noServices') }}
      </p>
    </section>
    <section>
      <h3>{{ t('meshInspector.nodes') }}</h3>
      <p class="setting-hint">
        {{ t('meshInspector.nodeEvidence') }}
      </p>
      <ul
        v-if="overview?.nodes.length"
        class="inspector-nodes"
      >
        <li
          v-for="(node,index) in overview.nodes"
          :key="node.peer_id"
        >
          <span>#{{ index + 1 }}</span><code>{{ node.endpoint }}</code><span>{{ t('meshInspector.expires') }} {{ time(node.expires_at) }}</span>
        </li>
      </ul>
      <p v-else>
        {{ t('meshInspector.noNodes') }}
      </p>
    </section>
    <section>
      <h3>{{ t('meshInspector.events') }}</h3>
      <p class="setting-hint">
        {{ t('meshInspector.eventHint') }}
      </p>
      <p
        v-if="restarted"
        role="status"
      >
        {{ t('meshInspector.sessionChanged') }}
      </p>
      <div
        v-if="events.length"
        class="inspector-table-scroll"
      >
        <table>
          <thead><tr><th>#</th><th>{{ t('meshInspector.time') }}</th><th>{{ t('meshInspector.direction') }}</th><th>{{ t('meshInspector.peer') }}</th><th>{{ t('meshInspector.bytes') }}</th></tr></thead><tbody>
            <tr
              v-for="event in events"
              :key="event.sequence"
            >
              <td>{{ event.sequence }}</td><td>{{ time(event.at) }}</td><td>{{ t(`meshInspector.${event.direction}`) }}</td><td>#{{ event.contact }}</td><td>{{ event.bytes }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else>
        {{ t(app ? 'meshInspector.noEvents' : 'meshInspector.contactsStopped') }}
      </p>
      <button
        v-if="hasMore"
        class="text-button"
        :disabled="busy"
        @click="more"
      >
        {{ t('meshInspector.more') }}
      </button>
    </section>
  </section>
</template>
<style scoped>
.mesh-inspector{display:grid;gap:20px;min-width:0}.inspector-toolbar{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.inspector-toolbar h2{margin:0 auto 0 0;font-size:18px}.mesh-inspector section{border-top:1px solid var(--line);padding-top:16px;min-width:0}.mesh-inspector h3{font-size:14px;margin:0 0 12px}.inspector-metrics{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:14px;margin:0}.inspector-metrics div{display:grid;gap:6px}.mesh-inspector dt{color:var(--muted)}.mesh-inspector dd{margin:0}.inspector-capabilities{display:flex;flex-wrap:wrap;gap:24px}.inspector-capabilities div{display:grid;gap:6px}.inspector-table-scroll{overflow:auto;margin-top:12px}table{width:100%;border-collapse:collapse;text-align:left;font-size:13px}th,td{padding:10px;border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-weight:500}td code{display:block;max-width:420px;margin-top:4px;overflow-wrap:anywhere;font-size:11px}.inspector-nodes{padding:0;list-style:none}.inspector-nodes li{display:flex;flex-wrap:wrap;gap:12px;padding:10px 0}.inspector-nodes code{flex:1;min-width:180px;overflow-wrap:anywhere}.setting-hint{margin:0;color:var(--muted);font-size:12px}.text-button{min-height:36px}
.inspector-services table{table-layout:fixed}.inspector-services th:first-child{width:46%}.inspector-services th:not(:first-child){width:18%}.inspector-services th:not(:first-child),.inspector-services td:not(:first-child){white-space:nowrap}.inspector-services code{font-size:12px;overflow-wrap:anywhere}.inspector-services th,.inspector-services td{padding:10px 6px}
</style>
