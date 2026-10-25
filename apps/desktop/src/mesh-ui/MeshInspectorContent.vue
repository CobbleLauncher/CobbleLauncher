<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { t } from '../i18n';
import AppIcon from '../ui/AppIcon.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import MeshEvidenceBadge from './MeshEvidenceBadge.vue';
import type { MeshOverview,MeshNetworkSnapshot,MeshNodeSnapshot } from '../mesh';
import type { MeshAppSnapshot } from '../mesh-social';
import type { MeshEventMetadata } from '../mesh-inspector';
const props=defineProps<{overview:MeshOverview|null;network:MeshNetworkSnapshot|null;app:MeshAppSnapshot|null;localNode:MeshNodeSnapshot|null;events:MeshEventMetadata[];readAt:number|null;busy:boolean;hasMore:boolean;restarted:boolean}>();
defineEmits<{more:[]}>();
const session=computed(()=>props.overview?.session);
const nodes=computed(()=>(props.overview?.nodes??[]).filter(node=>node.expires_at>(props.readAt??Date.now()/1000)));
const services=computed(()=>props.network?.services??[]);
const contacts=computed(()=>props.app?.contacts.filter(contact=>contact.connected).length??0);
const route=computed(()=>props.network?.session_evidence?.selected_route??'unknown');
const state=(value?:string)=>value?t(`meshInspector.state_${value}`):t('meshInspector.unknown');
const time=(value?:number|null)=>value?new Date(value*1000).toLocaleString():t('meshInspector.unknown');
function age(value:number|null){if(!value)return t('meshInspector.notTested');const seconds=Math.max(0,Math.floor((props.readAt??Date.now()/1000)-value));return seconds<60?t('meshInspector.secondsAgo',{count:seconds}):t('meshInspector.minutesAgo',{count:Math.floor(seconds/60)});}
</script>
<template>
  <div class="inspector-overview">
    <section class="inspector-section">
      <h3><AppIcon name="monitor" />{{ t('meshInspector.local') }}</h3>
      <dl class="inspector-metrics">
        <div>
          <dt>{{ t('meshInspector.network') }}</dt><dd>
            <MeshEvidenceBadge
              :value="network?.phase"
              :label="network?t(`mesh.networkStatus.${network.phase}`):t('meshInspector.unknown')"
            />
          </dd>
        </div>
        <div><dt>{{ t('meshInspector.component') }}</dt><dd>{{ state(overview?.component_state) }}</dd></div>
        <div><dt>{{ t('meshInspector.contactsService') }}</dt><dd>{{ t(app?'meshInspector.active':'meshInspector.inactive') }}</dd></div>
        <div><dt>{{ t('meshInspector.neighbors') }}</dt><dd>{{ overview?.neighbors.length??0 }}</dd></div>
        <div><dt>{{ t('meshInspector.contacts') }}</dt><dd>{{ contacts }}</dd></div>
        <div><dt>{{ t('meshInspector.advertisements') }}</dt><dd>{{ nodes.length }}</dd></div>
        <div><dt>{{ t('meshInspector.discovery') }}</dt><dd>{{ state(overview?.discovery_state) }}</dd></div>
      </dl>
      <p
        v-if="network?.phase==='failed'&&network.last_error"
        class="inspector-caption"
      >
        {{ t('meshInspector.networkError') }}: {{ network.last_error }}
      </p>
    </section>
    <section class="inspector-section">
      <h3><AppIcon name="network" />{{ t('meshInspector.route') }}</h3>
      <div class="route-verdict">
        <AppIcon :name="route==='direct'?'check':'network'" /><strong>{{ route!=='unknown'?t(`meshInspector.route_${route}`):t('meshInspector.noActualRoute') }}</strong>
      </div>
      <p
        v-if="session"
        class="inspector-caption"
      >
        {{ session.state==='listening'?t('meshInspector.reserved'):session.state==='bridge_ready'?t('meshInspector.bridgeReady'):state(session.state) }}
      </p>
      <p
        v-else-if="!network?.session_evidence"
        class="inspector-caption"
      >
        {{ t('meshInspector.noSession') }}
      </p>
      <dl
        v-if="session"
        class="inspector-session"
      >
        <div><dt>{{ t('meshInspector.role') }}</dt><dd>{{ state(session.role) }}</dd></div>
        <div><dt>{{ t('meshInspector.target') }}</dt><dd>{{ state(session.target) }}</dd></div>
        <div v-if="session.local_port">
          <dt>{{ t('meshInspector.port') }}</dt><dd>{{ session.local_port }}</dd>
        </div>
        <div><dt>{{ t('meshInspector.expires') }}</dt><dd>{{ time(session.expires_at) }}</dd></div>
      </dl>
      <dl
        v-if="network?.session_evidence"
        class="inspector-traffic"
      >
        <div><dt>{{ t('meshInspector.bytesSent') }}</dt><dd>{{ network.session_evidence.bytes_sent }}</dd></div>
        <div><dt>{{ t('meshInspector.bytesReceived') }}</dt><dd>{{ network.session_evidence.bytes_received }}</dd></div>
        <div><dt>{{ t('meshInspector.activeStreams') }}</dt><dd>{{ network.session_evidence.active_streams }}</dd></div>
      </dl>
      <code
        v-if="network?.session_evidence"
        class="inspector-source"
      >{{ network.session_evidence.peer_scope }}<template v-if="network.session_evidence.selected_connection"> · {{ network.session_evidence.selected_connection }}</template></code>
      <p
        v-if="session&&!network?.session_evidence"
        class="inspector-caption"
      >
        {{ t('meshInspector.noTrafficClaim') }}
      </p>
      <p class="inspector-caption relay-evidence">
        {{ t('meshInspector.relayEvidence') }}: <MeshEvidenceBadge
          :value="network?.phase==='online'&&network.relay_available?'verified':'not_tested'"
          :label="network?.phase==='online'&&network.relay_available?t('meshInspector.relayNegotiated'):t('meshInspector.noRelayEvidence')"
        />
      </p>
    </section>
  </div>
  <section class="inspector-section">
    <h3><AppIcon name="shield" />{{ t('meshInspector.checks') }}</h3>
    <div
      v-if="network?.checks?.length"
      class="inspector-table-scroll"
    >
      <table>
        <thead><tr><th>{{ t('meshInspector.method') }}</th><th>{{ t('meshInspector.result') }}</th><th>{{ t('meshInspector.evidence') }}</th></tr></thead><tbody>
          <tr
            v-for="check in network.checks"
            :key="`${check.capability}:${check.peer_scope}`"
          >
            <th scope="row">
              {{ t(`meshInspector.${check.capability}`) }}
            </th><td>
              <MeshEvidenceBadge
                :value="check.state"
                :label="t(`meshInspector.check_${check.state}`)"
              />
            </td><td>
              <template v-if="check.observed_at">
                <span class="inspector-caption">{{ age(check.observed_at) }}</span><code>{{ check.stage }} · {{ check.peer_scope }}</code>
              </template><template v-else>
                —
              </template><code v-if="check.error_code">{{ check.error_code }}</code>
            </td>
          </tr>
        </tbody>
      </table>
    </div><p
      v-else
      class="inspector-caption"
    >
      {{ t('meshInspector.noChecks') }}
    </p>
  </section>
  <SettingsDisclosure
    v-if="localNode"
    :title="t('mesh.node.title')"
    icon="network"
    :value="t(`mesh.node.${localNode.state}`)"
  >
    <dl class="inspector-metrics">
      <div><dt>{{ t('mesh.node.access') }}</dt><dd>{{ t(`mesh.node.${localNode.access}`) }}</dd></div><div><dt>{{ t('mesh.node.transportBytes') }}</dt><dd>{{ localNode.transport_bytes }} / {{ localNode.limits.max_bytes }}</dd></div><div><dt>{{ t('mesh.node.connections') }}</dt><dd>{{ localNode.connections }} / {{ localNode.limits.max_connections }}</dd></div><div><dt>{{ t('mesh.node.allowedCount') }}</dt><dd>{{ localNode.allowed_peer_count }}</dd></div><div><dt>{{ t('meshInspector.expires') }}</dt><dd>{{ time(localNode.expires_at) }}</dd></div>
    </dl>
  </SettingsDisclosure>
  <SettingsDisclosure
    :title="t('meshInspector.services')"
    icon="network"
    :value="String(services.length)"
    :default-open="true"
  >
    <div
      v-if="services.length"
      class="inspector-table-scroll"
    >
      <table class="inspector-services">
        <thead><tr><th>{{ t('meshInspector.service') }}</th><th>{{ t('meshInspector.connection') }}</th><th>{{ t('meshInspector.discovery') }}</th><th>{{ t('meshInspector.gameRelay') }}</th></tr></thead><tbody>
          <tr
            v-for="service in services"
            :key="service.id"
          >
            <th scope="row">
              {{ service.id }}<code>{{ service.endpoint }}</code>
            </th><td>
              <MeshEvidenceBadge
                :value="service.state"
                :label="state(service.state)"
              />
            </td><td>{{ state(service.discovery) }}</td><td>
              <MeshEvidenceBadge
                :value="service.game_relay?'available':'not_checked'"
                :label="t(service.game_relay?'meshInspector.yes':'meshInspector.no')"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </div><p
      v-else
      class="inspector-caption"
    >
      {{ t('meshInspector.noServices') }}
    </p>
  </SettingsDisclosure>
  <SettingsDisclosure
    :title="t('meshInspector.nodes')"
    icon="search"
    :value="String(nodes.length)"
  >
    <p class="inspector-caption">
      {{ t('meshInspector.nodeEvidence') }}
    </p>
    <div
      v-if="nodes.length"
      class="inspector-table-scroll"
    >
      <table>
        <thead><tr><th>#</th><th>{{ t('meshInspector.endpoint') }}</th><th>{{ t('meshInspector.expires') }}</th></tr></thead><tbody>
          <tr
            v-for="(node,index) in nodes"
            :key="node.peer_id"
          >
            <td>{{ index+1 }}</td><td><code>{{ node.endpoint }}</code><span class="inspector-caption">{{ t('meshInspector.signedEvidence') }}</span></td><td>{{ time(node.expires_at) }}</td>
          </tr>
        </tbody>
      </table>
    </div><p
      v-else
      class="inspector-caption"
    >
      {{ t('meshInspector.noNodes') }}
    </p>
  </SettingsDisclosure>
  <SettingsDisclosure
    :title="t('meshInspector.events')"
    icon="file-search"
    :value="String(events.length)"
  >
    <p class="inspector-caption">
      {{ t('meshInspector.eventHint') }}
    </p><p
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
    </div><p
      v-else
      class="inspector-caption"
    >
      {{ t(app?'meshInspector.noEvents':'meshInspector.contactsStopped') }}
    </p>
    <button
      v-if="hasMore"
      class="text-button"
      :disabled="busy"
      @click="$emit('more')"
    >
      <AppIcon name="chevron-right" />{{ t('meshInspector.more') }}
    </button>
  </SettingsDisclosure>
