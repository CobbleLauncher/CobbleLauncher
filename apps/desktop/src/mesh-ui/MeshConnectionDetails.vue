<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { MeshOverview, MeshSession } from '../mesh';
import { t } from '../i18n';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
defineProps<{ defaultOpen?: boolean; error?: string; overview: MeshOverview | null; session: MeshSession | null; relay: string; port: string; busy: boolean; discovering: boolean }>();
defineEmits<{ 'update:relay': [value: string]; 'update:port': [value: string]; discover: []; refresh: []; inspector: [] }>();
</script>
<template>
  <SettingsDisclosure
    class="mesh-details"
    :title="t('mesh.details')"
    :default-open="defaultOpen"
  >
    <button
      class="text-button"
      @click="$emit('inspector')"
    >
      {{ t('meshInspector.open') }}
    </button>
    <p
      v-if="error"
      class="settings-error"
    >
      {{ error }}
    </p>
    <dl>
      <div><dt>{{ t('mesh.connection') }}</dt><dd>{{ session ? t(`mesh.${session.state}`) : t('mesh.idle') }}</dd></div><div v-if="session">
        <dt>{{ t('mesh.route') }}</dt><dd>{{ t('mesh.capability_relay') }}</dd>
      </div>
    </dl>
    <label>{{ t('mesh.port') }}<input
      type="number"
      min="1024"
      max="65535"
      :value="port"
      :disabled="busy"
      @input="$emit('update:port', ($event.target as HTMLInputElement).value)"
    ></label>
    <label>{{ t('mesh.relay') }}<input
      :value="relay"
      :disabled="busy"
      @input="$emit('update:relay', ($event.target as HTMLInputElement).value)"
    ></label>
    <template v-if="overview">
      <dl>
        <div
          v-for="capability in (['relay', 'direct', 'hole_punch'] as const)"
          :key="capability"
        >
          <dt>{{ t(`mesh.capability_${capability}`) }}</dt><dd>{{ t(overview.capabilities[capability] ? 'mesh.available' : 'mesh.notAvailable') }}</dd>
        </div>
      </dl>
      <ul v-if="overview.configured_services?.length || overview.nodes.length">
        <li
          v-for="service in overview.configured_services"
          :key="service.id"
        >
          {{ t(service.game_relay ? 'mesh.officialRelay' : 'mesh.officialConnection') }} · {{ t(`mesh.service_${service.state}`) }}
        </li><li
          v-for="node in overview.nodes"
          :key="node.peer_id"
        >
          <code>{{ node.endpoint }}</code><button
            v-if="node.relay"
            class="text-button"
            :disabled="busy"
            @click="$emit('update:relay', node.endpoint)"
          >
            {{ t('mesh.useRelay') }}
          </button>
        </li>
      </ul>
    </template>
    <button
      class="text-button"
      :disabled="busy || discovering"
      @click="$emit('discover')"
    >
      {{ t(discovering ? 'mesh.findingNodes' : 'mesh.findNodes') }}
    </button><button
      class="text-button"
      :disabled="busy"
      @click="$emit('refresh')"
    >
      {{ t('mesh.refresh') }}
    </button>
  </SettingsDisclosure>
</template>
<style scoped>
.mesh-details{margin-top:18px;padding:16px;border:1px solid var(--line);border-radius:12px}.mesh-details summary{cursor:pointer}.mesh-details label{display:grid;gap:8px;margin:16px 0}.mesh-details input{min-width:0;width:100%;box-sizing:border-box;padding:10px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--ink)}dl>div{display:flex;justify-content:space-between;gap:16px;margin:10px 0}dd{margin:0}code{overflow-wrap:anywhere}ul{padding-left:20px}
</style>
