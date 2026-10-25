<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentRequirements } from '../readiness';
import { t } from '../i18n';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
const props = defineProps<{ taskId: string }>();
defineEmits<{ retry: [] }>();
const files = ref<EnvironmentRequirements | null>(null);
const busy = ref(false);
const error = ref('');
let revision = 0;
async function load() {
  const current = ++revision; busy.value = true; error.value = '';
  try {
    const value = await invoke<EnvironmentRequirements>('get_task_environment_requirements', { taskId: props.taskId });
    if (current === revision) files.value = value;
  } catch { if (current === revision) error.value = t('readiness.manualFailed'); }
  finally { if (current === revision) busy.value = false; }
}
async function choose(projectId: string, fileId: string) {
  if (busy.value) return;
  const current = ++revision; busy.value = true; error.value = '';
  try {
    const value = await invoke<EnvironmentRequirements | null>('select_task_environment_manual_file', { taskId: props.taskId, projectId, fileId });
    if (current === revision && value) files.value = value;
  } catch { if (current === revision) error.value = t('readiness.manualFailed'); }
  finally { if (current === revision) busy.value = false; }
}
async function openProject(projectId: string) {
  try { await invoke('open_curseforge_project', { projectId }); }
  catch { error.value = t('readiness.manualFailed'); }
}
watch(() => props.taskId, () => { files.value = null; void load(); }, { immediate: true });
onUnmounted(() => { ++revision; });
</script>
<template>
  <section
    class="task-manual-files"
    :aria-label="t('readiness.manualTitle')"
  >
    <LoadingIndicator
      v-if="busy && !files"
      :label="t('readiness.loadRequirements')"
      compact
    />
    <div
      v-if="error"
      class="manual-error"
      role="alert"
    >
      {{ error }}<button
        class="text-button"
        :disabled="busy"
        @click="load"
      >
        {{ t('readiness.retry') }}
      </button>
    </div>
    <template v-if="files">
      <p>{{ t(files.ready ? 'readiness.manualReady' : 'readiness.manualHint') }}</p>
      <ul>
        <li
          v-for="file in files.missing"
          :key="`${file.project_id}:${file.file_id}`"
        >
          <div><strong>{{ file.name }}</strong><small :title="file.file_name">{{ file.file_name }}</small></div><button
            v-if="file.website_url"
            class="text-button"
            :disabled="busy"
            @click="openProject(file.project_id)"
          >
            {{ t('readiness.openProject') }}
          </button><button
            class="secondary-button"
            :disabled="busy"
            @click="choose(file.project_id, file.file_id)"
          >
            {{ t('readiness.chooseFile') }}
          </button>
        </li>
      </ul>
      <button
        v-if="files.ready"
        class="secondary-button"
        :disabled="busy"
        @click="$emit('retry')"
      >
        {{ t('readiness.retry') }}
      </button>
    </template>
  </section>
</template>
<style scoped>
.task-manual-files{margin-block:var(--ui-space-3);padding:var(--ui-space-4);background:var(--ui-surface-2);border-radius:var(--ui-radius-md)}
.task-manual-files p{margin:0 0 var(--ui-space-3);font-size:var(--ui-text-body)}.task-manual-files ul{list-style:none;margin:0;padding:0}
.task-manual-files li{display:flex;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap;padding-block:var(--ui-space-2)}.task-manual-files li>div{flex:1;min-width:180px}
.task-manual-files strong{font-size:var(--ui-text-body);font-weight:var(--ui-weight-medium)}.task-manual-files small{display:block;font-size:var(--ui-text-caption);color:var(--ui-text-muted);overflow-wrap:anywhere}
.manual-error{color:var(--ui-danger)}
</style>
