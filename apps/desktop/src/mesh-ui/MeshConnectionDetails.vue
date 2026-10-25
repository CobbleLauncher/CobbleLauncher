<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { MeshOverview, MeshSession, MeshRoutePolicy, MeshRelay } from '../mesh';
import { defaultMeshRoutePolicy } from '../mesh';
import { t } from '../i18n';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import SelectField from '../ui/SelectField.vue';
import AppIcon from '../ui/AppIcon.vue';
const props = defineProps<{ defaultOpen?: boolean; error?: string; overview: MeshOverview | null; session: MeshSession | null; relay: string; port: string; busy: boolean; discovering: boolean; routePolicy?: MeshRoutePolicy }>();
const emit = defineEmits<{ 'update:relay': [value: string]; 'update:port': [value: string]; 'update:routePolicy': [value: MeshRoutePolicy]; discover: []; refresh: []; inspector: [] }>();
const saving = ref(false), saved = ref(false), failure = ref(false);
const activeSession = computed(() => props.session?.state === 'listening' || props.session?.state === 'bridge_ready');
function change(value: Partial<MeshRoutePolicy>) { saved.value = false; emit('update:routePolicy', { ...(props.routePolicy ?? defaultMeshRoutePolicy()), ...value }); }
async function save() {
  if (saving.value || props.busy || !props.relay.trim()) return;
  saving.value = true; saved.value = false; failure.value = false;
  try { const result = await invoke<MeshRelay>('mesh_route_policy_save', { relay: props.relay.trim(), routePolicy: props.routePolicy ?? defaultMeshRoutePolicy() }); if (result.route_policy) emit('update:routePolicy', result.route_policy); saved.value = true; }
  catch { failure.value = true; }
  finally { saving.value = false; }
}
</script>
<template>
  <SettingsDisclosure
    class="mesh-details"
    :title="t('mesh.routePolicy.title')"
    icon="settings"
    :value="t(`mesh.routePolicy.${routePolicy?.authorization ?? 'coordination'}`)"
    :default-open="defaultOpen"
  >
    <label>{{ t('mesh.relay') }}<input
      :value="relay"
      :disabled="busy || activeSession"
      @input="$emit('update:relay', ($event.target as HTMLInputElement).value)"
    ></label>
    <label for="mesh-relay-permission">{{ t('mesh.routePolicy.authorization') }}</label>
    <SelectField
      id="mesh-relay-permission"
      :model-value="routePolicy?.authorization ?? 'coordination'"
      :label="t('mesh.routePolicy.authorization')"
      :options="(['coordination','community','private','legacy_test'] as const).map(value => ({ value, label: t(`mesh.routePolicy.${value}`) }))"
      :disabled="busy || activeSession"
      @update:model-value="change({ authorization: $event as MeshRoutePolicy['authorization'] })"
    />
    <p
      v-if="routePolicy?.authorization === 'legacy_test'"
      class="setting-hint"
    >
      {{ t('mesh.routePolicy.legacyWarning') }}
    </p>
    <label>{{ t('mesh.routePolicy.observer') }}<input
      :value="routePolicy?.observer ?? ''"
      :disabled="busy || activeSession"
      @input="change({ observer: ($event.target as HTMLInputElement).value.trim() || null })"
    ></label>
    <div class="mesh-setting-actions">
      <button
        class="text-button"
        :disabled="busy || saving || !relay.trim() || activeSession"
        @click="save"
      >
        <AppIcon name="check" />{{ t('mesh.routePolicy.save') }}
      </button>
      <button
        class="text-button"
        :disabled="busy || discovering"
        @click="$emit('discover')"
      >
        <AppIcon :name="discovering ? 'loader-circle' : 'search'" />{{ t(discovering ? 'mesh.findingNodes' : 'mesh.findNodes') }}
      </button>
      <button
        class="text-button"
        @click="$emit('inspector')"
      >
        <AppIcon name="info" />{{ t('meshInspector.open') }}
      </button>
    </div>
    <p
      v-if="saved"
      role="status"
    >
      {{ t('mesh.routePolicy.saved') }}
    </p>
    <p
      v-if="failure"
      role="alert"
      class="settings-error"
    >
      {{ t('mesh.routePolicy.saveFailed') }}
    </p>
  </SettingsDisclosure>
</template>
<style scoped>
.mesh-details{margin-top:var(--ui-space-4,16px)}.mesh-details label{display:grid;gap:8px;margin:12px 0;font-size:14px}.mesh-details input{min-width:0;width:100%;box-sizing:border-box;padding:10px 12px;border:1px solid var(--line);border-radius:var(--ui-radius,12px);background:var(--bg);color:var(--ink);font:inherit}.mesh-setting-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:12px}.setting-hint{font-size:12px;color:var(--muted);margin:8px 0}
</style>
<style scoped>
.mesh-setting-actions button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.mesh-details label{font-weight:var(--ui-weight-medium)}
</style>
