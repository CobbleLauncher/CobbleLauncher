<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { PackPreview } from './games';
import { isActiveTask, taskHasKnownTotal, taskPhaseLabel, taskStateLabel, type TaskSnapshot } from './tasks';
const props = defineProps<{ tasks: TaskSnapshot[]; loading: boolean; error: string }>();
const emit = defineEmits<{ updated: [task: TaskSnapshot]; reload: []; selectedPack: [preview: PackPreview] }>();
const pending = ref<Record<string, 'cancel' | 'retry' | 'continue'>>({});
const actionErrors = ref<Record<string, string>>({});
const sorted = computed(() => [...props.tasks].sort((left, right) => Number(isActiveTask(right)) - Number(isActiveTask(left)) || right.updated_at - left.updated_at));
function count(value: number): string { return new Intl.NumberFormat('zh-CN').format(value); }
function timestamp(value: number): string { return new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(value); }
function percent(task: TaskSnapshot): number { return task.progress && task.progress.total > 0 ? Math.max(0, Math.min(100, task.progress.completed / task.progress.total * 100)) : 0; }
async function action(task: TaskSnapshot, operation: 'cancel' | 'retry' | 'continue') {
  if (pending.value[task.id]) return;
  pending.value[task.id] = operation; delete actionErrors.value[task.id];
  try {
    if (operation === 'continue') emit('selectedPack', await invoke<PackPreview>('get_task_pack_preview', { taskId: task.id }));
    else emit('updated', await invoke<TaskSnapshot>(operation === 'cancel' ? 'cancel_task' : 'retry_task', { taskId: task.id }));
  } catch (cause) { actionErrors.value[task.id] = `操作未完成：${String(cause)}`; }
  finally { delete pending.value[task.id]; }
}
</script>
<template>
  <div class="tasks-page page-content">
    <div class="tasks-page-tools">
      <p class="setting-hint">
        下载与安装记录
      </p><button
        class="text-button"
        :disabled="loading"
        @click="emit('reload')"
      >
        {{ loading ? '读取中…' : '刷新' }}
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
        重试读取任务
      </button>
    </div>
    <p
      v-if="loading && !tasks.length"
      class="content-empty"
      role="status"
    >
      正在读取任务…
    </p>
    <ul
      v-else-if="sorted.length"
      class="task-list"
      aria-label="任务列表"
    >
      <li
        v-for="task in sorted"
        :key="task.id"
        class="task-row"
        :class="{ 'task-active': isActiveTask(task) }"
      >
        <div class="task-row-heading">
          <div class="task-info">
            <h2>{{ task.title }}</h2><p
              v-if="task.game_name"
              class="mod-meta"
            >
              {{ task.game_name }}
            </p>
          </div><span
            class="task-state"
            :class="`task-state-${task.state}`"
          >{{ taskStateLabel(task) }}</span>
        </div>
        <div
          v-if="task.progress && task.state !== 'succeeded'"
          class="task-progress"
        >
          <div class="task-progress-caption">
            <span>{{ taskPhaseLabel(task.progress) }}</span><span v-if="taskHasKnownTotal(task.progress)">{{ count(task.progress.completed) }} / {{ count(task.progress.total) }}</span><span v-else-if="task.progress.total <= 0 && task.progress.completed > 0">已处理 {{ count(task.progress.completed) }}</span>
          </div>
          <div
            v-if="taskHasKnownTotal(task.progress)"
            class="install-track"
            role="progressbar"
            :aria-label="task.title"
            :aria-valuenow="Math.min(task.progress.completed, task.progress.total)"
            :aria-valuemin="0"
            :aria-valuemax="task.progress.total"
          >
            <div :style="{ width: `${percent(task)}%` }" />
          </div>
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
              v-if="task.state === 'running'"
              class="text-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'cancel')"
            >
              {{ pending[task.id] === 'cancel' ? '取消中…' : '取消任务' }}
            </button>
            <span
              v-else-if="task.state === 'cancelling'"
              class="task-action-hint"
            >取消中…</span>
            <button
              v-if="task.can_retry"
              class="secondary-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'retry')"
            >
              {{ pending[task.id] === 'retry' ? '正在重试…' : '重试任务' }}
            </button>
            <button
              v-if="task.state === 'succeeded' && task.kind === 'modpack_download' && task.has_result"
              class="secondary-button"
              :disabled="!!pending[task.id]"
              @click="action(task, 'continue')"
            >
              {{ pending[task.id] === 'continue' ? '正在读取…' : '继续安装' }}
            </button>
          </div>
        </div>
      </li>
    </ul>
    <p
      v-else-if="!error"
      class="content-empty"
    >
      还没有下载或安装任务。
    </p>
  </div>
</template>
