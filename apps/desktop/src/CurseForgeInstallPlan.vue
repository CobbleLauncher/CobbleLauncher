<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import type { CurseForgePlan } from './curseforge';
const props = defineProps<{gameId: string; projectId: string; fileId: string; locked: boolean}>();
const emit = defineEmits<{busy: [value: boolean]; installed: []}>();
const plan = ref<CurseForgePlan | null>(null);
const working = ref(false);
const error = ref('');
const complete = ref(false);
const applying = ref(false);
let revision = 0;
async function prepare() {
  const current = ++revision;
  working.value = true; error.value = ''; plan.value = null; complete.value = false;
  try { const value = await invoke<CurseForgePlan>('preview_curseforge_mod_install', {id: props.gameId, projectId: props.projectId, fileId: props.fileId}); if (current === revision) plan.value = value; }
  catch { if (current === revision) error.value = t('curseforge.installFailed'); }
  finally { if (current === revision) working.value = false; }
}
async function manual(projectId: string, fileId: string) {
  if (!plan.value || working.value) return;
  const current = ++revision; working.value = true; error.value = ''; emit('busy', true);
  try { const value = await invoke<CurseForgePlan | null>('select_curseforge_manual_file', {id: props.gameId, planId: plan.value.plan_id, projectId, fileId}); if (current === revision && value) plan.value = value; }
  catch (cause) { if (current === revision) error.value = t(String(cause) === 'manual_file_mismatch' ? 'curseforge.fileMismatch' : 'curseforge.installFailed'); }
  finally { if (current === revision) {working.value = false; applying.value = false;} emit('busy', false); }
}
async function openProject(projectId: string) {
  error.value = '';
  try { await invoke('open_curseforge_project', {projectId}); }
  catch { error.value = t('curseforge.openFailed'); }
}
async function install() {
  if (!plan.value?.ready || props.locked || working.value) return;
  const current = ++revision; working.value = true; applying.value = true; error.value = ''; emit('busy', true);
  try { await invoke('install_curseforge_mod_plan', {id: props.gameId, planId: plan.value.plan_id}); if (current === revision) { complete.value = true; emit('installed'); } }
  catch { if (current === revision) error.value = t('curseforge.installFailed'); }
  finally { if (current === revision) {working.value = false; applying.value = false;} emit('busy', false); }
}
watch(() => [props.gameId, props.projectId, props.fileId], () => void prepare(), {immediate: true});
onUnmounted(() => { ++revision; });
</script>
<template>
  <div class="cf-install-plan">
    <p
      v-if="complete"
      role="status"
    >
      {{ t('curseforge.installed') }}
    </p>
    <template v-else-if="plan">
      <ul class="mod-list">
        <li
          v-for="mod in plan.mods.filter(item => !item.required_by)"
          :key="mod.project_id"
          class="mod-row"
        >
          <div class="mod-info">
            <h3>{{ mod.name }}</h3><p class="mod-meta">
              {{ mod.version_number }}<span v-if="mod.required_by"> · {{ t('curseforge.required') }}</span>
            </p>
          </div>
        </li>
      </ul>
      <SettingsDisclosure
        v-if="plan.mods.some(item => item.required_by)"
        :key="plan.plan_id"
        :title="t('market.dependencies', { count: plan.mods.filter(item => item.required_by).length })"
      >
        <ul class="mod-list">
          <li
            v-for="mod in plan.mods.filter(item => item.required_by)"
            :key="mod.project_id"
            class="mod-row"
          >
            <div class="mod-info">
              <h3>{{ mod.name }}</h3><p class="mod-meta">
                {{ mod.version_number }} · {{ t('curseforge.required') }}
              </p>
            </div>
          </li>
        </ul>
      </SettingsDisclosure>
      <div
        v-for="file in plan.missing"
        :key="`${file.project_id}-${file.file_id}`"
        class="cf-missing"
      >
        <strong>{{ file.name }}</strong><p class="mod-meta">
          {{ file.file_name }}
        </p><p>{{ t(file.reason === 'manual' ? 'curseforge.manualHelp' : 'curseforge.archivedLocalHelp') }}</p><button
          v-if="file.website_url"
          class="text-button"
          :disabled="working"
          @click="openProject(file.project_id)"
        >
          {{ t('curseforge.openProject') }}
        </button><button
          class="secondary-button"
          :disabled="working || locked"
          @click="manual(file.project_id, file.file_id)"
        >
          {{ t(file.reason === 'manual' ? 'curseforge.chooseFile' : 'curseforge.chooseLocalFile') }}
        </button>
      </div>
      <button
        class="primary-button"
        :disabled="!plan.ready || locked || working"
        @click="install"
      >
        {{ t(applying ? 'curseforge.installing' : 'curseforge.install') }}
      </button>
      <p
        v-if="locked"
        class="mod-meta"
      >
        {{ t('curseforge.closeGame') }}
      </p>
    </template>
    <p
      v-if="working && !plan"
      role="status"
    >
      {{ t('curseforge.preparing') }}
    </p>
    <div
      v-if="error"
      role="alert"
      class="inline-error"
    >
      <p>{{ error }}</p><button
        v-if="!plan"
        class="text-button"
        :disabled="working"
        @click="prepare"
      >
        {{ t('curseforge.retry') }}
      </button>
    </div>
  </div>
</template>
<style scoped>.cf-install-plan { margin-top: 20px; }.cf-missing { padding: 16px 0; border-bottom: 1px solid var(--line); margin-bottom: 16px; }.cf-missing { overflow-wrap: anywhere; }.cf-missing p { margin: 8px 0; }</style>





