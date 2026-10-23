<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { LocalWorld } from './games';
import type { WorldRestoreResult } from './world-backups';
import type { WorldSnapshot, SnapshotCleanupPreview, SnapshotCleanupResult } from './world-snapshots';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { isActiveTask, isWorldSnapshotTask, taskHasKnownTotal, taskPhaseLabel, taskProgressUsesBytes, type TaskSnapshot } from './tasks';
type SnapshotStorage = { repository_stored_bytes: number; total_snapshot_logical_bytes: number; snapshot_count: number };
type SnapshotChange = { path: string; kind: 'added' | 'modified' | 'deleted'; before_bytes: number | null; after_bytes: number | null };
type SnapshotComparePage = { added: number; modified: number; deleted: number; unchanged: number; total: number; offset: number; rows: SnapshotChange[] };
const props = defineProps<{ tasks?: TaskSnapshot[]; gameId: string; world: LocalWorld; locked: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; close: []; restored: [value: WorldRestoreResult, folder: string] }>();
const snapshots = ref<WorldSnapshot[]>([]);
const orderedSnapshots = computed(() => [...snapshots.value].sort((a, b) => Date.parse(b.created_at) - Date.parse(a.created_at)));
const storage = ref<SnapshotStorage | null>(null);
const compareSelection = ref<string[]>([]);
const compareResult = ref<SnapshotComparePage | null>(null);
const compareBefore = ref<WorldSnapshot | null>(null);
const compareAfter = ref<WorldSnapshot | null>(null);
const comparing = ref(false);
const compareError = ref('');
const compareLimit = 50;
const reading = ref(false);
const saving = ref(false);
const restoring = ref(false);
const recovering = ref(false);
const previewingCleanup = ref(false);
const cleaning = ref(false);
const keepLast = ref(10);
const keepNamed = ref(true);
const cleanupPreview = ref<SnapshotCleanupPreview | null>(null);
const cleanupResult = ref<SnapshotCleanupResult | null>(null);
const validRetention = computed(() => Number.isInteger(keepLast.value) && keepLast.value >= 1 && keepLast.value <= 1000);
const activeTask = computed(() => props.tasks?.filter(task => task.game_id === props.gameId && task.world_folder === props.world.folder_name && isWorldSnapshotTask(task) && isActiveTask(task)).sort((a, b) => b.created_at - a.created_at)[0]);
const operationTaskId = ref<string | null>(null);
watch(activeTask, task => { if (task && (saving.value || restoring.value)) operationTaskId.value = task.id; }, { flush: 'sync' });
const cancelPending = ref(false);
const busy = computed(() => saving.value || restoring.value || recovering.value || previewingCleanup.value || cleaning.value || comparing.value || !!activeTask.value);
watch(busy, value => emit('busy', value), { immediate: !!activeTask.value });
watch(() => activeTask.value?.id, () => { cancelPending.value = false; });
const name = ref('');
const error = ref('');
const status = ref<'created' | 'restored' | 'keyExported' | 'keyImported' | 'cancelled' | null>(null);
const selected = ref<WorldSnapshot | null>(null);
let request = 0;
let storageRequest = 0;
let compareRequest = 0;
let disposed = false;
function dateLabel(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'short' }).format(date);
}
function sizeLabel(bytes: number) {
  const unit = bytes < 1024 ? 'B' : bytes < 1024 ** 2 ? 'KiB' : bytes < 1024 ** 3 ? 'MiB' : 'GiB';
  const divisor = { B: 1, KiB: 1024, MiB: 1024 ** 2, GiB: 1024 ** 3 }[unit];
  return `${new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 }).format(bytes / divisor)} ${unit}`;
}
async function load() {
  const current = ++request;
  ++compareRequest;
  comparing.value = false;
  compareResult.value = null;
  compareBefore.value = null;
  compareAfter.value = null;
  compareError.value = '';
  reading.value = true; error.value = '';
  try {
    const value = await invoke<WorldSnapshot[]>('list_world_snapshots', { gameId: props.gameId, folderName: props.world.folder_name });
    if (current === request && !disposed) snapshots.value = value;
  } catch (cause) { if (current === request && !disposed) error.value = String(cause); }
  finally {
    if (current === request) {
      await loadStorage();
      if (current === request) reading.value = false;
    }
  }
}
async function loadStorage() {
  const current = ++storageRequest;
  try {
    const value = await invoke<SnapshotStorage | null>('get_world_snapshot_storage', { gameId: props.gameId });
    if (current === storageRequest && !disposed) storage.value = value;
  } catch { if (current === storageRequest && !disposed) storage.value = null; }
}
function selectForCompare(snapshot: WorldSnapshot, checked: boolean) {
  compareResult.value = null;
  compareError.value = '';
  compareBefore.value = null;
  compareAfter.value = null;
  if (checked) {
    if (!compareSelection.value.includes(snapshot.id) && compareSelection.value.length < 2) compareSelection.value = [...compareSelection.value, snapshot.id];
  } else compareSelection.value = compareSelection.value.filter(id => id !== snapshot.id);
}
async function loadComparePage(offset: number) {
  if (!compareBefore.value || !compareAfter.value || comparing.value) return;
  const current = ++compareRequest;
  comparing.value = true;
  compareError.value = '';
  try {
    const page = await invoke<SnapshotComparePage>('compare_world_snapshots', {
      gameId: props.gameId,
      folderName: props.world.folder_name,
      beforeId: compareBefore.value.id,
      afterId: compareAfter.value.id,
      offset,
      limit: compareLimit,
    });
    if (current !== compareRequest || disposed) return;
    compareResult.value = offset && compareResult.value ? { ...page, rows: [...compareResult.value.rows, ...page.rows] } : page;
  } catch { if (current === compareRequest && !disposed) compareError.value = t('worldSnapshots.compareFailed'); }
  finally { if (current === compareRequest) comparing.value = false; }
}
async function compareSelected() {
  if (compareSelection.value.length !== 2 || busy.value || reading.value || comparing.value) return;
  const pair = compareSelection.value.map(id => snapshots.value.find(snapshot => snapshot.id === id)).filter((snapshot): snapshot is WorldSnapshot => !!snapshot).sort((a, b) => Date.parse(a.created_at) - Date.parse(b.created_at));
  if (pair.length !== 2) return;
  compareBefore.value = pair[0]!;
  compareAfter.value = pair[1]!;
  compareResult.value = null;
  selected.value = null;
  cleanupPreview.value = null;
  cleanupResult.value = null;
  await loadComparePage(0);
}
function progressLabel(value: number, phase: string): string {
  return taskProgressUsesBytes({ phase, completed: 0, total: 0, reused: 0 }) ? sizeLabel(value) : new Intl.NumberFormat(locale.value).format(value);
}
async function operationFailed(cause: unknown) {
  await nextTick();
  if (disposed) return;
  const id = operationTaskId.value;
  let task = props.tasks?.find(value => value.id === id);
  if (id && task?.state !== 'cancelled' && task?.state !== 'failed') {
    try { task = (await invoke<TaskSnapshot[]>('list_tasks')).find(value => value.id === id); }
    catch { /* Preserve the original operation error if task state cannot be confirmed. */ }
  }
  if (disposed) return;
  if (task?.state === 'cancelled') status.value = 'cancelled';
  else error.value = String(cause);
}
async function create() {
  if (busy.value || reading.value || props.locked) return;
  operationTaskId.value = null; saving.value = true; error.value = ''; status.value = null;
  cleanupPreview.value = null; cleanupResult.value = null;
  try {
    await invoke<WorldSnapshot>('create_world_snapshot', { gameId: props.gameId, folderName: props.world.folder_name, name: name.value.trim() || null });
    if (!disposed) { name.value = ''; status.value = 'created'; await load(); }
  } catch (cause) { await operationFailed(cause); }
  finally { saving.value = false; }
}
async function restore() {
  if (!selected.value || busy.value || props.locked) return;
  const snapshot = selected.value;
  operationTaskId.value = null; restoring.value = true; error.value = ''; status.value = null;
  try {
    const result = await invoke<WorldRestoreResult>('restore_world_snapshot', { gameId: props.gameId, folderName: props.world.folder_name, snapshotId: snapshot.id, replaceExisting: true });
    if (!disposed) { selected.value = null; status.value = 'restored'; emit('restored', result, props.world.folder_name); }
  } catch (cause) { await operationFailed(cause); }
  finally { restoring.value = false; }
}
async function cancelTask() {
  if (!activeTask.value || activeTask.value.state !== 'running' || cancelPending.value) return;
  operationTaskId.value = activeTask.value.id;
  cancelPending.value = true;
  try { await invoke<TaskSnapshot>('cancel_task', { taskId: activeTask.value.id }); }
  catch (cause) { if (!disposed) { error.value = String(cause); cancelPending.value = false; } }
}
async function recovery(action: 'export' | 'import') {
  if (busy.value || reading.value || props.locked) return;
  recovering.value = true; error.value = ''; status.value = null;
  try {
    const completed = await invoke<boolean>(`${action}_world_snapshot_recovery`, { gameId: props.gameId });
    if (!disposed && completed) {
      status.value = action === 'export' ? 'keyExported' : 'keyImported';
      if (action === 'import') await load();
    }
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { recovering.value = false; }
}
async function previewCleanup() {
  if (busy.value || reading.value || props.locked || !validRetention.value) return;
  const current = request;
  previewingCleanup.value = true; error.value = ''; status.value = null; selected.value = null; cleanupResult.value = null;
  try {
    const value = await invoke<SnapshotCleanupPreview | null>('preview_world_snapshot_cleanup', { gameId: props.gameId, folderName: props.world.folder_name, policy: { keep_last: keepLast.value, keep_named: keepNamed.value } });
    if (!disposed && current === request) cleanupPreview.value = value;
    if (!disposed && current === request && !value) cleanupResult.value = { removed_count: 0, reclaimed_bytes: 0, cleanup_complete: true, remaining: snapshots.value };
  } catch (cause) { if (!disposed && current === request) error.value = String(cause); }
  finally { previewingCleanup.value = false; }
}
async function applyCleanup() {
  if (!cleanupPreview.value || busy.value || props.locked) return;
  const current = request;
  const gameId = props.gameId; const folderName = props.world.folder_name;
  const previewId = cleanupPreview.value.previewId;
  cleaning.value = true; error.value = ''; status.value = null;
  try {
    const value = await invoke<SnapshotCleanupResult>('apply_world_snapshot_cleanup', { previewId, confirmed: true });
    if (!disposed && current === request) { cleanupResult.value = value; snapshots.value = value.remaining; cleanupPreview.value = null; await loadStorage(); }
  } catch (cause) {
    if (!disposed && current === request) { cleanupPreview.value = null; await load(); if (!disposed && props.gameId === gameId && props.world.folder_name === folderName) error.value = String(cause); }
  } finally { cleaning.value = false; }
}
watch([keepLast, keepNamed], () => { cleanupPreview.value = null; cleanupResult.value = null; });
watch(() => [props.gameId, props.world.folder_name], () => { snapshots.value = []; selected.value = null; compareSelection.value = []; compareResult.value = null; compareBefore.value = null; compareAfter.value = null; status.value = null; name.value = ''; cleanupPreview.value = null; cleanupResult.value = null; void load(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++request; ++storageRequest; ++compareRequest; });
</script>
<template>
  <section
    class="world-snapshots"
    :aria-busy="busy || reading"
  >
    <button
      class="text-button"
      :disabled="busy"
      @click="emit('close')"
    >
      ← {{ t('worldSnapshots.back') }}
    </button>
    <div class="content-heading">
      <h2>{{ t('worldSnapshots.historyFor', { world: world.name }) }}</h2>
      <button
        class="text-button"
        :disabled="busy || reading"
        @click="load"
      >
        {{ t('worldSnapshots.refresh') }}
      </button>
    </div>
    <LoadingIndicator
      v-if="reading"
      :label="t('worldSnapshots.loading')"
      :skeleton="!snapshots.length"
      :compact="!!snapshots.length"
    />
    <p
      v-if="storage"
      class="snapshot-storage-summary"
    >
      <span>{{ t('worldSnapshots.repositoryStorage', { size: sizeLabel(storage.repository_stored_bytes) }) }}</span>
      <span>{{ t('worldSnapshots.logicalStorage', { size: sizeLabel(storage.total_snapshot_logical_bytes), count: storage.snapshot_count }) }}</span>
    </p>
    <p
      v-if="locked && !busy"
      class="setting-hint"
    >
      {{ t('worldSnapshots.locked') }}
    </p>
    <form
      class="snapshot-create"
      @submit.prevent="create"
    >
      <div class="form-field snapshot-name-field">
        <label for="snapshot-name">{{ t('worldSnapshots.name') }}</label>
        <input
          id="snapshot-name"
          v-model="name"
          maxlength="80"
          :placeholder="t('worldSnapshots.namePlaceholder')"
          :disabled="locked || busy"
        >
      </div>
      <button
        class="secondary-button"
        :disabled="locked || busy || reading"
      >
        {{ saving ? t('worldSnapshots.creating') : t('worldSnapshots.create') }}
      </button>
    </form>
    <p
      v-if="status"
      role="status"
    >
      {{ t(`worldSnapshots.${status}`) }}
    </p>
    <div
      v-if="activeTask"
      class="snapshot-task"
      role="status"
    >
      <div class="snapshot-actions">
        <AppIcon
          v-if="activeTask.state === 'running'"
          name="loader-circle"
          class="ui-busy-icon"
        />
        <span v-if="activeTask.progress">{{ taskPhaseLabel(activeTask.progress) }}</span>
        <span v-if="activeTask.progress && taskHasKnownTotal(activeTask.progress)">{{ progressLabel(activeTask.progress.completed, activeTask.progress.phase) }} / {{ progressLabel(activeTask.progress.total, activeTask.progress.phase) }}</span>
        <span v-else-if="activeTask.progress && activeTask.progress.completed > 0">{{ t('tasks.processed', { size: progressLabel(activeTask.progress.completed, activeTask.progress.phase) }) }}</span>
        <button
          class="text-button"
          :disabled="cancelPending || activeTask.state === 'cancelling'"
          @click="cancelTask"
        >
          {{ cancelPending || activeTask.state === 'cancelling' ? t('tasks.cancelling') : t('tasks.cancel') }}
        </button>
      </div>
      <progress
        v-if="activeTask.progress && taskHasKnownTotal(activeTask.progress)"
        :value="Math.min(activeTask.progress.completed, activeTask.progress.total)"
        :max="activeTask.progress.total"
        :aria-label="taskPhaseLabel(activeTask.progress)"
      />
    </div>
    <details class="snapshot-recovery">
      <summary>{{ t('worldSnapshots.recovery') }}</summary>
      <p class="setting-hint">
        {{ t('worldSnapshots.recoveryWarning') }}
      </p>
      <div class="snapshot-actions">
        <button
          class="text-button"
          :disabled="locked || busy || reading"
          @click="recovery('export')"
        >
          {{ t('worldSnapshots.exportKey') }}
        </button>
        <button
          class="text-button"
          :disabled="locked || busy || reading"
          @click="recovery('import')"
        >
          {{ t('worldSnapshots.importKey') }}
        </button>
      </div>
    </details>
    <details class="snapshot-cleanup">
      <summary>{{ t('worldSnapshots.cleanup') }}</summary>
      <form
        class="snapshot-cleanup-policy"
        @submit.prevent="previewCleanup"
      >
        <label>{{ t('worldSnapshots.keepLatest') }}
          <input
            v-model.number="keepLast"
            type="number"
            min="1"
            max="1000"
            step="1"
            :disabled="locked || busy"
            class="retention-count"
          >
        </label>
        <details>
          <summary>{{ t('worldSnapshots.advanced') }}</summary>
          <label class="retention-named"><input
            v-model="keepNamed"
            type="checkbox"
            :disabled="locked || busy"
          >{{ t('worldSnapshots.keepNamed') }}</label>
        </details>
        <button
          class="secondary-button"
          :disabled="locked || busy || reading || !validRetention"
        >
          {{ previewingCleanup ? t('worldSnapshots.previewingCleanup') : t('worldSnapshots.previewCleanup') }}
        </button>
      </form>
      <div
        v-if="cleanupPreview"
        class="snapshot-confirm cleanup-preview"
      >
        <p>{{ t('worldSnapshots.cleanupCounts', { remove: cleanupPreview.remove.length, keep: cleanupPreview.keep_count }) }}</p>
        <p>{{ t('worldSnapshots.cleanupEstimate', { size: sizeLabel(cleanupPreview.estimated_reclaim_bytes) }) }}</p>
        <p
          v-if="cleanupPreview.repack_bytes > 0"
          class="setting-hint"
        >
          {{ t('worldSnapshots.cleanupRepack') }}
        </p>
        <details v-if="cleanupPreview.remove.length">
          <summary>{{ t('worldSnapshots.versionsToRemove') }}</summary>
          <ul>
            <li
              v-for="version in cleanupPreview.remove"
              :key="version.id"
            >
              {{ version.name || dateLabel(version.created_at) }} · {{ dateLabel(version.created_at) }}
            </li>
          </ul>
        </details>
        <p v-if="cleanupPreview.remove.length">
          {{ t('worldSnapshots.cleanupWarning') }}
        </p>
        <p v-else-if="!cleanupPreview.estimated_reclaim_bytes">
          {{ t('worldSnapshots.nothingToClean') }}
        </p>
        <div class="snapshot-actions">
          <button
            v-if="cleanupPreview.remove.length || cleanupPreview.estimated_reclaim_bytes"
            class="secondary-button danger-text"
            :disabled="locked || busy"
            @click="applyCleanup"
          >
            {{ cleaning ? t('worldSnapshots.cleaning') : t('worldSnapshots.confirmCleanup') }}
          </button>
          <button
            class="text-button"
            :disabled="busy"
            @click="cleanupPreview = null"
          >
            {{ t('worldSnapshots.cancel') }}
          </button>
        </div>
      </div>
      <p
        v-if="cleanupResult"
        role="status"
      >
        {{ cleanupResult.cleanup_complete && !cleanupResult.removed_count && !cleanupResult.reclaimed_bytes ? t('worldSnapshots.nothingToClean') : t('worldSnapshots.cleanupDone', { count: cleanupResult.removed_count, size: sizeLabel(cleanupResult.reclaimed_bytes) }) }}
      </p>
      <p
        v-if="cleanupResult && !cleanupResult.cleanup_complete"
        role="alert"
      >
        {{ t('worldSnapshots.cleanupPartial') }}
      </p>
    </details>
    <div
      v-if="error"
      role="alert"
      class="settings-error"
    >
      <p>{{ error }}</p><button
        class="text-button"
        :disabled="busy || reading"
        @click="load"
      >
        {{ t('worldSnapshots.retry') }}
      </button>
    </div>
    <div
      v-if="selected"
      class="snapshot-confirm"
    >
      <h3>{{ t('worldSnapshots.confirm', { name: selected.name || dateLabel(selected.created_at) }) }}</h3>
      <p class="snapshot-restore-target">
        {{ t('worldSnapshots.restoreTarget', { world: world.name }) }}
      </p>
      <p class="snapshot-restore-stats">
        {{ t('worldSnapshots.logicalSize', { size: sizeLabel(selected.processed_bytes) }) }} · {{ t('worldSnapshots.storedSize', { size: sizeLabel(selected.added_bytes) }) }}
      </p>
      <p>{{ t('worldSnapshots.warning') }}</p>
      <div class="snapshot-actions">
        <button
          class="secondary-button danger-text"
          :disabled="locked || busy"
          @click="restore"
        >
          {{ restoring ? t('worldSnapshots.restoring') : t('worldSnapshots.confirmRestore') }}
        </button>
        <button
          class="text-button"
          :disabled="busy"
          @click="selected = null"
        >
          {{ t('worldSnapshots.cancel') }}
        </button>
      </div>
    </div>
    <div
      v-if="snapshots.length"
      class="snapshot-compare-toolbar"
    >
      <span>{{ t('worldSnapshots.compareSelection', { count: compareSelection.length }) }}</span>
      <button
        class="secondary-button"
        type="button"
        :disabled="locked || busy || reading || comparing || compareSelection.length !== 2"
        @click="compareSelected"
      >
        {{ comparing ? t('worldSnapshots.comparing') : t('worldSnapshots.compare') }}
      </button>
    </div>
    <section
      v-if="compareBefore && compareAfter"
      class="snapshot-compare"
      :aria-busy="comparing"
    >
      <h3>{{ t('worldSnapshots.compareTitle', { before: compareBefore.name || dateLabel(compareBefore.created_at), after: compareAfter.name || dateLabel(compareAfter.created_at) }) }}</h3>
      <LoadingIndicator
        v-if="comparing && !compareResult"
        :label="t('worldSnapshots.comparing')"
        compact
      />
      <p
        v-if="compareResult"
        class="mod-meta"
      >
        {{ t('worldSnapshots.compareCounts', { added: compareResult.added, modified: compareResult.modified, deleted: compareResult.deleted, unchanged: compareResult.unchanged }) }}
      </p>
      <ul
        v-if="compareResult && compareResult.rows.length"
        class="snapshot-change-list"
        :aria-label="t('worldSnapshots.changedFiles')"
      >
        <li
          v-for="(change, index) in compareResult.rows"
          :key="`${change.kind}:${change.path}:${index}`"
        >
          <span class="snapshot-change-kind">{{ t(`worldSnapshots.changeKinds.${change.kind}`) }}</span>
          <span
            class="snapshot-change-path"
            :title="change.path"
          >{{ change.path }}</span>
          <span class="snapshot-change-size">{{ change.before_bytes === null ? '—' : sizeLabel(change.before_bytes) }} → {{ change.after_bytes === null ? '—' : sizeLabel(change.after_bytes) }}</span>
        </li>
      </ul>
      <p
        v-else-if="compareResult && !comparing"
        class="setting-hint"
      >
        {{ t('worldSnapshots.noChanges') }}
      </p>
      <p
        v-if="compareError"
        role="alert"
        class="settings-error"
      >
        {{ compareError }}
      </p>
      <button
        v-if="compareError"
        class="text-button"
        type="button"
        :disabled="comparing"
        @click="loadComparePage(compareResult ? compareResult.rows.length : 0)"
      >
        {{ t('worldSnapshots.retry') }}
      </button>
      <button
        v-if="compareResult && compareResult.rows.length < compareResult.total"
        class="text-button"
        type="button"
        :disabled="comparing"
        @click="loadComparePage(compareResult.offset + compareResult.rows.length)"
      >
        {{ comparing ? t('worldSnapshots.comparing') : t('worldSnapshots.loadMoreChanges') }}
      </button>
    </section>
    <ul
      v-if="snapshots.length"
      class="snapshot-list"
    >
      <li
        v-for="snapshot in orderedSnapshots"
        :key="snapshot.id"
      >
        <div class="mod-info">
          <h3>{{ snapshot.name || dateLabel(snapshot.created_at) }}</h3>
          <p
            v-if="snapshot.name"
            class="mod-meta"
          >
            {{ dateLabel(snapshot.created_at) }}
          </p>
          <p class="mod-meta">
            {{ t('worldSnapshots.storedSize', { size: sizeLabel(snapshot.added_bytes) }) }} · {{ t('worldSnapshots.logicalSize', { size: sizeLabel(snapshot.processed_bytes) }) }}
            <template v-if="snapshot.protected">
              · {{ t('worldSnapshots.protectedVersion') }}
            </template>
          </p>
        </div>
        <div class="snapshot-row-actions">
          <label class="snapshot-compare-check">
            <input
              type="checkbox"
              :checked="compareSelection.includes(snapshot.id)"
              :disabled="locked || busy || reading || comparing || (compareSelection.length >= 2 && !compareSelection.includes(snapshot.id))"
              :aria-label="t('worldSnapshots.selectForCompare', { name: snapshot.name || dateLabel(snapshot.created_at) })"
              @change="selectForCompare(snapshot, ($event.target as HTMLInputElement).checked)"
            >
            <span>{{ t('worldSnapshots.compareShort') }}</span>
          </label>
          <button
            class="text-button"
            :disabled="locked || busy || reading"
            @click="selected = snapshot; cleanupPreview = null; cleanupResult = null; error = ''; status = null"
          >
            {{ t('worldSnapshots.restore') }}
          </button>
        </div>
      </li>
    </ul>
    <p
      v-else-if="!reading && !error"
      class="content-empty"
    >
      {{ t('worldSnapshots.empty') }}
    </p>
  </section>
</template>
<style scoped>
.snapshot-create .snapshot-name-field{flex:1;min-width:180px;margin:0}.snapshot-create>.secondary-button{align-self:flex-end}
.snapshot-cleanup{margin:18px 0;font-size:13px}.snapshot-cleanup summary{cursor:pointer}.snapshot-cleanup-policy{display:flex;align-items:center;gap:16px;flex-wrap:wrap;margin:14px 0}.snapshot-cleanup-policy>label{display:flex;align-items:center;gap:10px}.retention-count{width:84px;min-height:42px;border:1px solid var(--line);border-radius:9px;background:var(--bg);color:var(--ink);font:inherit;padding:10px 12px}.retention-count:focus-visible{outline:2px solid #779f42;outline-offset:2px}.retention-named{display:flex;gap:8px;align-items:center;margin-top:12px}.cleanup-preview ul{padding-left:20px;line-height:1.8;max-height:220px;overflow:auto}.cleanup-preview>p:first-child{margin-top:0}
.world-snapshots{min-width:0}.snapshot-task{margin:16px 0;font-size:13px}.snapshot-task progress{width:100%;margin-top:10px}.snapshot-recovery{margin:18px 0;font-size:13px}.snapshot-recovery summary{cursor:pointer}.snapshot-recovery p{line-height:1.6;margin:10px 0}.snapshot-create{display:flex;align-items:center;gap:12px;flex-wrap:wrap;margin:20px 0}.snapshot-create input{min-width:180px;flex:1}.snapshot-create label{font-size:13px}.snapshot-list{list-style:none;padding:0}.snapshot-list>li{display:flex;gap:16px;align-items:center;padding:16px 0;border-bottom:1px solid var(--line)}.snapshot-list h3,.snapshot-confirm h3{font-size:14px;margin:0;overflow-wrap:anywhere}.snapshot-list p{margin:6px 0 0}.snapshot-list button{flex-shrink:0}.snapshot-confirm{padding:18px;border:1px solid var(--line);border-radius:12px;margin:18px 0}.snapshot-confirm p{font-size:13px;line-height:1.6}.snapshot-actions{display:flex;gap:12px;flex-wrap:wrap}
.snapshot-restore-target{margin-top:10px}.snapshot-restore-stats{color:var(--muted);margin:7px 0 12px}.snapshot-actions{align-items:center}.snapshot-task progress{transition:width 240ms ease-out}
.snapshot-storage-summary{margin:12px 0;color:var(--muted);font-size:12px;line-height:1.6;font-variant-numeric:tabular-nums}.snapshot-storage-summary span{display:block}.snapshot-compare-toolbar{display:flex;align-items:center;justify-content:space-between;gap:12px;margin:12px 0;color:var(--muted);font-size:12px}.snapshot-row-actions{display:flex;align-items:center;gap:10px;flex-shrink:0}.snapshot-compare-check{display:flex;align-items:center;gap:5px;color:var(--muted);font-size:11px;white-space:nowrap}.snapshot-compare h3{margin:0 0 7px;font-size:14px}.snapshot-compare .mod-meta{margin-bottom:10px}.snapshot-change-list{list-style:none;margin:0 0 10px;padding:0;max-height:360px;overflow:auto}.snapshot-change-list li{display:grid;grid-template-columns:82px minmax(0,1fr) auto;align-items:center;gap:10px;padding:8px 0;border-bottom:1px solid var(--line);font-size:11px}.snapshot-change-kind{color:var(--muted)}.snapshot-change-path{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.snapshot-change-size{font-variant-numeric:tabular-nums;white-space:nowrap;color:var(--muted)}
@media(max-width:560px){.snapshot-list>li{align-items:flex-start;gap:10px}.snapshot-list>li .mod-info{min-width:0}.snapshot-row-actions{flex-direction:column;align-items:flex-end;gap:8px}.snapshot-row-actions .text-button{white-space:normal;max-width:126px}.snapshot-restore-stats{overflow-wrap:anywhere}.snapshot-change-list li{grid-template-columns:62px minmax(0,1fr);gap:6px}.snapshot-change-size{grid-column:2;justify-self:start}}
</style>
