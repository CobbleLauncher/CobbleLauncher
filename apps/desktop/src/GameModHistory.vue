<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import type { Game } from './games';
import RecentGameMaintenance from './RecentGameMaintenance.vue';
const props = defineProps<{ game: Game; locked: boolean; revision?: number; defaultOpen?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; updated: [] }>();
interface Operation { operation_id: string; created_at: number; status: string; files: { name: string; before: string | null; after: string | null; enabled: boolean }[] }
const records = ref<Operation[]>([]);
const busy = ref(false);
const error = ref('');
const ready = ref<string | null>(null);
const requestedOpen = ref(0);
const historyOpen = ref(Boolean(props.defaultOpen));
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
async function openOperation(operationId: string) {
  if (busy.value || props.locked) return false;
  const gameId = props.game.id;
  requestedOpen.value++; ready.value = null; error.value = '';
  historyOpen.value = true;
  await load();
  if (props.game.id !== gameId || props.locked) return false;
  const operation = records.value.find(record => record.operation_id === operationId && record.status === 'applied');
  if (!operation) { error.value = t('modUpdates.historyConflict'); return false; }
  busy.value = true; emit('busy', true);
  try {
    const conflicts = await invoke<string[]>('preview_game_mod_operation_undo', { id: gameId, operationId });
    if (props.game.id !== gameId) return false;
    if (conflicts.length) { error.value = `${t('modUpdates.historyConflict')}\n${conflicts.join('\n')}`; return false; }
    ready.value = operationId; await nextTick();
    document.querySelector(`[data-recovery-operation="${operationId.replace(/[^A-Za-z0-9_-]/g, '')}"]`)?.scrollIntoView?.({ block: 'nearest' });
    return true;
  } catch (cause) { if (props.game.id === gameId) error.value = String(cause); return false; }
  finally { busy.value = false; emit('busy', false); }
}
defineExpose({ openOperation });
</script>

<template>
  <SettingsDisclosure
    :key="requestedOpen"
    class="mod-history"
    icon="file-search"
    :title="t('modUpdates.history')"
    :default-open="defaultOpen || requestedOpen > 0"
    @update:open="historyOpen = $event"
  >
    <RecentGameMaintenance
      v-if="historyOpen"
      :game-id="game.id"
      :revision="revision"
    />
    <p
      v-if="error"
      role="alert"
    >
      {{ error }}
    </p>
    <ol v-if="records.length">
      <li
        v-for="operation in records"
        :key="operation.operation_id"
        :data-recovery-operation="operation.operation_id"
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
.mod-history{margin:var(--ui-space-3) 0;padding:var(--ui-space-3) var(--ui-space-4);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.mod-history :deep(.settings-disclosure-trigger){font-size:var(--ui-text-label)}ol{list-style:none;margin:0;padding:0}ol>li{margin:0;padding:var(--ui-space-3) 0;border-top:1px solid var(--ui-border)}time{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums}ul{padding-left:var(--ui-space-4);margin:var(--ui-space-2) 0;font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.mod-history button{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}.mod-history [role=alert]{white-space:pre-line;color:var(--ui-danger);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
</style>
