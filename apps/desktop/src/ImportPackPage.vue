<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import type { Game, PackPreview } from './games';
import { loaderLabel } from './games';
import { t } from './i18n';

const props = defineProps<{ initialPreview?: PackPreview | null }>();
const emit = defineEmits<{ created: [game: Game]; curseforge: [] }>();
const preview = ref<PackPreview | null>(null);
const name = ref('');
const optionalFiles = ref<string[]>([]);
const source = ref(defaultDownloadSettings());
const directory = ref('');
const useDefaultDirectory = ref(true);
const loading = ref(true);
const picking = ref(false);
const submitting = ref(false);
const error = ref('');
let disposed = false;

function settingsCause(cause: unknown) {
  const message = String(cause);
  if (message.includes('下载源设置无效')) return t('importPack.invalidDownloadSource');
  if (message.includes('请输入 HTTPS 镜像地址')) return t('importPack.invalidMirror');
  return message;
}

async function initialize() {
  try {
    source.value = await readDownloadSettings();
    directory.value = isTauri() ? await invoke<string>('default_game_directory') : t('importPack.defaultGameDirectory');
  } catch (cause) { error.value = t('importPack.readSettingsError', { cause: settingsCause(cause) }); }
  finally { loading.value = false; }
}
function applyPreview(selected: PackPreview) {
  preview.value = selected;
  name.value = selected.name;
  optionalFiles.value = [];
}
async function choosePack() {
  if (picking.value || submitting.value || loading.value) return;
  if (!isTauri()) { error.value = t('importPack.desktopOnly'); return; }
  picking.value = true;
  error.value = '';
  try {
    const selected = await invoke<PackPreview | null>('preview_mrpack');
    if (disposed || !selected) return;
    applyPreview(selected);
  } catch (cause) { if (!disposed) error.value = t('importPack.readArchiveError', { cause: String(cause) }); }
  finally { picking.value = false; }
}
async function chooseDirectory() {
  if (picking.value || submitting.value) return;
  picking.value = true;
  error.value = '';
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (!disposed && selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch (cause) { if (!disposed) error.value = t('importPack.chooseFolderError', { cause: String(cause) }); }
  finally { picking.value = false; }
}
async function create() {
  if (!preview.value || picking.value || submitting.value || loading.value) return;
  const downloadError = validateDownloadSettings(source.value);
  error.value = downloadError && source.value.source !== 'custom' ? t('importPack.invalidDownloadSource')
    : downloadError ? t('importPack.invalidMirror') : '';
  if (error.value) return;
  if (!name.value.trim()) { error.value = t('importPack.nameRequired'); return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_mrpack_game', { previewId: preview.value.preview_id, name: name.value.trim(), source: { ...source.value, mirror: source.value.mirror.trim() }, useDefaultDirectory: useDefaultDirectory.value, optionalFiles: [...optionalFiles.value] });
    if (!disposed) emit('created', game);
  } catch (cause) { if (!disposed) error.value = t('importPack.importError', { cause: String(cause) }); }
  finally { submitting.value = false; }
}
function fileSize(bytes: number) { return bytes < 1024 * 1024 ? `${Math.ceil(bytes / 1024)} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`; }
onMounted(() => { if (props.initialPreview) applyPreview(props.initialPreview); void initialize(); });
onUnmounted(() => { disposed = true; });
</script>

<template>
  <div class="import-page page-content">
    <button
      v-if="!initialPreview"
      class="text-button"
      :disabled="loading || picking || submitting"
      @click="emit('curseforge')"
    >
      {{ t('curseforge.importArchive') }}
    </button>
    <button
      v-if="!initialPreview"
      class="pack-picker"
      :class="{ 'has-pack': preview }"
      :disabled="loading || picking || submitting"
      @click="choosePack"
    >
      <svg
        viewBox="0 0 40 40"
        fill="none"
        aria-hidden="true"
      ><path d="m7 12 13-7 13 7-13 7zM7 12v16l13 7 13-7V12M20 19v16M13 9l13 7" /></svg>
      <span><strong>{{ picking ? t('importPack.reading') : preview ? t('importPack.changeArchive') : t('importPack.chooseArchive') }}</strong><span class="setting-hint">{{ t('importPack.archiveFormat') }}</span></span>
      <span
        aria-hidden="true"
        class="picker-arrow"
      >↗</span>
    </button>
    <form
      v-if="preview"
      class="create-form pack-form"
      @submit.prevent="create"
    >
      <section
        class="pack-summary"
        :aria-label="t('importPack.packInfo')"
      >
        <div class="pack-title">
          <h2>{{ preview.name }}</h2><span class="count">{{ preview.version_id }}</span>
        </div>
        <p
          v-if="preview.summary"
          class="preview-description"
        >
          {{ preview.summary }}
        </p>
        <div class="compatibility-tags">
          <span>Minecraft {{ preview.minecraft_version }}</span><span>{{ preview.loader_version ? `${loaderLabel(preview.loader_kind ?? 'fabric')} ${preview.loader_version}` : t('importPack.vanilla') }}</span>
        </div>
        <p class="setting-hint">
          {{ t('importPack.fileCounts', { required: preview.required_files, overrides: preview.overrides }) }}
        </p>
        <p
          v-if="preview.loader_kind === 'forge' || preview.loader_kind === 'neoforge'"
          class="setting-hint"
        >
          {{ t('importPack.installerWarning', { loader: loaderLabel(preview.loader_kind) }) }}
        </p>
      </section>
      <div
        v-if="preview.optional_files.length"
        class="pack-optionals"
      >
        <h3>{{ t('importPack.optionalFiles') }} <span class="count">{{ preview.optional_files.length }}</span></h3>
        <p class="setting-hint">
          {{ t('importPack.optionalFilesHint') }}
        </p>
        <label
          v-for="file in preview.optional_files"
          :key="file.path"
          class="pack-optional"
        >
          <input
            v-model="optionalFiles"
            type="checkbox"
            :value="file.path"
            :disabled="picking || submitting"
          >
          <span>{{ file.path }}</span><small>{{ fileSize(file.size) }}</small>
        </label>
      </div>
      <div class="form-field">
        <label for="pack-game-name">{{ t('importPack.gameName') }}</label>
        <input
          id="pack-game-name"
          v-model="name"
          maxlength="80"
          autocomplete="off"
          :disabled="picking || submitting"
        >
      </div>
      <div class="form-field">
        <label id="pack-source-label">{{ t('importPack.downloadSource') }}</label>
        <RadioGroupRoot
          v-model="source.source"
          class="source-options"
          :disabled="picking || submitting"
          aria-labelledby="pack-source-label"
        >
          <RadioGroupItem
            value="automatic"
            class="source-option"
          >
            {{ t('importPack.automaticRecommended') }}
          </RadioGroupItem>
          <RadioGroupItem
            value="official"
            class="source-option"
          >
            {{ t('importPack.official') }}
          </RadioGroupItem>
          <RadioGroupItem
            value="bmclapi"
            class="source-option"
          >
            BMCLAPI
          </RadioGroupItem>
          <RadioGroupItem
            v-if="source.mirror"
            value="custom"
            class="source-option"
          >
            {{ t('importPack.customMirror') }}
          </RadioGroupItem>
        </RadioGroupRoot>
        <p
          v-if="source.source === 'automatic'"
          class="setting-hint"
        >
          {{ t('importPack.automaticHint') }}
        </p>
        <template v-if="source.source === 'custom'">
          <label
            class="sr-only"
            for="pack-mirror"
          >{{ t('importPack.mirrorAddress') }}</label>
          <input
            id="pack-mirror"
            v-model="source.mirror"
            readonly
            type="url"
            placeholder="https://mirror.example.com/"
            :disabled="picking || submitting"
          >
        </template>
        <p class="setting-hint">
          {{ t('importPack.sourceNote') }}
        </p>
      </div>
      <div class="form-field">
        <label>{{ t('importPack.installLocation') }}</label>
        <button
          type="button"
          class="folder-button"
          :disabled="picking || submitting"
          @click="chooseDirectory"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
          ><path d="M3 7h7l2 2h9v11H3zM3 7V4h7l2 3" /></svg>
          <span>{{ directory || t('importPack.defaultGameDirectory') }}</span><span class="folder-change">{{ t('importPack.change') }}</span>
        </button>
        <p class="setting-hint">
          {{ t('importPack.installLocationHint') }}
        </p>
      </div>
      <button
        class="done-button"
        type="submit"
        :disabled="loading || picking || submitting"
      >
        {{ submitting ? t('importPack.creating') : t('importPack.install') }}
      </button>
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
