<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type Game, type LauncherImportPreview } from './games';
import { t } from './i18n';

const emit = defineEmits<{ created: [game: Game]; busy: [value: boolean] }>();
const preview = ref<LauncherImportPreview | null>(null);
const candidateId = ref('');
const contentId = ref('');
const candidate = computed(() => preview.value?.candidates.find((item) => item.candidate_id === candidateId.value));
const contentRoot = computed(() => candidate.value?.content_roots.find((item) => item.content_id === contentId.value));
const name = ref('');
const includeSaves = ref(false);
const source = ref(defaultDownloadSettings());
const directory = ref('');
const useDefaultDirectory = ref(true);
const loading = ref(true);
const picking = ref<'source' | 'destination' | null>(null);
const submitting = ref(false);
const busy = computed(() => picking.value !== null || submitting.value);
const error = ref('');
let disposed = false;

watch(busy, (value) => emit('busy', value), { flush: 'sync' });
watch(candidateId, () => {
  name.value = candidate.value?.name ?? '';
  contentId.value = candidate.value?.content_roots.length === 1 ? candidate.value.content_roots[0]!.content_id : '';
  includeSaves.value = false;
  error.value = '';
}, { flush: 'sync' });
watch(contentId, () => { includeSaves.value = false; }, { flush: 'sync' });

async function initialize() {
  try { source.value = await readDownloadSettings(); directory.value = isTauri() ? await invoke<string>('default_game_directory') : t('launcherImport.defaultGameDirectory'); }
  catch (cause) { if (!disposed) error.value = t('launcherImport.readLocationError', { cause: String(cause) }); }
  finally { loading.value = false; }
}
async function chooseSource() {
  if (busy.value || loading.value) return;
  if (!isTauri()) { error.value = t('launcherImport.desktopOnly'); return; }
  picking.value = 'source'; error.value = '';
  try {
    const selected = await invoke<LauncherImportPreview | null>('preview_launcher_import');
    if (disposed || !selected) return;
    candidateId.value = '';
    preview.value = selected;
    contentId.value = ''; name.value = ''; includeSaves.value = false;
    if (selected.candidates.length === 1) candidateId.value = selected.candidates[0]!.candidate_id;
  } catch (cause) { if (!disposed) error.value = t('launcherImport.readSourceError', { cause: String(cause) }); }
  finally { picking.value = null; }
}
async function chooseDirectory() {
  if (busy.value || loading.value) return;
  picking.value = 'destination'; error.value = '';
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (!disposed && selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch (cause) { if (!disposed) error.value = t('launcherImport.chooseFolderError', { cause: String(cause) }); }
  finally { picking.value = null; }
}
async function create() {
  if (busy.value || loading.value || !preview.value || !candidate.value?.supported || !contentRoot.value) return;
  const downloadError = validateDownloadSettings(source.value);
  error.value = downloadError && source.value.source !== 'custom' ? t('launcherImport.invalidDownloadSource')
    : downloadError ? t('launcherImport.invalidMirror') : '';
  if (error.value) return;
  const trimmedName = name.value.trim();
  if (!trimmedName) { error.value = t('launcherImport.nameRequired'); return; }
  if (trimmedName.length > 80) { error.value = t('launcherImport.nameTooLong'); return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_launcher_import', {
      previewId: preview.value.preview_id, candidateId: candidate.value.candidate_id,
      contentId: contentRoot.value.content_id, name: trimmedName,
      includeSaves: contentRoot.value.has_saves && includeSaves.value,
      source: { ...source.value, mirror: source.value.mirror.trim() }, useDefaultDirectory: useDefaultDirectory.value,
    });
    if (!disposed) emit('created', game);
  } catch (cause) { if (!disposed) error.value = t('launcherImport.importError', { cause: String(cause) }); }
  finally { submitting.value = false; }
}
onMounted(() => { void initialize(); });
onUnmounted(() => { disposed = true; emit('busy', false); });
</script>

