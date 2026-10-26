<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { CurseForgePackPreview } from './curseforge';
import type { PackPreview } from './games';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import type { AppIconName } from './ui/icons';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import EnvironmentTaskFiles from './readiness/EnvironmentTaskFiles.vue';
import { readinessTaskNeedsFiles } from './readiness';
import { isActiveTask, taskProgressUsesBytes, taskTitleLabel, taskFailureMessage, taskHasKnownTotal, taskPhaseLabel, taskStateLabel, type TaskSnapshot } from './tasks';
const props = defineProps<{ tasks: TaskSnapshot[]; loading: boolean; error: string; continueDisabled?: boolean }>();
const emit = defineEmits<{ updated: [task: TaskSnapshot]; reload: []; locate: [gameId: string]; continuing: [value: boolean]; selectedPack: [preview: PackPreview]; selectedCfPack: [preview: CurseForgePackPreview]; 'open-migration': [taskId: string]; 'open-transfer': [taskId: string] }>();
const pending = ref<Record<string, 'cancel' | 'retry' | 'continue'>>({});
const actionErrors = ref<Record<string, string>>({});
function taskIcon(task: TaskSnapshot): AppIconName {
  if (task.kind === 'game_transfer') return 'monitor';
  if (task.kind === 'directory_migration') return 'folder';
  if (task.kind === 'game_java') return 'cpu';
  if (task.kind.startsWith('world_') || task.kind === 'storage_move') return 'hard-drive';
  if (task.kind.startsWith('mod_') || task.kind === 'resource_pack' || task.kind === 'shader_pack') return 'puzzle';
  return ['game_copy', 'trial_copy', 'pack_update_copy'].includes(task.kind) ? 'folder' : 'download';
}
function transferValue(task: TaskSnapshot): string {
  const detail = task.progress?.detail;
  if (!detail) return '';
  return isActiveTask(task) ? `${bytes(detail.bytesPerSecond)}/s · ${t('tasks.activeDownloads', { count: detail.activeDownloads })}` : `${bytes(detail.bytesCompleted)} / ${bytes(detail.bytesTotal)}`;
}
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
        class="secondary-button"
        :disabled="loading"
        @click="emit('reload')"
      >
        <AppIcon :name="loading ? 'loader-circle' : 'refresh-cw'" />
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
          <span class="task-kind-icon"><AppIcon
            :name="taskIcon(task)"
            :size="20"
          /></span>
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
            :name="task.state === 'running' || task.state === 'cancelling' ? 'loader-circle' : task.state === 'succeeded' ? 'check' : task.state === 'failed' ? 'alert-circle' : 'info'"
            :class="{ 'ui-busy-icon': task.state === 'running' || task.state === 'cancelling' }"
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
          <SettingsDisclosure
            v-if="task.progress.detail"
            class="task-transfer-details"
            icon="network"
            :title="t('tasks.downloadingFiles')"
            :value="transferValue(task)"
          >
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
          </SettingsDisclosure>
        </div>
        <div
          v-if="task.error && !readinessTaskNeedsFiles(task.error)"
          class="settings-error task-error"
        >
          <p>{{ taskFailureMessage(task) }}</p>
          <SettingsDisclosure
            class="task-error-details"
            icon="info"
            :title="t('tasks.errorDetails')"
          >
            <p>{{ task.error }}</p>
          </SettingsDisclosure>
        </div>
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
              v-if="task.kind === 'game_transfer'"
              type="button"
              class="secondary-button"
              @click="emit('open-transfer', task.id)"
            >
              <AppIcon name="monitor" />{{ t('tasks.openTransfer') }}
            </button>
            <button
              v-if="task.kind === 'directory_migration'"
              type="button"
              class="secondary-button"
              @click="emit('open-migration', task.id)"
            >
              <AppIcon name="folder" />{{ locale === 'zh-CN' ? '查看搬迁' : 'View move' }}
            </button>
            <button
              v-if="task.game_id && (task.state === 'succeeded' || (task.state === 'failed' && ['mod_update', 'game_install', 'change_share', 'game_transfer'].includes(task.kind)))"
              class="secondary-button"
              @click="emit('locate', task.game_id)"
            >
              <AppIcon name="folder" />{{ t('tasks.locateGame') }}
            </button>
            <button
              v-if="task.state === 'running' || task.state === 'queued'"
              class="secondary-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'cancel')"
            >
              <AppIcon :name="pending[task.id] === 'cancel' ? 'loader-circle' : 'x'" />{{ pending[task.id] === 'cancel' ? t('tasks.cancelling') : t('tasks.cancel') }}
            </button>
            <span
              v-else-if="task.state === 'cancelling'"
              class="task-action-hint"
            >{{ t('tasks.cancelling') }}</span>
            <button
              v-if="task.can_retry && !readinessTaskNeedsFiles(task.error)"
              class="secondary-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'retry')"
            >
              <AppIcon :name="pending[task.id] === 'retry' ? 'loader-circle' : 'refresh-cw'" />{{ pending[task.id] === 'retry' ? t('tasks.retrying') : t('tasks.retryTask') }}
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
        <EnvironmentTaskFiles
          v-if="readinessTaskNeedsFiles(task.error)"
          :task-id="task.id"
          @retry="action(task, 'retry')"
        />
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
.task-info h2 { font-size: var(--ui-text-section); line-height: var(--ui-leading-section); font-weight: var(--ui-weight-semibold); }
.task-info .mod-meta { margin-top: 4px; font-size: var(--ui-text-body); }
.task-row { padding: var(--ui-space-5) var(--ui-space-4); border-bottom: 1px solid var(--ui-border); }
.task-row.task-active { background: color-mix(in srgb, var(--ui-accent) 8%, var(--ui-surface-1)); }
.task-kind-icon { display: grid; place-items: center; flex: 0 0 40px; height: 40px; background: var(--ui-surface-2); border-radius: var(--ui-radius-md); color: var(--ui-text-muted); }
.task-active .task-kind-icon { background: var(--ui-accent); color: var(--accent-ink); }
.task-progress, .task-error, .task-row-footer { margin-inline-start: 56px; }
.task-progress .install-track { height: 6px; }
.task-state, .task-progress-caption, .task-row-footer time, .task-action-hint { font-size: var(--ui-text-caption); line-height: var(--ui-leading-caption); font-variant-numeric: tabular-nums; }
.task-actions .secondary-button, .tasks-page-tools .secondary-button { display: inline-flex; align-items: center; gap: 8px; font-size: var(--ui-text-body); }
.task-state :deep(svg), .task-actions :deep(svg) { width: 14px; height: 14px; }
.task-error p { margin: 0; }
.task-error-details { margin-top: 8px; color: var(--ui-text-muted); font-size: var(--ui-text-caption); }
.task-error-details p { margin-top: 8px; overflow-wrap: anywhere; }
.task-transfer-details { margin-top: 8px; color: var(--muted); font-size: var(--ui-text-caption); }
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
.task-transfer-details ul { display: grid; gap: 6px; margin: 10px 0 0; padding: 0; list-style: none; }
.task-progress .install-track>div { transition: width 240ms ease-out; }
.task-progress-caption>span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-progress-caption>span:not(:first-child) { flex: 0 0 auto; white-space: nowrap; }
.task-transfer-details li { display: grid; grid-template-columns: minmax(0,1fr) minmax(0,auto); align-items: baseline; gap: 16px; min-width: 0; }
.task-transfer-file { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-transfer-meta { display: grid; grid-template-columns: minmax(0,1fr) auto; align-items: baseline; gap: 10px; min-width: 0; max-width: 55%; font-variant-numeric: tabular-nums; }
.task-transfer-meta>span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-transfer-meta>span:last-child { white-space: nowrap; text-align: right; }
@media(max-width:560px){.task-row{padding-inline:var(--ui-space-3)}.task-progress,.task-error,.task-row-footer{margin-inline-start:0}.task-info h2{font-size:var(--ui-text-body)}.task-row-heading{flex-wrap:wrap}.task-state{margin-left:auto}.task-history-controls{align-items:stretch;flex-direction:column;gap:var(--ui-space-2)}.task-filter-group{width:fit-content;max-width:100%}.task-filter{padding-inline:8px}.task-search{flex:0 0 auto;width:100%}.task-transfer-details li{gap:8px}.task-transfer-meta{max-width:62%;gap:6px;font-size:var(--ui-text-caption)}}
</style>
