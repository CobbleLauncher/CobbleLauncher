<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import VersionSelector from './VersionSelector.vue';
import { suggestedGameName } from './version-selector';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type FabricVersion, type Game, type GameVersion, type LoaderVersion } from './games';

const emit = defineEmits<{ created: [game: Game] }>();
const name = ref('Minecraft');
const manuallyNamed = ref(false);
const version = ref('');
const loader = ref('vanilla');
const loaderVersion = ref('');
const loaderVersions = ref<LoaderVersion[]>([]);
const suggestedName = computed(() => suggestedGameName(version.value, loader.value, loaderVersion.value));
watch(suggestedName, (value) => { if (!manuallyNamed.value) name.value = value; });
function editName(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  manuallyNamed.value = !!value.trim();
  name.value = manuallyNamed.value ? value : suggestedName.value;
}
const selectedLoaderLabel = computed(() => loaderLabel(loader.value));
const loadingLoader = ref(false);
const loaderError = ref('');
let loaderRequest = 0;
async function loadLoaderVersions() {
  const request = ++loaderRequest;
  loaderVersion.value = '';
  loaderVersions.value = [];
  loaderError.value = '';
  loadingLoader.value = false;
  if (loader.value === 'vanilla' || !version.value || !isTauri()) return;
  const sourceError = validateDownloadSettings(source.value);
  if (sourceError) { loaderError.value = t('createGame.sourceInvalid'); return; }
  const kind = loader.value;
  const label = loaderLabel(kind);
  loadingLoader.value = true;
  try {
    const result = kind === 'fabric'
      ? (await invoke<FabricVersion[]>('list_fabric_versions', { version: version.value, source: { ...source.value } })).map((item) => ({ version: item.version, recommended: item.stable }))
      : await invoke<LoaderVersion[]>('list_loader_versions', { version: version.value, kind, source: { ...source.value } });
    if (request !== loaderRequest) return;
    loaderVersions.value = result;
    loaderVersion.value = (result.find((item) => item.recommended) ?? result[0])?.version ?? '';
    if (!result.length) loaderError.value = t('createGame.loaderEmpty', {loader: label});
  } catch {
    if (request === loaderRequest) loaderError.value = t('createGame.loaderFailed', {loader: label});
  } finally { if (request === loaderRequest) loadingLoader.value = false; }
}
const versions = ref<GameVersion[]>([]);
const source = ref(defaultDownloadSettings());
watch([loader, version, () => source.value.source, () => source.value.mirror], () => { void loadLoaderVersions(); });
const directory = ref('');
const useDefaultDirectory = ref(true);
const loading = ref(false);
const submitting = ref(false);
const error = ref('');
let versionRequest = 0;

async function loadVersions() {
  const request = ++versionRequest;
  loading.value = false;
  versions.value = [];
  version.value = '';
  error.value = validateDownloadSettings(source.value) ? t('createGame.sourceInvalid') : '';
  if (error.value || !isTauri()) return;
  loading.value = true;
  try {
    const result = await invoke<GameVersion[]>('list_game_versions', { source: { ...source.value } });
    if (request !== versionRequest) return;
    versions.value = result;
    version.value = result[0]?.id ?? '';
    if (!result.length) error.value = t('createGame.versionsEmpty');
  } catch {
    if (request === versionRequest) error.value = t('createGame.versionsFailed');
  } finally {
    if (request === versionRequest) loading.value = false;
  }
}

async function show() {
  error.value = '';
  useDefaultDirectory.value = true;
  try {
    source.value = await readDownloadSettings();
    directory.value = isTauri() ? await invoke<string>('default_game_directory') : t('createGame.defaultDirectory');
    await loadVersions();
    if (!isTauri()) error.value = t('createGame.desktopOnly');
  } catch { error.value = t('createGame.settingsFailed'); }
}

async function changeSource(value: unknown) {
  if (value !== 'automatic' && value !== 'official' && value !== 'bmclapi' && value !== 'custom') return;
  source.value.source = value;
  await loadVersions();
}
function editMirror() {
  ++versionRequest;
  loading.value = false;
  version.value = '';
  versions.value = [];
  error.value = '';
}

async function chooseDirectory() {
  if (!isTauri()) { error.value = t('createGame.desktopOnly'); return; }
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch { error.value = t('createGame.directoryFailed'); }
}

async function create() {
  if (!isTauri()) { error.value = t('createGame.desktopOnly'); return; }
  if (submitting.value) return;
  error.value = validateDownloadSettings(source.value) ? t('createGame.sourceInvalid') : '';
  if (error.value) return;
  if (!name.value.trim() || !version.value) { error.value = t('createGame.nameVersionRequired'); return; }
  if (loader.value !== 'vanilla' && (loadingLoader.value || !loaderVersion.value)) { error.value = t('createGame.loaderRequired', {loader: selectedLoaderLabel.value}); return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_game', { name: name.value.trim(), automaticName: !manuallyNamed.value, version: version.value, loaderKind: loader.value === 'vanilla' ? null : loader.value, loaderVersion: loader.value !== 'vanilla' ? loaderVersion.value : null, source: { ...source.value }, useDefaultDirectory: useDefaultDirectory.value });
    emit('created', game);
  } catch { error.value = t('createGame.createFailed'); }
  finally { submitting.value = false; }
}

onMounted(() => { void show(); });
onUnmounted(() => { ++versionRequest; ++loaderRequest; });
</script>

