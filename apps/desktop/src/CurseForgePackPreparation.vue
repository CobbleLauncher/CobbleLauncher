<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { curseForgeLoaderLabel, type CurseForgePackPreview } from './curseforge';
const props = defineProps<{preview: CurseForgePackPreview}>();
const emit = defineEmits<{busy: [value: boolean]; selected: [preview: CurseForgePackPreview]}>();
const current = ref(props.preview);
const optional = ref([...props.preview.selected_optional_files]);
const working = ref(false);
const error = ref('');
const prepared = computed(() => !!current.value.plan_id && optional.value.length === current.value.selected_optional_files.length && optional.value.every(id => current.value.selected_optional_files.includes(id)));
let requestId = 0;
function setBusy(value: boolean) { working.value = value; emit('busy', value); }
async function prepare() {
  if (working.value) return;
  const request = ++requestId; setBusy(true); error.value = '';
  try { const result = await invoke<CurseForgePackPreview>('prepare_curseforge_pack', {previewId: current.value.preview_id, optionalFiles: [...optional.value]}); if (request === requestId) current.value = result; }
  catch { if (request === requestId) error.value = t('curseforge.packPrepareFailed'); }
  finally { if (request === requestId) setBusy(false); }
}
async function manual(projectId: string, fileId: string) {
  if (working.value || !current.value.plan_id) return;
  const request = ++requestId; setBusy(true); error.value = '';
  try { const result = await invoke<CurseForgePackPreview | null>('select_curseforge_pack_manual_file', {previewId: current.value.preview_id, planId: current.value.plan_id, projectId, fileId}); if (request === requestId && result) current.value = result; }
  catch (cause) { if (request === requestId) error.value = t(String(cause) === 'manual_file_mismatch' ? 'curseforge.fileMismatch' : 'curseforge.packPrepareFailed'); }
  finally { if (request === requestId) setBusy(false); }
}
async function open(projectId: string) { try { await invoke('open_curseforge_project', {projectId}); } catch { error.value = t('curseforge.openFailed'); } }
function select() { if (!working.value && prepared.value && current.value.ready && current.value.plan_id) emit('selected', current.value); }
watch(() => props.preview, value => { ++requestId; current.value = value; optional.value = [...value.selected_optional_files]; error.value = ''; });
onUnmounted(() => { ++requestId; });
</script>
<template>
  <div class="cf-pack-preparation">
    <h2>{{ current.name }} <span class="count">{{ current.version_id }}</span></h2>
    <p class="mod-meta">
      Minecraft {{ current.minecraft_version }}<span v-if="current.loader_kind"> · {{ curseForgeLoaderLabel(current.loader_kind) }} {{ current.loader_version }}</span>
    </p>
    <fieldset
      v-if="current.optional_files.length"
      :disabled="working"
      class="cf-optionals"
    >
      <legend>{{ t('curseforge.optionalFiles') }}</legend>
      <label
        v-for="file in current.optional_files"
        :key="file.id"
        class="pack-optional"
      ><input
        v-model="optional"
        type="checkbox"
        :value="file.id"
      ><span>{{ t('curseforge.projectFile', {project: file.project_id, file: file.file_id}) }}</span></label>
    </fieldset>
    <button
      v-if="!prepared"
      class="primary-button"
      :disabled="working"
      @click="prepare"
    >
      {{ t(working ? 'curseforge.preparing' : 'curseforge.resolvePack') }}
    </button>
    <template v-else>
      <p class="mod-meta">
        {{ t('curseforge.packFiles', {count: current.files.length}) }}
      </p>
      <div
        v-for="file in current.missing"
        :key="`${file.project_id}-${file.file_id}`"
        class="cf-missing"
      >
        <strong>{{ file.name }}</strong><p class="mod-meta">
          {{ file.file_name }}
        </p><p>{{ t(file.reason === 'manual' ? 'curseforge.manualHelp' : 'curseforge.archivedLocalHelp') }}</p><div class="cf-actions">
          <button
            v-if="file.website_url"
            class="text-button"
            :disabled="working"
            @click="open(file.project_id)"
          >
            {{ t('curseforge.openProject') }}
          </button><button
            class="secondary-button"
            :disabled="working"
            @click="manual(file.project_id, file.file_id)"
          >
            {{ t(file.reason === 'manual' ? 'curseforge.chooseFile' : 'curseforge.chooseLocalFile') }}
          </button>
        </div>
      </div>
      <button
        class="primary-button"
        :disabled="working || !current.ready"
        @click="select"
      >
        {{ t('curseforge.continue') }}
      </button>
    </template>
    <p
      v-if="working && prepared"
      role="status"
    >
      {{ t('curseforge.preparing') }}
    </p>
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
.cf-pack-preparation { margin-top: 20px; overflow-wrap: anywhere; }.cf-optionals { border: 0; padding: 0; margin: 20px 0; }.cf-missing { padding: 16px 0; border-bottom: 1px solid var(--line); margin-bottom: 16px; }.cf-actions { display: flex; flex-wrap: wrap; gap: 16px; align-items: center; }
</style>

