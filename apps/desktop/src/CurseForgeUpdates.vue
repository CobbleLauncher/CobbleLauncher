<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { ModUpdateCandidate } from './games';
import CurseForgeInstallPlan from './CurseForgeInstallPlan.vue';
const props = defineProps<{gameId: string; locked: boolean; revision?: number}>();
const emit = defineEmits<{busy: [value: boolean]; updated: []; count: [value: number]; failed: [value: boolean]}>();
const updates = ref<ModUpdateCandidate[]>([]);
const selected = ref<ModUpdateCandidate | null>(null);
const error = ref('');
const installing = ref(false);
let requestId = 0;
async function check() {
  const current = ++requestId; error.value = ''; emit('failed', false); selected.value = null; updates.value = []; emit('count', 0);
  try {
    const managed = await invoke<{project_id: string}[]>('list_game_curseforge_mods', {id: props.gameId});
    if (current !== requestId || !managed.length) return;
    const result = await invoke<ModUpdateCandidate[]>('check_game_curseforge_updates', {id: props.gameId});
    if (current === requestId) { updates.value = result; emit('count', result.length); }
  } catch { if (current === requestId) { error.value = t('curseforge.updateFailed'); emit('failed', true); } }
}
function busy(value: boolean) { installing.value = value; emit('busy', value); }
function installed() { busy(false); const id = selected.value?.project_id; updates.value = updates.value.filter(item => item.project_id !== id); selected.value = null; emit('count', updates.value.length); emit('updated'); }
watch(() => [props.gameId, props.revision], () => { ++requestId; updates.value = []; selected.value = null; error.value = ''; emit('count', 0); });
onUnmounted(() => { ++requestId; });
defineExpose({check});
</script>
<template>
  <div class="cf-updates">
    <template v-if="selected">
      <button
        class="text-button"
        :disabled="installing"
        @click="selected = null"
      >
        {{ t('curseforge.backUpdates') }}
      </button>
      <p class="mod-meta">
        CurseForge · {{ selected.current_version }} → {{ selected.version_number }}
      </p>
      <CurseForgeInstallPlan
        :game-id="gameId"
        :project-id="selected.project_id"
        :file-id="selected.version_id"
        :locked="locked"
        @busy="busy"
        @installed="installed"
      />
    </template>
    <ul
      v-else-if="updates.length"
      class="mod-list update-list"
      :aria-label="t('curseforge.updates')"
    >
      <li
        v-for="update in updates"
        :key="update.project_id"
        class="mod-row"
      >
        <div class="mod-info">
          <h3>{{ update.name }} <span class="mod-state">CurseForge</span></h3><p class="mod-meta">
            {{ update.current_version }} → {{ update.version_number }}
          </p>
        </div>
        <button
          class="secondary-button"
          :disabled="locked || installing"
          @click="selected = update"
        >
          {{ t('curseforge.update') }}
        </button>
      </li>
    </ul>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ error }}
    </p>
  </div>
</template>