<template>
  <div class="create-page page-content">
    <form
      class="create-form"
      @submit.prevent="create"
    >
      <div class="form-field">
        <label for="game-name">{{ t('createGame.name') }}</label>
        <input
          id="game-name"
          :value="name"
          maxlength="80"
          autocomplete="off"
          :disabled="submitting"
          @input="editName"
        >
      </div>
      <div class="form-field">
        <label id="create-source-label">{{ t('createGame.source') }}</label>
        <RadioGroupRoot
          class="source-options"
          :model-value="source.source"
          :disabled="submitting"
          aria-labelledby="create-source-label"
          @update:model-value="changeSource"
        >
          <RadioGroupItem
            value="automatic"
            class="source-option"
          >
            {{ t('createGame.automatic') }}
          </RadioGroupItem>
          <RadioGroupItem
            value="official"
            class="source-option"
          >
            {{ t('createGame.official') }}
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
            {{ t('createGame.custom') }}
          </RadioGroupItem>
        </RadioGroupRoot>
        <p
          v-if="source.source === 'automatic'"
          class="setting-hint"
        >
          {{ t('createGame.automaticHelp') }}
        </p>
        <div
          v-if="source.source === 'custom'"
          class="custom-version-source"
        >
          <label
            class="sr-only"
            for="create-mirror"
          >{{ t('createGame.mirror') }}</label>
          <input
            id="create-mirror"
            v-model="source.mirror"
            readonly
            type="url"
            placeholder="https://mirror.example.com/"
            :disabled="submitting"
            @input="editMirror"
            @change="loadVersions"
          >
          <button
            type="button"
            class="text-button"
            :disabled="loading || submitting"
            @click="loadVersions"
          >
            {{ t('createGame.refresh') }}
          </button>
        </div>
      </div>
      <div class="form-field">
        <label for="game-version">{{ t('createGame.version') }} <span class="field-note">{{ t('createGame.release') }}</span></label>
        <div class="version-control">
          <VersionSelector
            id="game-version"
            v-model="version"
            :disabled="loading || submitting || !versions.length"
            :options="versions.map(item => ({ value: item.id, label: item.id }))"
            :placeholder="t(loading ? 'createGame.loadingVersions' : 'createGame.noVersions')"
          />
          <button
            v-if="!loading && !versions.length && source.source !== 'custom'"
            type="button"
            class="text-button"
            @click="loadVersions"
          >
            {{ t('createGame.retry') }}
          </button>
        </div>
      </div>
      <div class="form-field">
        <label id="loader-label">{{ t('createGame.type') }}</label>
        <RadioGroupRoot
          v-model="loader"
          class="source-options"
          :disabled="submitting"
          aria-labelledby="loader-label"
        >
          <RadioGroupItem
            value="vanilla"
            class="source-option"
          >
            {{ t('createGame.vanilla') }}
          </RadioGroupItem>
          <RadioGroupItem
            value="fabric"
            class="source-option"
          >
            Fabric
          </RadioGroupItem>
          <RadioGroupItem
            value="forge"
            class="source-option"
          >
            Forge
          </RadioGroupItem>
          <RadioGroupItem
            value="neoforge"
            class="source-option"
          >
            NeoForge
          </RadioGroupItem>
        </RadioGroupRoot>
        <p
          v-if="loader === 'forge' || loader === 'neoforge'"
          class="setting-hint"
        >
          {{ t('createGame.installer', {loader: selectedLoaderLabel}) }}
        </p>
      </div>
      <div
        v-if="loader !== 'vanilla'"
        class="form-field"
      >
        <label for="loader-version">{{ t('createGame.loaderVersion', {loader: selectedLoaderLabel}) }}</label>
        <div class="version-control">
          <VersionSelector
            id="loader-version"
            v-model="loaderVersion"
            :disabled="loadingLoader || submitting || !loaderVersions.length"
            :options="loaderVersions.map(item => ({ value: item.version, label: `${item.version}${item.recommended ? ` · ${t(loader === 'fabric' ? 'createGame.stable' : 'createGame.recommended')}` : ''}` }))"
            :placeholder="t(loadingLoader ? 'createGame.loadingVersions' : 'createGame.noVersions')"
          />
          <button
            v-if="loaderError"
            type="button"
            class="text-button"
            :disabled="loadingLoader || submitting"
            @click="loadLoaderVersions"
          >
            {{ t('createGame.retry') }}
          </button>
        </div>
        <p
          v-if="loaderError"
          class="settings-error"
          role="alert"
        >
          {{ loaderError }}
        </p>
      </div>
      <div class="form-field">
        <label>{{ t('createGame.location') }}</label>
        <button
          type="button"
          class="folder-button"
          :disabled="submitting"
          @click="chooseDirectory"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
          ><path d="M3 7h7l2 2h9v11H3zM3 7V4h7l2 3" /></svg>
          <span>{{ directory || t('createGame.defaultDirectory') }}</span><span class="folder-change">{{ t('createGame.change') }}</span>
        </button>
        <p class="setting-hint">
          {{ t('createGame.directoryHelp') }}
        </p>
      </div>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <button
        class="done-button"
        type="submit"
        :disabled="loading || submitting || (loader !== 'vanilla' && (loadingLoader || !loaderVersion))"
      >
        {{ t(submitting ? 'createGame.creating' : 'createGame.create') }}
      </button>
    </form>
  </div>
</template>

