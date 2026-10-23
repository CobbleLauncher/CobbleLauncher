<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { CurseForgePackPreview } from './curseforge';
import type { PackPreview } from './games';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import { isActiveTask, taskProgressUsesBytes, taskTitleLabel, taskHasKnownTotal, taskPhaseLabel, taskStateLabel, type TaskSnapshot } from './tasks';
const props = defineProps<{ tasks: TaskSnapshot[]; loading: boolean; error: string; continueDisabled?: boolean }>();
const emit = defineEmits<{ updated: [task: TaskSnapshot]; reload: []; locate: [gameId: string]; continuing: [value: boolean]; selectedPack: [preview: PackPreview]; selectedCfPack: [preview: CurseForgePackPreview] }>();
const pending = ref<Record<string, 'cancel' | 'retry' | 'continue'>>({});
const actionErrors = ref<Record<string, string>>({});
const sorted = computed(() => [...props.tasks].sort((left, right) => Number(isActiveTask(right)) - Number(isActiveTask(left)) || right.updated_at - left.updated_at));
const filter = ref<'all' | 'active' | 'failed'>('all');
const search = ref('');
const filterCounts = computed(() => ({ all: props.tasks.length, active: props.tasks.filter(isActiveTask).length, failed: props.tasks.filter(task => task.state === 'failed').length }));
const showFilters = computed(() => props.tasks.length >= 4);
const showSearch = showFilters;
const visibleTasks = computed(() => {
  const query = search.value.trim().toLocaleLowerCase(locale.value);
  return sorted.value.filter(task => {
    if (filter.value === 'active' && !isActiveTask(task)) return false;
    if (filter.value === 'failed' && task.state !== 'failed') return false;
    if (!query) return true;
    return [taskTitleLabel(task), task.game_name, task.error ?? '', taskStateLabel(task)]
      .some(value => value.toLocaleLowerCase(locale.value).includes(query));
  });
});
watch(filterCounts, counts => {
  if (props.tasks.length < 4 || (filter.value === 'active' && !counts.active) || (filter.value === 'failed' && !counts.failed)) filter.value = 'all';
  if (props.tasks.length < 4) search.value = '';
});
function count(value: number): string { return new Intl.NumberFormat(locale.value).format(value); }
function timestamp(value: number): string { return new Intl.DateTimeFormat(locale.value, { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(value); }
function percent(task: TaskSnapshot): number { return task.progress && task.progress.total > 0 ? Math.max(0, Math.min(100, task.progress.completed / task.progress.total * 100)) : 0; }
function bytes(value: number): string { const unit = value >= 1024 ** 3 ? 3 : value >= 1024 ** 2 ? 2 : value >= 1024 ? 1 : 0; return `${new Intl.NumberFormat(locale.value, { maximumFractionDigits: unit ? 1 : 0 }).format(value / 1024 ** unit)} ${['B', 'KB', 'MB', 'GB'][unit]}`; }
function source(value: string): string { return ({ official: t('tasks.sources.official'), bmclapi: 'BMCLAPI' } as Record<string, string>)[value] ?? (value.startsWith('custom-bmclapi-') ? t('tasks.sources.custom') : value); }
async function continueInstall(task: TaskSnapshot) {
  const curseforge = task.kind === 'curseforge_pack_download';
  let preview: CurseForgePackPreview | PackPreview;
  emit('continuing', true);
  try { preview = await invoke<CurseForgePackPreview | PackPreview>(curseforge ? 'get_task_curseforge_pack_preview' : 'get_task_pack_preview', {taskId: task.id}); }
  finally { emit('continuing', false); }
  if (curseforge) emit('selectedCfPack', preview as CurseForgePackPreview);
  else emit('selectedPack', preview as PackPreview);
}
async function action(task: TaskSnapshot, operation: 'cancel' | 'retry' | 'continue') {
  if (pending.value[task.id] || (operation === 'continue' && props.continueDisabled)) return;
  pending.value[task.id] = operation; delete actionErrors.value[task.id];
  try {
    if (operation === 'continue') await continueInstall(task);
    else emit('updated', await invoke<TaskSnapshot>(operation === 'cancel' ? 'cancel_task' : 'retry_task', { taskId: task.id }));
  } catch (cause) { actionErrors.value[task.id] = t('tasks.actionError', { cause: String(cause) }); }
  finally { delete pending.value[task.id]; }
}
</script>
<template>
  <div class="tasks-page page-content">
    <div class="tasks-page-tools">
      <p class="setting-hint">
        {{ t('tasks.history') }}
      </p><button
        class="text-button"
        :disabled="loading"
        @click="emit('reload')"
      >
        {{ loading ? t('tasks.loading') : t('tasks.refresh') }}
      </button>
    </div>
    <div
      v-if="error"
      class="inline-error"
      role="alert"
    >
      <p>{{ error }}</p><button
        class="text-button"
        :disabled="loading"
        @click="emit('reload')"
      >
        {{ t('tasks.retryLoad') }}
      </button>
    </div>
    <p
      v-if="loading && !tasks.length"
      class="content-empty"
      role="status"
    >
      {{ t('tasks.loadingTasks') }}
    </p>
    <div
      v-if="showFilters"
      class="task-history-controls"
    >
      <div
        class="task-filter-group"
        role="group"
        :aria-label="t('tasks.filterHistory')"
      >
        <button
          v-for="option in ['all', 'active', 'failed'] as const"
          :key="option"
          type="button"
          class="task-filter"
          :aria-pressed="filter === option"
          :disabled="option !== 'all' && !filterCounts[option]"
          @click="filter = option"
        >
          {{ t(`tasks.filters.${option}`, { count: count(filterCounts[option]) }) }}
        </button>
      </div>
      <div
        v-if="showSearch"
        class="task-search"
      >
        <input
          v-model="search"
          type="text"
          :aria-label="t('tasks.search')"
          :placeholder="t('tasks.search')"
        >
        <button
          v-if="search"
          class="text-button"
          type="button"
          :aria-label="t('tasks.clearSearch')"
          @click="search = ''"
        >
          <AppIcon name="x" />
        </button>
      </div>
    </div>
    <ul
      v-if="visibleTasks.length"
      class="task-list"
      :aria-label="t('tasks.listLabel')"
    >
      <li
        v-for="task in visibleTasks"
        :key="task.id"
        class="task-row"
        :class="{ 'task-active': isActiveTask(task) }"
      >
        <div class="task-row-heading">
          <div class="task-info">
            <h2>{{ taskTitleLabel(task) }}</h2><p
              v-if="task.game_name"
              class="mod-meta"
            >
              {{ task.game_name }}
            </p>
          </div><span
            class="task-state"
            :class="`task-state-${task.state}`"
          ><AppIcon
            v-if="task.state === 'running'"
            name="loader-circle"
            class="ui-busy-icon"
          /><span>{{ taskStateLabel(task) }}</span></span>
        </div>
        <div
          v-if="task.progress && task.state !== 'succeeded'"
          class="task-progress"
        >
          <div class="task-progress-caption">
            <span>{{ taskPhaseLabel(task.progress) }}</span><span v-if="taskHasKnownTotal(task.progress)">{{ taskProgressUsesBytes(task.progress) ? bytes(task.progress.completed) : count(task.progress.completed) }} / {{ taskProgressUsesBytes(task.progress) ? bytes(task.progress.total) : count(task.progress.total) }}</span><span v-else-if="task.progress.total <= 0 && task.progress.completed > 0">{{ t('tasks.processed', { size: taskProgressUsesBytes(task.progress) ? bytes(task.progress.completed) : count(task.progress.completed) }) }}</span>
          </div>
          <div
            v-if="taskHasKnownTotal(task.progress)"
            class="install-track"
            role="progressbar"
            :aria-label="taskTitleLabel(task)"
            :aria-valuenow="Math.min(task.progress.completed, task.progress.total)"
            :aria-valuemin="0"
            :aria-valuemax="task.progress.total"
          >
            <div :style="{ width: `${percent(task)}%` }" />
          </div>
          <details
            v-if="task.progress.detail"
            class="task-transfer-details"
          >
            <summary>
              {{ bytes(task.progress.detail.bytesCompleted) }} / {{ bytes(task.progress.detail.bytesTotal) }}<template v-if="isActiveTask(task)">
                · {{ bytes(task.progress.detail.bytesPerSecond) }}/s · {{ t('tasks.activeDownloads', { count: task.progress.detail.activeDownloads }) }}
              </template>
            </summary>
            <ul
              v-if="task.progress.detail.items.length"
              :aria-label="t('tasks.downloadingFiles')"
            >
              <li
                v-for="(item, index) in task.progress.detail.items"
                :key="`${item.source}-${item.item}-${index}`"
              >
                <span class="task-transfer-file">{{ item.item }}</span><span class="task-transfer-meta"><span>{{ source(item.source) }}</span><span>{{ bytes(item.bytesReceived) }} / {{ bytes(item.bytesTotal) }}</span></span>
              </li>
            </ul>
            <p v-if="task.progress.reused">
              {{ t('tasks.reusedFiles', { count: count(task.progress.reused) }) }}
            </p>
          </details>
        </div>
        <p
          v-if="task.error"
          class="settings-error task-error"
        >
          {{ task.error }}
        </p>
        <p
          v-if="actionErrors[task.id]"
          class="settings-error task-error"
          role="alert"
        >
          {{ actionErrors[task.id] }}
        </p>
        <div class="task-row-footer">
          <time
            v-if="task.updated_at > 0"
            :datetime="new Date(task.updated_at).toISOString()"
          >{{ timestamp(task.updated_at) }}</time>
          <div class="task-actions">
            <button
              v-if="task.state === 'succeeded' && task.game_id"
              class="text-button"
              @click="emit('locate', task.game_id)"
            >
              <AppIcon name="folder" />{{ t('tasks.locateGame') }}
            </button>
            <button
              v-if="task.state === 'running'"
              class="text-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'cancel')"
            >
              {{ pending[task.id] === 'cancel' ? t('tasks.cancelling') : t('tasks.cancel') }}
            </button>
            <span
              v-else-if="task.state === 'cancelling'"
              class="task-action-hint"
            >{{ t('tasks.cancelling') }}</span>
            <button
              v-if="task.can_retry"
              class="secondary-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'retry')"
            >
              {{ pending[task.id] === 'retry' ? t('tasks.retrying') : t('tasks.retryTask') }}
            </button>
            <button
              v-if="task.state === 'succeeded' && ['modpack_download', 'curseforge_pack_download'].includes(task.kind) && task.has_result"
              class="secondary-button"
              :disabled="!!pending[task.id] || continueDisabled"
              @click="action(task, 'continue')"
            >
              {{ pending[task.id] === 'continue' ? t('tasks.loadingPreview') : t('tasks.continueInstall') }}
            </button>
          </div>
        </div>
      </li>
    </ul>
    <p
      v-else-if="tasks.length"
      class="content-empty"
    >
      {{ t('tasks.noMatches') }}
    </p>
    <p
      v-else-if="!error && !loading"
      class="content-empty"
    >
      {{ t('tasks.empty') }}
    </p>
  </div>
</template>
<style scoped>
.task-info h2 { font-size: 14px; }
.task-row { padding-block: 16px; }
.task-state, .task-progress-caption, .task-row-footer time, .task-action-hint, .task-actions .secondary-button { font-size: 12px; }
.task-state :deep(svg), .task-actions :deep(svg) { width: 14px; height: 14px; }
.task-actions .text-button { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; }
.task-transfer-details { margin-top: 8px; color: var(--muted); font-size: 12px; }
.task-state { display: inline-flex; align-items: center; gap: 5px; }
.task-history-controls { display: flex; align-items: center; gap: var(--ui-space-3); margin: 8px 0 16px; }
.task-filter-group { display: flex; flex: 0 0 auto; align-items: center; gap: 3px; padding: 3px; border: 1px solid var(--line); border-radius: var(--ui-radius-md); background: var(--hover); }
.task-filter { min-height: var(--ui-control-compact); padding: 5px 9px; border: 0; border-radius: var(--ui-radius-sm); background: transparent; color: var(--muted); font-size: var(--ui-text-caption); white-space: nowrap; }
.task-filter[aria-pressed="true"] { background: var(--panel); color: var(--ink); box-shadow: 0 1px 3px #00000008; font-weight: var(--ui-weight-medium); }
.task-filter:disabled { opacity: .5; }
.task-search { display: flex; flex: 1 1 180px; align-items: center; gap: var(--ui-space-2); min-width: 0; }
.task-search input { flex: 1; min-width: 0; min-height: var(--ui-control-compact); padding: 6px 10px; border: 1px solid var(--line); border-radius: var(--ui-radius-md); background: var(--panel); color: var(--ink); font: inherit; font-size: var(--ui-text-caption); }
.task-search input:focus-visible { outline: var(--ui-focus-ring); outline-offset: 2px; }
.task-search .text-button { flex: 0 0 auto; min-width: var(--ui-control-compact); padding: 4px; }
.task-search .text-button :deep(svg) { width: var(--ui-icon-sm); height: var(--ui-icon-sm); }
.task-transfer-details summary { cursor: pointer; }
.task-transfer-details ul { display: grid; gap: 6px; margin: 10px 0 0; padding: 0; list-style: none; }
.task-progress .install-track>div { transition: width 240ms ease-out; }
.task-progress-caption>span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-progress-caption>span:not(:first-child) { flex: 0 0 auto; white-space: nowrap; }
.task-transfer-details li { display: grid; grid-template-columns: minmax(0,1fr) minmax(0,auto); align-items: baseline; gap: 16px; min-width: 0; }
.task-transfer-file { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-transfer-meta { display: grid; grid-template-columns: minmax(0,1fr) auto; align-items: baseline; gap: 10px; min-width: 0; max-width: 55%; font-variant-numeric: tabular-nums; }
.task-transfer-meta>span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-transfer-meta>span:last-child { white-space: nowrap; text-align: right; }
@media(max-width:560px){.task-history-controls{align-items:stretch;flex-direction:column;gap:var(--ui-space-2)}.task-filter-group{width:fit-content;max-width:100%}.task-filter{padding-inline:8px}.task-search{flex:0 0 auto;width:100%}.task-transfer-details li{gap:8px}.task-transfer-meta{max-width:62%;gap:6px;font-size:10px}}
</style>
