<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SelectField from './ui/SelectField.vue';
import { computed, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { Game } from './games';
import { canImportBlueprint, type BlueprintFormat, type BlueprintImportResult } from './blueprints';

const props = withDefaults(defineProps<{ blueprintId: string; format?: BlueprintFormat; initialGameId?: string }>(), { format: 'schem', initialGameId: '' });
const emit = defineEmits<{ market: [value: { gameId: string; query: string }] }>();
const games = ref<Game[]>([]);
const selected = ref('');
const busy = ref(false);
const error = ref('');
const destination = ref('');
const duplicatePolicy = ref<'error' | 'keep_both'>('error');
const modStatus = ref<'checking' | 'found' | 'missing' | 'unknown'>('unknown');
const targetMod = computed(() => props.format === 'litematic' ? 'Litematica' : 'WorldEdit');
let scan = 0;
watch(selected, async id => {
  const current = ++scan;
  destination.value = ''; error.value = ''; modStatus.value = id ? 'checking' : 'unknown';
  if (!id) return;
  try {
    const mods = await invoke<{ name: string; file_name: string; enabled: boolean }[]>('list_game_mods', { id });
    if (current === scan) modStatus.value = mods.some(mod => mod.enabled && [mod.name, mod.file_name].some(value => value?.toLowerCase().includes(targetMod.value.toLowerCase()))) ? 'found' : 'missing';
  } catch { if (current === scan) modStatus.value = 'unknown'; }
});
async function openDestination() {
  try { await invoke('open_blueprint_destination', { id: selected.value }); }
  catch (cause) { error.value = String(cause); }
}
onMounted(async () => {
  try { games.value = await invoke<Game[]>('list_games'); if (games.value.some(game => game.id === props.initialGameId && canImportBlueprint(game))) selected.value = props.initialGameId ?? ''; }
  catch (cause) { error.value = String(cause); }
});
async function importFile() {
  if (!games.value.some(game => game.id === selected.value && canImportBlueprint(game))) return;
  busy.value = true; error.value = ''; destination.value = '';
  try {
    const result = await invoke<BlueprintImportResult>('import_blueprint', { id: selected.value, blueprintId: props.blueprintId, duplicatePolicy: duplicatePolicy.value });
    destination.value = result.destination;
  } catch (cause) { error.value = String(cause).startsWith('BLUEPRINT_ALREADY_EXISTS') ? t('blueprints.duplicateExists') : String(cause); }
  finally { busy.value = false; }
}
</script>
<template>
  <div class="blueprint-import">
    <SelectField
      :id="`blueprint-target-${blueprintId}`"
      v-model="selected"
      :label="t('blueprints.targetGame')"
      :placeholder="t('blueprints.targetGame')"
      :options="games.map(game => ({ value: game.id, label: game.name + (canImportBlueprint(game) ? '' : ` · ${t('blueprints.unavailable')}`), disabled: !canImportBlueprint(game) }))"
      :disabled="busy"
    />
    <SelectField
      :id="`blueprint-duplicates-${blueprintId}`"
      v-model="duplicatePolicy"
      :label="t('blueprints.duplicateFiles')"
      :options="[{ value: 'error', label: t('blueprints.stopOnDuplicate') }, { value: 'keep_both', label: t('blueprints.keepBoth') }]"
      :disabled="busy"
    />
    <button
      class="secondary-button"
      :disabled="busy || !selected"
      @click="importFile"
    >
      {{ busy ? t('blueprints.importing') : t('blueprints.importFile', { extension: props.format }) }}
    </button>
    <p class="setting-hint">
      {{ t('blueprints.destinationFolder', { extension: props.format }) }}
    </p>
    <p
      v-if="selected"
      class="setting-hint"
      aria-live="polite"
    >
      {{ t(`blueprints.mod${modStatus}`, { name: targetMod }) }}
    </p>
    <button
      v-if="selected && modStatus === 'missing'"
      class="text-button"
      @click="emit('market', { gameId: selected, query: targetMod })"
    >
      {{ t('blueprints.searchMod', { name: targetMod }) }}
    </button>
    <p class="setting-hint">
      {{ t(props.format === 'litematic' ? 'blueprints.litematicCompatibility' : 'blueprints.compatibility') }}
    </p>
    <p
      v-if="!games.length && !error"
      class="setting-hint"
    >
      {{ t('blueprints.noGames') }}
    </p>
    <p
      v-if="destination"
      class="setting-hint"
      role="status"
    >
      {{ t('blueprints.imported', { destination }) }}
    </p>
    <button
      v-if="destination"
      class="text-button"
      @click="openDestination"
    >
      {{ t('blueprints.openDestination') }}
    </button>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
<style scoped>
.blueprint-import{display:flex;gap:10px;align-items:center;flex-wrap:wrap;padding:8px 0 16px}.blueprint-import p{width:100%;margin:0;overflow-wrap:anywhere}.blueprint-import :deep(.select-field-trigger){width:auto;max-width:100%;flex:1 1 180px}.blueprint-import>button{min-height:40px}
</style>
