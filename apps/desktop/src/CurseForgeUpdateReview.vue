<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { CurseForgeMissing, CurseForgePlan } from './curseforge';
import type { ModUpdatePreview } from './games';
type Preview = ModUpdatePreview & { missing: CurseForgeMissing[]; ready: boolean };
const props = defineProps<{ gameId: string; projectIds: string[]; locked: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; installed: [] }>();
const preview = ref<Preview | null>(null), busy = ref(false), error = ref('');
let generation = 0;
async function prepare() {
  const current = ++generation; busy.value = true; emit('busy', true); error.value = ''; preview.value = null;
  try { const result = await invoke<Preview>('preview_game_curseforge_updates', { id: props.gameId, projectIds: props.projectIds }); if (current === generation) preview.value = result; }
  catch { if (current === generation) error.value = t('modUpdates.prepareFailed'); }
  finally { if (current === generation) busy.value = false; emit('busy', false); }
}
async function manual(file: CurseForgeMissing) {
  if (!preview.value || busy.value || props.locked) return;
  const current = generation; busy.value = true; emit('busy', true); error.value = '';
  try { const result = await invoke<CurseForgePlan | null>('select_curseforge_manual_file', { id: props.gameId, planId: preview.value.plan_id, projectId: file.project_id, fileId: file.file_id }); if (result && current === generation) preview.value = { ...preview.value, missing: result.missing, ready: result.ready }; }
  catch { if (current === generation) error.value = t('curseforge.fileMismatch'); }
  finally { if (current === generation) busy.value = false; emit('busy', false); }
}
async function apply() {
  if (!preview.value?.ready || !preview.value.changes.length || busy.value || props.locked) return;
  const current = generation; busy.value = true; emit('busy', true); error.value = '';
  try { await invoke('apply_game_mod_updates', { id: props.gameId, planId: preview.value.plan_id }); if (current === generation) emit('installed'); }
  catch { if (current === generation) error.value = t('modUpdates.applyFailed'); }
  finally { if (current === generation) busy.value = false; emit('busy', false); }
}
async function openProject(projectId: string) { try { await invoke('open_curseforge_project', { projectId }); } catch { error.value = t('curseforge.openFailed'); } }
watch(() => [props.gameId, props.projectIds], () => void prepare(), { immediate: true });
onUnmounted(() => { ++generation; emit('busy', false); });
</script>
<template>
  <section
    class="cf-update-review"
    :aria-busy="busy"
  >
    <p
      v-if="busy && !preview"
      role="status"
    >
      {{ t('modUpdates.preparing') }}
    </p>
    <div
      v-if="error"
      role="alert"
      class="ui-danger"
    >
      {{ error }} <button
        v-if="!preview"
        type="button"
        class="text-button"
        :disabled="busy"
        @click="prepare"
      >
        {{ t('modUpdates.retryLoad') }}
      </button>
    </div>
    <template v-if="preview">
      <ul class="mod-list">
        <li
          v-for="change in preview.changes"
          :key="change.project_id"
          class="mod-row"
        >
          <div class="mod-info">
            <h3>{{ change.name }}</h3><p class="mod-meta">
              {{ change.from_version ?? t('modUpdates.addedDependency') }} → {{ change.to_version }}<span v-if="!change.enabled"> · {{ t('modUpdates.keepDisabled') }}</span>
            </p>
          </div>
        </li>
      </ul>
      <div
        v-for="file in preview.missing"
        :key="`${file.project_id}:${file.file_id}`"
        class="cf-missing"
      >
        <strong>{{ file.name }}</strong><p class="ui-caption">
          {{ file.file_name }}
        </p><p>{{ t(file.reason === 'manual' ? 'curseforge.manualHelp' : 'curseforge.archivedLocalHelp') }}</p>
        <button
          v-if="file.website_url"
          type="button"
          class="text-button"
          :disabled="busy"
          @click="openProject(file.project_id)"
        >
          {{ t('curseforge.openProject') }}
        </button>
        <button
          type="button"
          class="secondary-button"
          :disabled="busy || locked"
          @click="manual(file)"
        >
          {{ t('curseforge.chooseFile') }}
        </button>
      </div>
      <button
        type="button"
        class="primary-button"
        :disabled="!preview.ready || !preview.changes.length || busy || locked"
        @click="apply"
      >
        {{ t(busy ? 'modUpdates.applying' : 'modUpdates.confirm') }}
      </button>
    </template>
  </section>
</template>
<style scoped>
.cf-update-review{font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}
.cf-update-review h3{font-size:var(--ui-text-label);font-weight:var(--ui-weight-medium)}
.cf-missing{margin-block:var(--ui-space-4);padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md)}
.cf-missing p{margin-block:var(--ui-space-2)}
</style>