</template>
<style scoped>
.inspector-overview{display:grid;grid-template-columns:minmax(230px,1fr) minmax(0,1.35fr);gap:var(--ui-space-6)}.inspector-section{min-width:0;border-top:1px solid var(--ui-border);padding-top:var(--ui-space-4)}h3{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-label);font-weight:var(--ui-weight-semibold);margin:0 0 var(--ui-space-4)}h3 svg{width:20px;height:20px;color:var(--ui-text-muted)}.inspector-metrics{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--ui-space-4);margin:0}.inspector-metrics>div{display:grid;gap:var(--ui-space-1)}dt{font-size:var(--ui-text-caption);color:var(--ui-text-muted)}dd{margin:0;font-size:var(--ui-text-body);font-weight:var(--ui-weight-medium);overflow-wrap:anywhere;font-variant-numeric:tabular-nums}.inspector-caption{margin:var(--ui-space-2) 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);overflow-wrap:anywhere}.route-verdict{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-section);line-height:var(--ui-leading-section)}.route-verdict>svg{width:20px;height:20px;color:var(--ui-info)}.inspector-session{display:flex;gap:var(--ui-space-4);flex-wrap:wrap;margin:var(--ui-space-4) 0}.inspector-session dd{font-size:var(--ui-text-caption)}.inspector-traffic{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:var(--ui-space-3);margin:var(--ui-space-4) 0}.inspector-traffic dd{font-family:var(--ui-font-mono);font-size:var(--ui-text-section)}.inspector-source{display:block;color:var(--ui-text-muted);font:var(--ui-text-caption)/1.5 var(--ui-font-mono);overflow-wrap:anywhere}.relay-evidence{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap;padding-top:var(--ui-space-3);border-top:1px solid var(--ui-border)}.inspector-table-scroll{max-width:100%;overflow:auto;min-width:0;margin-top:var(--ui-space-3)}table{width:100%;min-width:460px;border-collapse:collapse;text-align:start;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}th,td{text-align:start;padding:var(--ui-space-3);border-bottom:1px solid var(--ui-border);vertical-align:top}thead th{background:var(--ui-surface-2);color:var(--ui-text-muted);font-weight:var(--ui-weight-medium)}tbody th{font-weight:var(--ui-weight-medium)}td code,th code{display:block;max-width:420px;min-width:0;margin-top:var(--ui-space-1);overflow-wrap:anywhere;white-space:normal;color:var(--ui-text-muted);font:var(--ui-text-caption)/1.6 var(--ui-font-mono)}.inspector-services{min-width:680px}.inspector-services th:first-child{width:46%;min-width:280px}.inspector-services td{white-space:nowrap}button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}@media(max-width:760px){.inspector-overview{grid-template-columns:minmax(0,1fr);gap:var(--ui-space-5)}}
</style>
