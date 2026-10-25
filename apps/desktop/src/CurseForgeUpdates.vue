<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import type { ManagedGameMod, ModUpdateCandidate } from './games';
import CurseForgeUpdateReview from './CurseForgeUpdateReview.vue';
import ModVersionPolicy from './ModVersionPolicy.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
const props = defineProps<{gameId: string; locked: boolean; revision?: number}>();
const emit = defineEmits<{busy: [value: boolean]; updated: []; count: [value: number]; failed: [value: boolean]; policyChanged: []}>();
const updates = ref<ModUpdateCandidate[]>([]), managed = ref<ManagedGameMod[]>([]), selected = ref<string[]>([]);
const reviewing = ref<string[] | null>(null), policyProject = ref<string | null>(null), error = ref(''), installing = ref(false);
const allSelected = computed(() => updates.value.length > 0 && updates.value.every(item => selected.value.includes(item.project_id)));
const selectedPolicyVersion = computed(() => { const item = updates.value.find(value => value.project_id === policyProject.value); return item ? { version_id: item.version_id, version_number: item.version_number } : undefined; });
const versionsLabel = computed(() => locale.value === 'en-US' ? 'CurseForge versions' : 'CurseForge 版本');
const preferencesLabel = computed(() => locale.value === 'en-US' ? 'Preferences' : '偏好');
let requestId = 0;
function reset() { ++requestId; error.value = ''; reviewing.value = null; updates.value = []; selected.value = []; emit('count', 0); }
async function check() {
  reset(); const current = requestId; emit('failed', false);
  try {
    const value = await invoke<ManagedGameMod[]>('list_game_curseforge_mods', {id: props.gameId});
    if (current !== requestId) return; managed.value = value;
    if (!value.length) return;
    const result = await invoke<ModUpdateCandidate[]>('check_game_curseforge_updates', {id: props.gameId});
    if (current === requestId) { updates.value = result; emit('count', result.length); }
  } catch { if (current === requestId) { error.value = t('curseforge.updateFailed'); emit('failed', true); } }
}
function busy(value: boolean) { installing.value = value; emit('busy', value); }
function installed() { const projects = reviewing.value ?? []; reviewing.value = null; updates.value = updates.value.filter(item => !projects.includes(item.project_id)); selected.value = []; emit('count', updates.value.length); emit('updated'); busy(false); }
function toggle(project: string, enabled: boolean) { selected.value = enabled ? [...new Set([...selected.value, project])] : selected.value.filter(value => value !== project); }
async function pin(mod: ManagedGameMod) {
  if (props.locked || installing.value) return;
  busy(true); error.value = '';
  try { await invoke('set_game_mod_pinned', { id: props.gameId, projectId: mod.project_id, source: 'curseforge', pinned: !mod.pinned }); await check(); }
  catch { error.value = t('modUpdates.pinFailed'); }
  finally { busy(false); }
}
function policyChanged() { reset(); emit('policyChanged'); }
watch(() => [props.gameId, props.revision], () => { reset(); managed.value = []; policyProject.value = null; });
onUnmounted(() => { ++requestId; });
defineExpose({check, reset});
</script>
<template>
  <div class="cf-updates">
    <template v-if="reviewing">
      <div class="ui-inline">
        <button
          type="button"
          class="text-button"
          :disabled="installing"
          @click="reviewing = null"
        >
          {{ t('curseforge.backUpdates') }}
        </button>
        <span class="ui-caption">CurseForge</span>
      </div>
      <CurseForgeUpdateReview
        :game-id="gameId"
        :project-ids="reviewing"
        :locked="locked"
        @busy="busy"
        @installed="installed"
      />
    </template>
    <template v-else>
      <div
        v-if="updates.length"
        class="cf-update-selection"
      >
        <label><input
          type="checkbox"
          :checked="allSelected"
          :indeterminate="selected.length > 0 && !allSelected"
          :disabled="locked || installing"
          @change="selected = ($event.target as HTMLInputElement).checked ? updates.map(item => item.project_id) : []"
        >{{ t('modUpdates.selectAll') }} · CurseForge</label>
        <button
          type="button"
          class="secondary-button"
          :disabled="!selected.length || locked || installing"
          @click="reviewing = [...selected]"
        >
          {{ t('modUpdates.updateSelected', {count: selected.length}) }}
        </button>
      </div>
      <ul
        v-if="updates.length"
        class="mod-list update-list"
        :aria-label="t('curseforge.updates')"
      >
        <li
          v-for="update in updates"
          :key="update.project_id"
          class="mod-row"
        >
          <input
            type="checkbox"
            :aria-label="t('modUpdates.selectUpdate', {name: update.name})"
            :checked="selected.includes(update.project_id)"
            :disabled="locked || installing"
            @change="toggle(update.project_id, ($event.target as HTMLInputElement).checked)"
          >
          <div class="mod-info">
            <h3>{{ update.name }}</h3><p class="mod-meta">
              {{ update.current_version }} → {{ update.version_number }}
            </p>
          </div>
          <button
            type="button"
            class="text-button"
            :disabled="locked || installing"
            @click="policyProject = update.project_id"
          >
            {{ preferencesLabel }}
          </button>
          <button
            type="button"
            class="secondary-button"
            :disabled="locked || installing"
            @click="reviewing = [update.project_id]"
          >
            {{ t('curseforge.update') }}
          </button>
        </li>
      </ul>
      <SettingsDisclosure
        v-if="managed.length"
        :title="versionsLabel"
      >
        <ul class="cf-managed-mods">
          <li
            v-for="mod in managed"
            :key="mod.project_id"
          >
            <span>{{ mod.name ?? mod.filename ?? mod.project_id }}</span><button
              type="button"
              class="text-button"
              :disabled="locked || installing"
              @click="policyProject = mod.project_id"
            >
              {{ preferencesLabel }}
            </button><button
              type="button"
              class="text-button"
              :disabled="locked || installing"
              @click="pin(mod)"
            >
              {{ t(mod.pinned ? 'modUpdates.unpin' : 'modUpdates.pin') }}
            </button>
          </li>
        </ul>
      </SettingsDisclosure>
      <ModVersionPolicy
        v-if="policyProject"
        :key="policyProject"
        :game-id="gameId"
        source="curseforge"
        hide-game-channel
        :project-id="policyProject"
        :selected-version="selectedPolicyVersion"
        :disabled="locked || installing"
        @busy="busy"
        @changed="policyChanged"
      />
    </template>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ error }}
    </p>
  </div>
</template>
<style scoped>
.cf-update-selection{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);flex-wrap:wrap;margin-block:var(--ui-space-4)}
.cf-update-selection label{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-label)}
.cf-managed-mods{list-style:none;margin:0;padding:0}
.cf-managed-mods li{display:flex;align-items:center;gap:var(--ui-space-3);padding-block:var(--ui-space-2);border-top:1px solid var(--ui-border)}
.cf-managed-mods li>span{flex:1;min-width:0;overflow-wrap:anywhere}
.cf-updates .mod-row input{flex:none}
@media(max-width:600px){.cf-managed-mods li{flex-wrap:wrap}}
</style>
