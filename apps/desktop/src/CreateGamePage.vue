<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import CompatibilityNotice from './CompatibilityNotice.vue';
import { t } from './i18n';
import VersionSelector from './VersionSelector.vue';
import MinecraftVersionTiles from './MinecraftVersionTiles.vue';
import { suggestedGameName } from './version-selector';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type FabricVersion, type Game, type GameVersion, type LoaderVersion } from './games';
import ReverseConfiguration from './reverse-config/ReverseConfiguration.vue';
import EnvironmentInstallReview from './reverse-config/EnvironmentInstallReview.vue';
import type { ReverseChoice } from './reverse-config/types';
import AppIcon from './ui/AppIcon.vue';

const emit = defineEmits<{ created: [game: Game] }>();
const mode = ref('version');
const reverseChoice = ref<ReverseChoice | null>(null);
const reverseReady = ref(false);
const reversePreparing = ref(false);
const environmentReview = ref<InstanceType<typeof EnvironmentInstallReview>>();
const reverseConfiguration = ref<InstanceType<typeof ReverseConfiguration>>();
const reverseCreateError = ref('');
const showInstallOptions = ref(false);
const name = ref('Minecraft');
const manuallyNamed = ref(false);
const version = ref('');
const loader = ref('vanilla');
const loaderVersion = ref('');
const loaderVersions = ref<LoaderVersion[]>([]);
const suggestedName = computed(() => mode.value === 'mods' && reverseChoice.value
  ? suggestedGameName(reverseChoice.value.game_version, reverseChoice.value.loader, '')
  : suggestedGameName(version.value, loader.value, loaderVersion.value));
watch(suggestedName, (value) => { if (!manuallyNamed.value) name.value = value; });
function editName(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  manuallyNamed.value = !!value.trim();
  name.value = manuallyNamed.value ? value : suggestedName.value;
}
const selectedLoaderLabel = computed(() => loaderLabel(loader.value));
const loadingLoader = ref(false);
const loaderError = ref('');
const compatibilityBlocked = ref(false);
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
watch(mode, () => { reverseChoice.value = null; error.value = ''; reverseCreateError.value = ''; showInstallOptions.value = false; });
function chooseReverseConfiguration(choice: ReverseChoice | null) {
  reverseChoice.value = choice; reverseReady.value = false; error.value = ''; reverseCreateError.value = '';
}
function updateReverseReady(ready: boolean) {
  reverseReady.value = ready;
  if (ready && ['SPACE_INSUFFICIENT', 'ENVIRONMENT_MANUAL_FILES_REQUIRED'].includes(reverseCreateError.value)) { reverseCreateError.value = ''; error.value = ''; }
}

