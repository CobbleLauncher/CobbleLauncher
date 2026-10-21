<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type FabricVersion, type Game, type GameVersion, type LoaderVersion } from './games';

const emit = defineEmits<{ created: [game: Game] }>();
const name = ref('我的世界');
const version = ref('');
const loader = ref('vanilla');
const loaderVersion = ref('');
const loaderVersions = ref<LoaderVersion[]>([]);
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
  if (sourceError) { loaderError.value = sourceError; return; }
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
    if (!result.length) loaderError.value = `此 Minecraft 版本暂无可用的 ${label}`;
  } catch (cause) {
    if (request === loaderRequest) loaderError.value = `无法获取 ${label} 版本：${String(cause)}`;
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
  error.value = validateDownloadSettings(source.value) ?? '';
  if (error.value || !isTauri()) return;
  loading.value = true;
  try {
    const result = await invoke<GameVersion[]>('list_game_versions', { source: { ...source.value } });
    if (request !== versionRequest) return;
    versions.value = result;
    version.value = result[0]?.id ?? '';
    if (!result.length) error.value = '没有可安装的版本，请切换下载源重试';
  } catch (cause) {
    if (request === versionRequest) error.value = `无法获取版本：${String(cause)}`;
  } finally {
    if (request === versionRequest) loading.value = false;
  }
}

async function show() {
  error.value = '';
  useDefaultDirectory.value = true;
  try {
    source.value = await readDownloadSettings();
    directory.value = isTauri() ? await invoke<string>('default_game_directory') : '默认游戏文件夹';
    await loadVersions();
    if (!isTauri()) error.value = '请在桌面应用中安装游戏';
  } catch { error.value = '无法读取游戏设置，请重试'; }
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
  if (!isTauri()) { error.value = '请在桌面应用中安装游戏'; return; }
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch { error.value = '无法选择文件夹，请重试'; }
}

async function create() {
  if (!isTauri()) { error.value = '请在桌面应用中安装游戏'; return; }
  if (submitting.value) return;
  error.value = validateDownloadSettings(source.value) ?? '';
  if (error.value) return;
  if (!name.value.trim() || !version.value) { error.value = '请填写游戏名称并选择版本'; return; }
  if (loader.value !== 'vanilla' && (loadingLoader.value || !loaderVersion.value)) { error.value = "请选择可用的" + selectedLoaderLabel.value + "版本"; return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_game', { name: name.value.trim(), version: version.value, loaderKind: loader.value === 'vanilla' ? null : loader.value, loaderVersion: loader.value !== 'vanilla' ? loaderVersion.value : null, source: { ...source.value }, useDefaultDirectory: useDefaultDirectory.value });
    emit('created', game);
  } catch (cause) { error.value = `无法创建游戏：${String(cause)}`; }
  finally { submitting.value = false; }
}

onMounted(() => { void show(); });
onUnmounted(() => { ++versionRequest; ++loaderRequest; });
</script>

<template>
  <div class="create-page page-content">
    <p class="create-description">
      选择喜欢的版本，开始一个新世界。
    </p>
    <form
      class="create-form"
      @submit.prevent="create"
    >
      <div class="form-field">
        <label for="game-name">游戏名称</label>
        <input
          id="game-name"
          v-model="name"
          maxlength="80"
          autocomplete="off"
          :disabled="submitting"
        >
      </div>
      <div class="form-field">
        <label id="create-source-label">下载源</label>
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
            自动（推荐）
          </RadioGroupItem>
          <RadioGroupItem
            value="official"
            class="source-option"
          >
            官方
          </RadioGroupItem>
          <RadioGroupItem
            value="bmclapi"
            class="source-option"
          >
            BMCLAPI
          </RadioGroupItem>
          <RadioGroupItem
            value="custom"
            class="source-option"
          >
            自定义镜像
          </RadioGroupItem>
        </RadioGroupRoot>
        <p
          v-if="source.source === 'automatic'"
          class="setting-hint"
        >
          自动选择响应较快的下载源，失败时切换。
        </p>
        <div
          v-if="source.source === 'custom'"
          class="custom-version-source"
        >
          <label
            class="sr-only"
            for="create-mirror"
          >镜像地址</label>
          <input
            id="create-mirror"
            v-model="source.mirror"
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
            刷新版本
          </button>
        </div>
      </div>
      <div class="form-field">
        <label for="game-version">Minecraft 版本 <span class="field-note">正式版</span></label>
        <div class="version-control">
          <select
            id="game-version"
            v-model="version"
            :disabled="loading || submitting || !versions.length"
          >
            <option
              v-if="!versions.length"
              value=""
            >
              {{ loading ? '正在获取版本…' : '暂无可选版本' }}
            </option>
            <option
              v-for="item in versions"
              :key="item.id"
              :value="item.id"
            >
              {{ item.id }}
            </option>
          </select>
          <button
            v-if="!loading && !versions.length && source.source !== 'custom'"
            type="button"
            class="text-button"
            @click="loadVersions"
          >
            重试
          </button>
        </div>
      </div>
      <div class="form-field">
        <label id="loader-label">游戏类型</label>
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
            原版
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
        <p class="setting-hint">
          {{ loader === 'vanilla' ? 'Minecraft 原版体验。' : loader === 'fabric' ? '轻量模组加载器，安装后可添加本地 Mod。' : `${selectedLoaderLabel} 模组加载器，目前支持 Minecraft 1.21.1。安装时会运行官方安装器。` }}
        </p>
      </div>
      <div
        v-if="loader !== 'vanilla'"
        class="form-field"
      >
        <label for="loader-version">{{ selectedLoaderLabel }} 版本</label>
        <div class="version-control">
          <select
            id="loader-version"
            v-model="loaderVersion"
            :disabled="loadingLoader || submitting || !loaderVersions.length"
          >
            <option
              v-if="!loaderVersions.length"
              value=""
            >
              {{ loadingLoader ? '正在获取版本…' : '暂无可选版本' }}
            </option>
            <option
              v-for="item in loaderVersions"
              :key="item.version"
              :value="item.version"
            >
              {{ item.version }}{{ item.recommended ? (loader === 'fabric' ? ' · 稳定版' : ' · 推荐') : '' }}
            </option>
          </select>
          <button
            v-if="loaderError"
            type="button"
            class="text-button"
            :disabled="loadingLoader || submitting"
            @click="loadLoaderVersions"
          >
            重试
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
        <label>安装位置</label>
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
          <span>{{ directory || '默认游戏文件夹' }}</span><span class="folder-change">更改</span>
        </button>
        <p class="setting-hint">
          将在此文件夹内创建独立的游戏目录。
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
        {{ submitting ? '正在创建…' : '创建并安装' }}
      </button>
    </form>
  </div>
</template>
