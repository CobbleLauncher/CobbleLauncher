<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { t } from './i18n';
import type { Game } from './games';
import { curseForgeLoaderLabel, type CurseForgePackPreview } from './curseforge';
import CurseForgePackPreparation from './CurseForgePackPreparation.vue';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
const props = defineProps<{initialPreview?: CurseForgePackPreview | null}>();
const emit = defineEmits<{created: [game: Game]; busy: [value: boolean]}>();
const preview = ref(props.initialPreview ?? null);
const name = ref(props.initialPreview?.name ?? '');
const source = ref(defaultDownloadSettings());
const loading = ref(true);
const working = ref(false);
const submitting = ref(false);
const directory = ref('');
const useDefaultDirectory = ref(true);
const error = ref('');
let disposed = false;
function busy(value: boolean) { working.value = value; emit('busy', value); }
function selected(value: CurseForgePackPreview) { preview.value = value; name.value = value.name; }
async function choosePack() {
  if (working.value) return;
  busy(true); error.value = '';
  try { const value = await invoke<CurseForgePackPreview | null>('preview_curseforge_pack'); if (!disposed && value) selected(value); }
  catch { if (!disposed) error.value = t('curseforge.packPrepareFailed'); }
  finally { submitting.value = false; busy(false); }
}
async function chooseDirectory() {
  if (working.value) return;
  busy(true); error.value = '';
  try { const value = await invoke<string | null>('pick_game_directory'); if (!disposed && value) {directory.value = value; useDefaultDirectory.value = false;} }
  catch { if (!disposed) error.value = t('curseforge.directoryFailed'); }
  finally { submitting.value = false; busy(false); }
}
async function create() {
  if (working.value || loading.value || !preview.value?.ready || !preview.value.plan_id) return;
  error.value = '';
  if (!name.value.trim()) {error.value = t('curseforge.nameRequired'); return;}
  if (validateDownloadSettings(source.value)) {error.value = t('curseforge.sourceInvalid'); return;}
  busy(true); submitting.value = true;
  try { const game = await invoke<Game>('create_curseforge_pack_game', {name: name.value.trim(), previewId: preview.value.preview_id, planId: preview.value.plan_id, source: {...source.value, mirror: source.value.mirror.trim()}, useDefaultDirectory: useDefaultDirectory.value}); if (!disposed) emit('created', game); }
  catch { if (!disposed) error.value = t('curseforge.packInstallFailed'); }
  finally { submitting.value = false; busy(false); }
}
let appliedInitialPreview = props.initialPreview;
watch([() => props.initialPreview, working], ([value, busy]) => {
  if (busy || value === appliedInitialPreview) return;
  appliedInitialPreview = value; preview.value = value ?? null; name.value = value?.name ?? ''; error.value = '';
});
onMounted(async () => { try { const value = await readDownloadSettings(); if (!disposed) source.value = value; } catch { if (!disposed) error.value = t('curseforge.settingsFailed'); } finally { loading.value = false; } });
onUnmounted(() => { disposed = true; });
</script>
<template>
  <div class="page-content cf-pack-install">
    <button
      v-if="!initialPreview"
      class="secondary-button"
      :disabled="working || loading"
      @click="choosePack"
    >
      {{ t('curseforge.chooseArchive') }}
    </button>
    <CurseForgePackPreparation
      v-if="preview && (!preview.ready || !preview.plan_id)"
      :preview="preview"
      @busy="busy"
      @selected="selected"
    />
    <form
      v-else-if="preview"
      class="create-form"
      @submit.prevent="create"
    >
      <div class="pack-title">
        <h2>{{ preview.name }}</h2><span class="count">{{ preview.version_id }}</span><span class="mod-state">CurseForge</span>
      </div>
      <div class="compatibility-tags">
        <span>Minecraft {{ preview.minecraft_version }}</span><span v-if="preview.loader_kind">{{ curseForgeLoaderLabel(preview.loader_kind) }} {{ preview.loader_version }}</span>
      </div>
      <p class="mod-meta">
        {{ t('curseforge.packFiles', {count: preview.files.length}) }}
      </p>
      <p
        v-if="preview.loader_kind === 'forge' || preview.loader_kind === 'neoforge'"
        class="setting-hint"
      >
        {{ t('curseforge.installerWarning', {loader: preview.loader_kind === 'forge' ? 'Forge' : 'NeoForge'}) }}
      </p>
      <div class="form-field">
        <label for="cf-game-name">{{ t('curseforge.gameName') }}</label><input
          id="cf-game-name"
          v-model="name"
          maxlength="80"
          autocomplete="off"
          :disabled="working"
        >
      </div>
      <div class="form-field">
        <label id="cf-source-label">{{ t('curseforge.minecraftSource') }}</label><RadioGroupRoot
          v-model="source.source"
          class="source-options"
          :disabled="working"
          aria-labelledby="cf-source-label"
        >
          <RadioGroupItem
            value="automatic"
            class="source-option"
          >
            {{ t('curseforge.automatic') }}
          </RadioGroupItem><RadioGroupItem
            value="official"
            class="source-option"
          >
            {{ t('curseforge.official') }}
          </RadioGroupItem><RadioGroupItem
            value="bmclapi"
            class="source-option"
          >
            BMCLAPI
          </RadioGroupItem><RadioGroupItem
            v-if="source.mirror"
            value="custom"
            class="source-option"
          >
            {{ t('curseforge.customMirror') }}
          </RadioGroupItem>
        </RadioGroupRoot>
      </div>
      <details class="cf-advanced">
        <summary>{{ t('curseforge.installLocation') }}</summary><button
          type="button"
          class="folder-button"
          :disabled="working"
          @click="chooseDirectory"
        >
          <span>{{ useDefaultDirectory ? t('curseforge.defaultDirectory') : directory }}</span><span class="folder-change">{{ t('curseforge.change') }}</span>
        </button>
      </details>
      <button
        class="done-button"
        type="submit"
        :disabled="loading || working"
      >
        {{ t(submitting ? 'curseforge.installing' : 'curseforge.createInstall') }}
      </button>
    </form>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ error }}
    </p>
  </div>
</template>
<style scoped>.cf-pack-install .create-form { margin-top: 20px; }.cf-pack-install .compatibility-tags { margin: 12px 0; }.cf-advanced { margin: 20px 0; }.cf-advanced summary { cursor: pointer; font-size: 12px; }.cf-advanced button { margin-top: 12px; }.cf-pack-install { overflow-wrap: anywhere; }</style>