async function loadVersions() {
  const request = ++versionRequest;
  const previousVersion = version.value;
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
    version.value = (result.find(item => item.id === previousVersion) ?? result.find(item => item.kind === 'release') ?? result[0])?.id ?? '';
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
  if (submitting.value || (mode.value === 'version' && compatibilityBlocked.value)) return;
  if (mode.value === 'mods' && (!reverseReady.value || reversePreparing.value || ['ENVIRONMENT_PLAN_EXPIRED', 'ENVIRONMENT_TARGET_UNSUPPORTED'].includes(reverseCreateError.value))) return;
  reverseCreateError.value = '';
  error.value = validateDownloadSettings(source.value) ? t('createGame.sourceInvalid') : '';
  if (error.value) return;
  if (mode.value === 'mods') {
    if (!reverseChoice.value || !name.value.trim()) { error.value = t('reverseConfig.chooseConfiguration'); return; }
    submitting.value = true;
    try {
      const game = await invoke<Game>('create_game_from_environment', { planId: reverseChoice.value.plan_id, name: name.value.trim(), automaticName: !manuallyNamed.value, source: { ...source.value }, useDefaultDirectory: useDefaultDirectory.value });
      emit('created', game);
    } catch (cause) {
      const message = String(cause);
      reverseCreateError.value = ['ENVIRONMENT_PLAN_EXPIRED', 'ENVIRONMENT_PLAN_BUSY', 'ENVIRONMENT_LOADER_UNAVAILABLE', 'ENVIRONMENT_TARGET_UNSUPPORTED', 'ENVIRONMENT_MANUAL_FILES_REQUIRED', 'SPACE_INSUFFICIENT'].find(code => message.includes(code)) ?? '';
      error.value = reverseCreateError.value === 'ENVIRONMENT_PLAN_EXPIRED' ? t('reverseConfig.planExpired')
        : reverseCreateError.value === 'ENVIRONMENT_PLAN_BUSY' ? t('reverseConfig.planBusy')
          : reverseCreateError.value === 'ENVIRONMENT_LOADER_UNAVAILABLE' ? t('reverseConfig.loaderUnavailable')
            : reverseCreateError.value === 'ENVIRONMENT_TARGET_UNSUPPORTED' ? t('reverseConfig.targetUnsupported')
              : reverseCreateError.value === 'SPACE_INSUFFICIENT' ? t('reverseConfig.spaceInsufficient')
                : reverseCreateError.value === 'ENVIRONMENT_MANUAL_FILES_REQUIRED' ? t('reverseConfig.manualFilesRequired')
              : t('reverseConfig.createFailed');
      if (['SPACE_INSUFFICIENT', 'ENVIRONMENT_MANUAL_FILES_REQUIRED'].includes(reverseCreateError.value)) void environmentReview.value?.prepare();
    }
    finally { submitting.value = false; }
    return;
  }
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
  <div
    class="create-page page-content"
    :class="{ 'create-from-mods': mode === 'mods' }"
  >
    <RadioGroupRoot
      v-model="mode"
      class="create-mode-options"
      :disabled="submitting"
      :aria-label="t('reverseConfig.creationMode')"
    >
      <RadioGroupItem
        value="version"
        class="create-mode-option"
      >
        {{ t('reverseConfig.manualMode') }}
      </RadioGroupItem>
      <RadioGroupItem
        value="mods"
        class="create-mode-option"
      >
        {{ t('reverseConfig.modsMode') }}
      </RadioGroupItem>
    </RadioGroupRoot>
    <form
      class="create-form"
      @submit.prevent="create"
    >
      <ReverseConfiguration
        v-if="mode === 'mods'"
        ref="reverseConfiguration"
        :disabled="submitting"
        @selected="chooseReverseConfiguration"
      />
      <div
        v-if="mode === 'version' || reverseChoice"
        class="form-field"
      >
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
      <div
        v-if="mode === 'version'"
        class="form-field"
      >
        <label id="game-version-label">{{ t('createGame.version') }}</label>
        <div class="version-control">
          <MinecraftVersionTiles
            id="game-version"
            v-model="version"
            labelled-by="game-version-label"
            :disabled="loading || submitting || !versions.length"
            :versions="versions"
            :loading="loading"
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
      <div
        v-if="mode === 'version'"
        class="form-field"
      >
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
        v-if="mode === 'version' && loader !== 'vanilla'"
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
      <CompatibilityNotice
        v-if="mode === 'version'"
        :minecraft-version="version"
        :loader-kind="loader"
        :loader-version="loaderVersion"
        @blocked="compatibilityBlocked = $event"
      />
      <button
        v-if="mode === 'version' || reverseChoice"
        type="button"
        class="text-button create-install-options"
        :aria-expanded="showInstallOptions"
        aria-controls="create-install-source create-install-directory"
        :disabled="submitting"
        @click="showInstallOptions = !showInstallOptions"
      >
        <AppIcon name="settings" />{{ t('reverseConfig.installOptions') }}<AppIcon name="chevron-down" />
      </button>
      <div
        v-if="showInstallOptions && (mode === 'version' || reverseChoice)"
        id="create-install-source"
        class="form-field"
      >
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
          v-if="mode === 'version' && source.source === 'automatic'"
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
      <div
        v-if="showInstallOptions && (mode === 'version' || reverseChoice)"
        id="create-install-directory"
        class="form-field"
      >
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
        <p
          v-if="mode === 'version'"
          class="setting-hint"
        >
          {{ t('createGame.directoryHelp') }}
        </p>
      </div>
      <EnvironmentInstallReview
        v-if="mode === 'mods' && reverseChoice"
        ref="environmentReview"
        :key="reverseChoice.plan_id"
        :plan-id="reverseChoice.plan_id"
        :source="source"
        :use-default-directory="useDefaultDirectory"
        :disabled="submitting"
        @ready="updateReverseReady"
        @busy="reversePreparing = $event"
        @invalid="reverseCreateError = $event"
      />
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <button
        v-if="mode === 'mods' && ['ENVIRONMENT_PLAN_EXPIRED', 'ENVIRONMENT_TARGET_UNSUPPORTED'].includes(reverseCreateError)"
        type="button"
        class="text-button create-resolve-again"
        @click="reverseConfiguration?.solve()"
      >
        {{ t('reverseConfig.findAgain') }}
      </button>
      <button
        v-if="mode === 'version' || reverseChoice"
        class="done-button"
        type="submit"
        :disabled="submitting || (mode === 'mods' && (!reverseReady || reversePreparing || ['ENVIRONMENT_PLAN_EXPIRED', 'ENVIRONMENT_TARGET_UNSUPPORTED'].includes(reverseCreateError))) || (mode === 'version' && (compatibilityBlocked || loading || !version || (loader !== 'vanilla' && (loadingLoader || !loaderVersion))))"
      >
        {{ t(submitting ? 'createGame.creating' : mode === 'mods' ? 'reverseConfig.create' : 'createGame.create') }}
      </button>
    </form>
  </div>
</template>

<style scoped>
.create-page:not(.create-from-mods){max-width:800px}.create-form{gap:var(--ui-space-5)}.create-form>.form-field>label{font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:600;margin-bottom:var(--ui-space-2)}.create-form>.form-field>input{min-height:var(--ui-control-height);font-size:var(--ui-text-body)}.create-form .source-option{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}.create-form>.done-button{font-size:var(--ui-text-label);min-height:var(--ui-control-comfortable)}
.create-mode-options { display: flex; width: fit-content; gap: 4px; padding: 4px; border: 1px solid var(--line); border-radius: 11px; margin-bottom: 26px; background: var(--panel); }
.create-mode-option { border: 0; background: transparent; color: var(--muted); border-radius: 7px; padding: 10px 16px; font-size: var(--ui-text-body); }
.create-mode-option[data-state=checked] { background: var(--hover); color: var(--ink); font-weight: 600; }
.create-from-mods { max-width: 1000px; }
.create-from-mods .create-form { max-width: none; }
.create-from-mods .reverse-config { margin-bottom: 28px; }
.create-from-mods .create-form > .form-field { max-width: 520px; }
.create-from-mods .done-button { max-width: 520px; }
.create-install-options { display: flex; align-items: center; align-self: flex-start; gap: 8px; padding: 4px 0; color: var(--muted); }
.create-install-options[aria-expanded=true] > .app-icon:last-child { transform: rotate(180deg); }
</style>