<template>
  <div class="import-launcher-page page-content">
    <p class="create-description">
      {{ t('launcherImport.description') }}
    </p>
    <button
      class="pack-picker"
      :class="{ 'has-pack': preview }"
      :disabled="loading || busy"
      @click="chooseSource"
    >
      <svg
        viewBox="0 0 40 40"
        fill="none"
        aria-hidden="true"
      ><path d="M5 12h12l4 4h14v17H5zM5 12V7h12l4 5M20 21v8m-4-4 4 4 4-4" /></svg>
      <span><strong>{{ picking === 'source' ? t('launcherImport.reading') : preview ? t('launcherImport.changeSource') : t('launcherImport.chooseSource') }}</strong><span class="setting-hint">{{ t('launcherImport.sourceHint') }}</span></span>
      <span
        class="picker-arrow"
        aria-hidden="true"
      >↗</span>
    </button>
    <form
      v-if="preview"
      class="create-form pack-form"
      @submit.prevent="create"
    >
      <section
        class="launcher-import-source"
        :aria-label="t('launcherImport.sourceInfo')"
      >
        <p class="launcher-source-label">
          {{ t('launcherImport.sourceDirectory') }}
        </p>
        <p class="pack-origin">
          {{ preview.source_display }}
        </p>
      </section>
      <p
        v-if="!preview.candidates.length"
        class="profile-notice"
      >
        {{ t('launcherImport.noCandidates') }}
      </p>
      <div
        v-else
        class="form-field"
      >
        <label for="launcher-import-candidate">{{ t('launcherImport.sourceGame') }}</label>
        <select
          id="launcher-import-candidate"
          v-model="candidateId"
          :disabled="busy"
        >
          <option
            disabled
            value=""
          >
            {{ t('launcherImport.selectSourceGame') }}
          </option>
          <option
            v-for="item in preview.candidates"
            :key="item.candidate_id"
            :value="item.candidate_id"
          >
            {{ item.name }}{{ item.supported ? '' : ` (${t('launcherImport.unsupported')})` }}
          </option>
        </select>
      </div>
      <template v-if="candidate">
        <section
          class="launcher-import-summary"
          :aria-label="t('launcherImport.gameInfo')"
        >
          <div class="compatibility-tags">
            <span v-if="candidate.minecraft_version">Minecraft {{ candidate.minecraft_version }}</span>
            <span v-if="candidate.supported || candidate.loader_kind">{{ candidate.loader_kind ? `${loaderLabel(candidate.loader_kind)} ${candidate.loader_version ?? ''}` : t('launcherImport.vanilla') }}</span>
          </div>
          <p
            v-if="!candidate.supported"
            class="settings-error"
            role="status"
          >
            {{ candidate.unsupported_reason || t('launcherImport.unsupportedImport') }}
          </p>
          <ul
            v-if="candidate.warnings.length"
            class="launcher-import-warnings"
          >
            <li
              v-for="warning in candidate.warnings"
              :key="warning"
            >
              {{ warning }}
            </li>
          </ul>
        </section>
        <template v-if="candidate.supported">
          <div
            v-if="candidate.content_roots.length > 1"
            class="form-field"
          >
            <label for="launcher-import-root">{{ t('launcherImport.gameDataDirectory') }}</label>
            <select
              id="launcher-import-root"
              v-model="contentId"
              :disabled="busy"
            >
              <option
                disabled
                value=""
              >
                {{ t('launcherImport.selectDataDirectory') }}
              </option>
              <option
                v-for="root in candidate.content_roots"
                :key="root.content_id"
                :value="root.content_id"
              >
                {{ root.label }}
              </option>
            </select>
            <p class="setting-hint">
              {{ t('launcherImport.dataDirectoryHint') }}
            </p>
          </div>
          <p
            v-else-if="!candidate.content_roots.length"
            class="profile-notice"
          >
            {{ t('launcherImport.noDataDirectory') }}
          </p>
          <template v-if="contentRoot">
            <label
              v-if="contentRoot.has_saves"
              class="pack-optional launcher-import-saves"
            >
              <input
                v-model="includeSaves"
                type="checkbox"
                :disabled="busy"
              ><span>{{ t('launcherImport.includeSaves') }}</span>
            </label>
            <div class="form-field">
              <label for="launcher-import-name">{{ t('launcherImport.gameName') }}</label>
              <input
                id="launcher-import-name"
                v-model="name"
                maxlength="80"
                autocomplete="off"
                :disabled="busy"
              >
            </div>
            <div class="form-field">
              <label id="launcher-import-source-label">{{ t('launcherImport.downloadSource') }}</label>
              <RadioGroupRoot
                v-model="source.source"
                class="source-options"
                :disabled="busy"
                aria-labelledby="launcher-import-source-label"
              >
                <RadioGroupItem
                  value="automatic"
                  class="source-option"
                >
                  {{ t('launcherImport.automaticRecommended') }}
                </RadioGroupItem>
                <RadioGroupItem
                  value="official"
                  class="source-option"
                >
                  {{ t('launcherImport.official') }}
                </RadioGroupItem>
                <RadioGroupItem
                  value="bmclapi"
                  class="source-option"
                >
                  {{ t('launcherImport.bmclapi') }}
                </RadioGroupItem>
                <RadioGroupItem
                  v-if="source.mirror"
                  value="custom"
                  class="source-option"
                >
                  {{ t('launcherImport.customMirror') }}
                </RadioGroupItem>
              </RadioGroupRoot>
              <p
                v-if="source.source === 'automatic'"
                class="setting-hint"
              >
                {{ t('launcherImport.automaticHint') }}
              </p>
              <template v-if="source.source === 'custom'">
                <label
                  class="sr-only"
                  for="launcher-import-mirror"
                >{{ t('launcherImport.mirrorAddress') }}</label>
                <input
                  id="launcher-import-mirror"
                  v-model="source.mirror"
                  readonly
                  type="url"
                  placeholder="https://mirror.example.com/"
                  :disabled="busy"
                >
              </template>
            </div>
            <div class="form-field">
              <label>{{ t('launcherImport.installLocation') }}</label>
              <button
                type="button"
                class="folder-button"
                :disabled="busy"
                @click="chooseDirectory"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  aria-hidden="true"
                ><path d="M3 7h7l2 2h9v11H3zM3 7V4h7l2 3" /></svg>
                <span>{{ directory || t('launcherImport.defaultGameDirectory') }}</span><span class="folder-change">{{ t('launcherImport.change') }}</span>
              </button>
              <p class="setting-hint">
                {{ t('launcherImport.installLocationHint') }}
              </p>
            </div>
            <p class="launcher-import-copy-hint">
              {{ t('launcherImport.copyNotice') }}
            </p>
          </template>
        </template>
        <button
          class="done-button"
          type="submit"
          :disabled="loading || busy || !candidate.supported || !contentRoot"
        >
          {{ submitting ? t('launcherImport.creating') : t('launcherImport.importButton') }}
        </button>
      </template>
    </form>
    <p
      v-if="error"
      class="settings-error pack-error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
