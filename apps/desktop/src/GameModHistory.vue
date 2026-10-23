<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import type { Game } from './games';
const props = defineProps<{ game: Game; locked: boolean; revision?: number }>();
const emit = defineEmits<{ busy: [value: boolean]; updated: [] }>();
interface Operation { operation_id: string; created_at: number; status: string; files: { name: string; before: string | null; after: string | null; enabled: boolean }[] }
const records = ref<Operation[]>([]);
const busy = ref(false);
const error = ref('');
const ready = ref<string | null>(null);
const en = computed(() => locale.value === 'en-US');
let request = 0;
async function load() {
  const current = ++request;
  try {
    const result = await invoke<Operation[]>('list_game_mod_history', { id: props.game.id });
    if (current === request) records.value = result;
  } catch (cause) { if (current === request) error.value = String(cause); }
}
async function undo(operation: Operation) {
  if (busy.value || props.locked) return;
  busy.value = true; emit('busy', true); error.value = '';
  try {
    if (ready.value !== operation.operation_id) {
      const conflicts = await invoke<string[]>('preview_game_mod_operation_undo', { id: props.game.id, operationId: operation.operation_id });
      if (conflicts.length) { error.value = `${t('modUpdates.historyConflict')}\n${conflicts.join('\n')}`; return; }
      ready.value = operation.operation_id;
    } else {
      await invoke('undo_game_mod_operation', { id: props.game.id, operationId: operation.operation_id });
      ready.value = null; emit('updated');
    }
  } catch (cause) { error.value = String(cause); }
  finally { busy.value = false; emit('busy', false); await load(); }
}
watch(() => [props.game.id, props.revision], () => { records.value = []; ready.value = null; error.value = ''; void load(); }, { immediate: true });
</script>

<template>
  <SettingsDisclosure
    class="mod-history"
    :title="t('modUpdates.history')"
  >
    <p
      v-if="error"
      role="alert"
    >
      {{ error }}
    </p>
    <p v-if="!records.length && !error">
      {{ t('modUpdates.historyEmpty') }}
    </p>
    <ol v-else>
      <li
        v-for="operation in records"
        :key="operation.operation_id"
      >
        <time>{{ new Date(operation.created_at).toLocaleString(en ? 'en-US' : 'zh-CN') }}</time>
        <ul>
          <li
            v-for="(file, index) in operation.files"
            :key="index"
          >
            {{ file.name }} · {{ !file.before ? t('modUpdates.historyAdded') : !file.after ? t('modUpdates.historyRemoved') : t('modUpdates.historyReplaced') }}<span v-if="!file.enabled"> · {{ t('modUpdates.historyDisabled') }}</span>
          </li>
        </ul>
        <button
          v-if="operation.status === 'applied' || operation.status === 'undoing'"
          type="button"
          class="secondary-button"
          :disabled="locked || busy"
          @click="undo(operation)"
        >
          {{ ready === operation.operation_id ? t('modUpdates.historyConfirm') : operation.status === 'undoing' ? t('modUpdates.historyRetry') : t('modUpdates.historyUndo') }}
        </button>
        <span v-else>{{ operation.status === 'undone' ? t('modUpdates.historyUndone') : t('modUpdates.historyRecovery') }}</span>
      </li>
    </ol>
  </SettingsDisclosure>
</template>

<style scoped>
.mod-history { margin-block: 1rem; }
ol { padding-left: 1.4rem; }
ol > li { margin-block: .8rem; }
time { color: var(--muted); font-size: 12px; }
ul { padding-left: 1.2rem; margin-block: .4rem; }
</style>
