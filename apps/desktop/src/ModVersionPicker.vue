<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, useId, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import VersionSelector from './VersionSelector.vue';
import type { ModVersion, ModVersionDetails } from './games';
import { t } from './i18n';
const props = defineProps<{ gameId: string; projectId: string; disabled: boolean; currentVersionId?: string; preferredVersionId?: string }>();
const emit = defineEmits<{ selected: [version: ModVersion | null] }>();
const controlId = useId();
const versions = ref<ModVersion[]>([]);
const versionId = ref('');
const chosen = computed(() => versions.value.find((version) => version.version_id === versionId.value) ?? null);
const loading = ref(false);
const error = ref('');
const expanded = ref(false);
const details = ref<ModVersionDetails | null>(null);
const readingDetails = ref(false);
const detailsError = ref('');
let request = 0;
let detailsRequest = 0;
function versionType(value: string): string { return ['release', 'beta', 'alpha'].includes(value) ? t(`modUpdates.${value}`) : value; }
async function load() {
  const current = ++request;
  loading.value = true; error.value = ''; versions.value = []; versionId.value = '';
  try {
    const value = await invoke<ModVersion[]>('list_game_mod_versions', { id: props.gameId, projectId: props.projectId });
    if (current !== request) return;
    versions.value = value;
    versionId.value = value.find((version) => version.version_id === props.preferredVersionId)?.version_id
      ?? value.find((version) => version.version_id === props.currentVersionId)?.version_id
      ?? value.find((version) => version.version_type === 'release')?.version_id ?? value[0]?.version_id ?? '';
  } catch { if (current === request) error.value = t('modUpdates.loadFailed'); }
  finally { if (current === request) loading.value = false; }
}
async function readDetails() {
  if (!chosen.value || readingDetails.value || props.disabled) return;
  const current = ++detailsRequest;
  readingDetails.value = true; detailsError.value = '';
  try {
    const value = await invoke<ModVersionDetails>('get_game_mod_version_details', { id: props.gameId, projectId: props.projectId, versionId: chosen.value.version_id });
    if (current === detailsRequest) details.value = value;
  } catch { if (current === detailsRequest) detailsError.value = t('modUpdates.changelogFailed'); }
  finally { if (current === detailsRequest) readingDetails.value = false; }
}
function toggleDetails() {
  expanded.value = !expanded.value;
  if (expanded.value && !details.value) void readDetails();
}
watch(chosen, (value) => {
  ++detailsRequest; details.value = null; detailsError.value = ''; expanded.value = false; readingDetails.value = false;
  emit('selected', value);
}, { flush: 'sync' });
watch(() => [props.gameId, props.projectId], () => { void load(); }, { immediate: true });
onUnmounted(() => { ++request; ++detailsRequest; });
</script>
<template>
  <div class="mod-version-picker">
    <p
      v-if="loading"
      class="content-empty"
      role="status"
    >
      {{ t('modUpdates.loading') }}
    </p>
    <div
      v-else-if="error"
      class="inline-error"
      role="alert"
    >
      <p>{{ error }}</p><button
        class="text-button"
        :disabled="disabled"
        @click="load"
      >
        {{ t('modUpdates.retryLoad') }}
      </button>
    </div>
    <template v-else-if="versions.length">
      <div class="form-field">
        <label :for="`mod-version-${controlId}`">{{ t('modUpdates.versionLabel') }}</label>
        <VersionSelector
          :id="`mod-version-${controlId}`"
          v-model="versionId"
          :disabled="disabled"
          :current-value="currentVersionId"
          :options="versions.map(version => ({ value: version.version_id, label: `${version.name}${version.name !== version.version_number ? ` · ${version.version_number}` : ''} · ${versionType(version.version_type)} · ${version.date_published.slice(0, 10)}` }))"
        />
      </div>
      <button
        class="text-button mod-changelog-toggle"
        :aria-expanded="expanded"
        :disabled="disabled"
        @click="toggleDetails"
      >
        {{ t(expanded ? 'modUpdates.hideChangelog' : 'modUpdates.showChangelog') }}
      </button>
      <section
        v-if="expanded"
        class="mod-changelog"
        :aria-label="t('modUpdates.changelog')"
        tabindex="0"
      >
        <p
          v-if="readingDetails"
          class="setting-hint"
          role="status"
        >
          {{ t('modUpdates.readingChangelog') }}
        </p>
        <div
          v-else-if="detailsError"
          class="inline-error"
          role="alert"
        >
          <p>{{ detailsError }}</p><button
            class="text-button"
            :disabled="disabled"
            @click="readDetails"
          >
            {{ t('modUpdates.retryChangelog') }}
          </button>
        </div>
        <p
          v-else-if="details?.changelog.trim()"
          class="mod-changelog-text"
        >
          {{ details.changelog }}
        </p>
        <p
          v-else
          class="setting-hint"
        >
          {{ t('modUpdates.noChangelog') }}
        </p>
      </section>
    </template>
    <p
      v-else
      class="content-empty"
    >
      {{ t('modUpdates.noCompatibleVersions') }}
    </p>
  </div>
</template>
