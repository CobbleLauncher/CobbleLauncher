<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import { t } from '../i18n';
import type { DownloadSettings } from '../download-settings';
import type { EnvironmentInstallationPreview, EnvironmentRequirements } from './types';

const props = defineProps<{ planId: string; source: DownloadSettings; useDefaultDirectory: boolean; disabled?: boolean }>();
const emit = defineEmits<{ ready: [value: boolean]; busy: [value: boolean]; invalid: [code: string] }>();
const preview = ref<EnvironmentInstallationPreview | null>(null);
const busy = ref(false);
const error = ref('');
let revision = 0;
const ready = computed(() => !!preview.value?.requirements.ready && preview.value.space.can_proceed && !busy.value && !error.value);
watch(ready, value => emit('ready', value), { immediate: true });
watch(busy, value => emit('busy', value));
function bytes(value: number) { return `${(value / 1024 / 1024 / 1024).toFixed(1)} GiB`; }
function report(cause: unknown) {
  const message = String(cause);
  const invalid = ['ENVIRONMENT_PLAN_EXPIRED', 'ENVIRONMENT_TARGET_UNSUPPORTED'].find(code => message.includes(code));
  if (invalid) emit('invalid', invalid);
  error.value = message.includes('ENVIRONMENT_PLAN_EXPIRED') ? t('reverseConfig.planExpired')
    : message.includes('ENVIRONMENT_MANUAL_FILE_MISMATCH') ? t('curseforge.fileMismatch')
      : message.includes('SPACE_INSUFFICIENT') ? t('reverseConfig.spaceInsufficient') : t('reverseConfig.preflightFailed');
}
async function prepare() {
  const current = ++revision;
  busy.value = true; error.value = ''; preview.value = null;
  try {
    const value = await invoke<EnvironmentInstallationPreview>('preview_environment_installation', { planId: props.planId, source: { ...props.source }, useDefaultDirectory: props.useDefaultDirectory });
    if (current === revision) preview.value = value;
  } catch (cause) { if (current === revision) report(cause); }
  finally { if (current === revision) busy.value = false; }
}
async function manual(projectId: string, fileId: string) {
  if (busy.value || props.disabled || !preview.value) return;
  const current = ++revision; busy.value = true; error.value = '';
  try {
    const value = await invoke<EnvironmentRequirements | null>('select_environment_manual_file', { planId: props.planId, projectId, fileId });
    if (current === revision && value && preview.value) preview.value.requirements = value;
  } catch (cause) { if (current === revision) report(cause); }
  finally { if (current === revision) busy.value = false; }
}
async function openProject(projectId: string) {
  try { await invoke('open_curseforge_project', { projectId }); }
  catch { error.value = t('curseforge.openFailed'); }
}
watch(() => [props.planId, props.source.source, props.source.mirror, props.useDefaultDirectory], () => void prepare(), { immediate: true });
onUnmounted(() => { ++revision; });
defineExpose({ prepare });
</script>
<template>
  <section
    class="environment-review"
    :aria-label="t('reverseConfig.installReview')"
    :aria-busy="busy"
  >
    <LoadingIndicator
      v-if="busy"
      :label="t('reverseConfig.preparingInstall')"
    />
    <template v-if="preview">
      <div
        v-for="volume in preview.space.volumes"
        :key="volume.volume.id"
        class="environment-space"
        :class="{ 'is-insufficient': volume.verdict === 'insufficient' }"
      >
        <span>{{ volume.volume.label }}</span><span>{{ t('reverseConfig.spaceNeeded', { value: bytes(volume.additional_peak_bytes) }) }}<template v-if="volume.volume.available_bytes !== null"> · {{ t('reverseConfig.spaceAvailable', { value: bytes(volume.volume.available_bytes) }) }}</template></span>
      </div>
      <div
        v-if="!preview.space.can_proceed"
      >
        <p
          class="settings-error"
          role="alert"
        >
          {{ t('reverseConfig.spaceInsufficient') }}
        </p><button
          type="button"
          class="text-button"
          :disabled="busy || disabled"
          @click="prepare"
        >
          {{ t('reverseConfig.recheckSpace') }}
        </button>
      </div>
      <p
        v-else-if="!preview.space.exact"
        class="setting-hint"
      >
        {{ t('reverseConfig.spaceEstimate') }}
      </p>
      <div
        v-for="file in preview.requirements.missing"
        :key="`${file.project_id}:${file.file_id}`"
        class="environment-manual-file"
      >
        <strong>{{ file.name }}</strong><span class="environment-file-name">{{ file.file_name }}</span>
        <p>{{ t(file.reason === 'manual' ? 'curseforge.manualHelp' : 'curseforge.archivedLocalHelp') }}</p>
        <div class="environment-file-actions">
          <button
            v-if="file.website_url"
            type="button"
            class="text-button"
            :disabled="busy || disabled"
            @click="openProject(file.project_id)"
          >
            {{ t('curseforge.openProject') }}
          </button><button
            type="button"
            class="secondary-button"
            :disabled="busy || disabled"
            @click="manual(file.project_id, file.file_id)"
          >
            {{ t('curseforge.chooseLocalFile') }}
          </button>
        </div>
      </div>
    </template>
    <div
      v-if="error"
      role="alert"
      class="environment-review-error"
    >
      <p>{{ error }}</p><button
        type="button"
        class="text-button"
        :disabled="busy || disabled"
        @click="prepare"
      >
        {{ t('createGame.retry') }}
      </button>
    </div>
  </section>
</template>
<style scoped>
.environment-review{display:grid;gap:var(--ui-space-3);max-width:680px;padding-block:var(--ui-space-4);border-top:1px solid var(--ui-border);font-size:var(--ui-text-label);line-height:var(--ui-leading-body)}
.environment-space{display:flex;justify-content:space-between;gap:var(--ui-space-4);flex-wrap:wrap;color:var(--ui-text-muted)}.environment-space.is-insufficient{color:var(--ui-danger)}
.environment-manual-file{display:grid;gap:var(--ui-space-2);padding:var(--ui-space-4);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md)}.environment-file-name{font-size:var(--ui-text-caption);color:var(--ui-text-muted);overflow-wrap:anywhere}
.environment-file-actions,.environment-review-error{display:flex;gap:var(--ui-space-3);align-items:center;flex-wrap:wrap}.environment-review-error{color:var(--ui-danger)}
</style>
