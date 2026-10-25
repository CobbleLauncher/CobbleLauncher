<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, toRef, watch } from 'vue';
import MeshEnvironmentPanel from '../mesh-environment/MeshEnvironmentPanel.vue';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import AppIcon from '../ui/AppIcon.vue';
import MeshGroupCard from './MeshGroupCard.vue';
import MeshGroupSetup from './MeshGroupSetup.vue';
import { groupText as text } from './labels';
import { groupActive } from './types';
import { useMeshGroup } from './use-mesh-group';
const props = defineProps<{ gameId: string; gameName: string; running: boolean; active: boolean; initialInvitation?: string }>();
const emit = defineEmits<{ busy: [value: boolean]; close: []; launch: [gameId: string]; created: [gameId: string]; prepared: [value:{gameId:string;invitation:string}] }>();
const { group, hostOptions, names, selfPeerId, invitation, address, copied, loading, busy: groupBusy, error, refresh, create, join, copy, invite, revoke, kick, stop } = useMeshGroup({ gameId: toRef(props, 'gameId'), running: toRef(props, 'running'), active: toRef(props, 'active') });
const environmentBusy = ref(false), environmentInvitation = ref('');
const busy = computed(() => groupBusy.value || environmentBusy.value);
watch(() => props.gameId, () => { environmentInvitation.value = ''; environmentBusy.value = false; });
watch(busy, value => emit('busy', value), { flush: 'sync' });
defineExpose({ canLeave: () => !busy.value, confirmLeave: async () => !busy.value });
</script>
<template>
  <section
    class="mesh-group-panel page-content"
    :aria-busy="busy || loading"
  >
    <header class="mesh-group-heading">
      <AppIcon name="network" /><p>{{ gameName }}</p>
    </header>
    <p
      v-if="error"
      class="group-error"
      role="alert"
    >
      {{ text(error) }}<button
        class="text-button"
        :disabled="busy || loading"
        @click="refresh"
      >
        {{ text('refresh') }}
      </button>
    </p>
    <LoadingIndicator
      v-if="loading && !group"
      class="group-loading"
      :label="text('loading')"
      compact
    />
    <MeshGroupCard
      v-if="group"
      :group="group"
      :names="names"
      :self-peer-id="selfPeerId"
      :invitation="invitation"
      :address="address"
      :disabled="busy || loading || !active"
      :copied="copied"
      @copy="copy"
      @invite="invite"
      @revoke="revoke"
      @stop="stop"
      @kick="kick"
    />
    <MeshGroupSetup
      v-if="!loading && !groupActive(group)"
      :running="running"
      :active="active"
      :disabled="busy || !active"
      :host-options="hostOptions"
      :initial-invitation="initialInvitation"
      @create="create"
      @join="join"
      @refresh="refresh"
      @launch="$emit('launch', gameId)"
      @prepare="environmentInvitation = $event"
    />
    <MeshEnvironmentPanel
      v-if="active && (group?.role === 'host' && groupActive(group) || !groupActive(group) && ((!running && !initialInvitation) || !!environmentInvitation))"
      :game-id="gameId"
      :hosting="group?.role === 'host' && groupActive(group)"
      :invitation="group?.role === 'host' && groupActive(group) ? undefined : environmentInvitation || undefined"
      :disabled="groupBusy || loading"
      @invitation="invitation = $event"
      @created="$emit('created', $event)"
      @preparing="(gameId, invitation) => $emit('prepared', { gameId, invitation })"
      @busy="environmentBusy = $event"
    />
    <LoadingIndicator
      v-if="busy"
      class="group-loading"
      :label="text('working')"
      compact
    />
  </section>
</template>
<style scoped>
.mesh-group-panel{width:100%;margin:0;color:var(--ui-text);min-width:0}.mesh-group-heading{display:grid;gap:8px;margin-bottom:24px}.mesh-group-heading h1{font-size:14px;margin:0;letter-spacing:-.4px}.mesh-group-heading p{font-size:14px;color:var(--ui-text-muted);margin:0;overflow-wrap:anywhere}.group-error{display:flex;align-items:center;gap:14px;flex-wrap:wrap;border:1px solid var(--ui-danger);border-radius:9px;padding:12px 16px;margin-bottom:16px;font-size:14px;color:var(--ui-danger)}.group-error button{margin-left:auto}.group-loading{display:flex;align-items:center;gap:8px;color:var(--ui-text-muted);font-size:14px;margin:16px 0}.mesh-group-panel :deep(button){font-size:14px}.mesh-group-panel :deep(.mesh-group-card + .group-setup){margin-top:18px}@media(max-width:500px){.mesh-group-heading h1{font-size:14px}}
</style>
<style scoped>
.mesh-group-heading{display:flex;align-items:center;gap:var(--ui-space-3);padding-bottom:var(--ui-space-4);border-bottom:1px solid var(--ui-border);margin-bottom:var(--ui-space-5)}.mesh-group-heading>svg{width:24px;height:24px;color:var(--ui-text-muted)}.mesh-group-heading p{font-size:var(--ui-text-label);color:var(--ui-text);font-weight:var(--ui-weight-medium)}
</style>
