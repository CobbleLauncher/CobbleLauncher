<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import type { Game, PackPreview } from './games';
import { loaderLabel } from './games';

const emit = defineEmits<{ created: [game: Game] }>();
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

async function initialize() {
  try {
    source.value = await readDownloadSettings();
    directory.value = isTauri() ? await invoke<string>('default_game_directory') : '默认游戏文件夹';
  } catch (cause) { error.value = `无法读取游戏设置：${String(cause)}`; }
  finally { loading.value = false; }
}
async function choosePack() {
  if (picking.value || submitting.value || loading.value) return;
  if (!isTauri()) { error.value = '请在桌面应用中导入整合包'; return; }
  picking.value = true;
  error.value = '';
  try {
    const selected = await invoke<PackPreview | null>('preview_mrpack');
    if (disposed || !selected) return;
    preview.value = selected;
    name.value = selected.name;
    optionalFiles.value = [];
  } catch (cause) { if (!disposed) error.value = `无法读取整合包：${String(cause)}`; }
  finally { picking.value = false; }
}
async function chooseDirectory() {
  if (picking.value || submitting.value) return;
  picking.value = true;
  error.value = '';
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (!disposed && selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch (cause) { if (!disposed) error.value = `无法选择文件夹：${String(cause)}`; }
  finally { picking.value = false; }
}
async function create() {
  if (!preview.value || picking.value || submitting.value || loading.value) return;
  error.value = validateDownloadSettings(source.value) ?? '';
  if (error.value) return;
  if (!name.value.trim()) { error.value = '请填写游戏名称'; return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_mrpack_game', { previewId: preview.value.preview_id, name: name.value.trim(), source: { ...source.value, mirror: source.value.mirror.trim() }, useDefaultDirectory: useDefaultDirectory.value, optionalFiles: [...optionalFiles.value] });
    if (!disposed) emit('created', game);
  } catch (cause) { if (!disposed) error.value = `无法导入整合包：${String(cause)}`; }
  finally { submitting.value = false; }
}
function fileSize(bytes: number) { return bytes < 1024 * 1024 ? `${Math.ceil(bytes / 1024)} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`; }
onMounted(() => { void initialize(); });
onUnmounted(() => { disposed = true; });
</script>

<template>
  <div class="import-page page-content">
    <p class="create-description">
      把喜欢的整合包，变成一个新的世界。
    </p>
    <button
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
      <span><strong>{{ picking ? '正在读取…' : preview ? '更换整合包' : '选择整合包文件' }}</strong><span class="setting-hint">Modrinth 整合包 · .mrpack</span></span>
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
        aria-label="整合包信息"
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
          <span>Minecraft {{ preview.minecraft_version }}</span><span>{{ preview.loader_version ? `${loaderLabel(preview.loader_kind ?? 'fabric')} ${preview.loader_version}` : '原版' }}</span>
        </div>
        <p class="setting-hint">
          {{ preview.required_files }} 个必需文件 · {{ preview.overrides }} 个配置文件
        </p>
        <p
          v-if="preview.loader_kind === 'forge' || preview.loader_kind === 'neoforge'"
          class="setting-hint"
        >
          安装时会运行 {{ loaderLabel(preview.loader_kind) }} 官方安装器。
        </p>
      </section>
      <div
        v-if="preview.optional_files.length"
        class="pack-optionals"
      >
        <h3>可选内容 <span class="count">{{ preview.optional_files.length }}</span></h3>
        <p class="setting-hint">
          按需选择要一起安装的文件。
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
        <label for="pack-game-name">游戏名称</label>
        <input
          id="pack-game-name"
          v-model="name"
          maxlength="80"
          autocomplete="off"
          :disabled="picking || submitting"
        >
      </div>
      <div class="form-field">
        <label id="pack-source-label">下载源</label>
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
        <template v-if="source.source === 'custom'">
          <label
            class="sr-only"
            for="pack-mirror"
          >镜像地址</label>
          <input
            id="pack-mirror"
            v-model="source.mirror"
            type="url"
            placeholder="https://mirror.example.com/"
            :disabled="picking || submitting"
          >
        </template>
        <p class="setting-hint">
          Minecraft 文件使用所选下载源，整合包文件使用包内来源。
        </p>
      </div>
      <div class="form-field">
        <label>安装位置</label>
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
          <span>{{ directory || '默认游戏文件夹' }}</span><span class="folder-change">更改</span>
        </button>
        <p class="setting-hint">
          将在此文件夹内创建独立的游戏目录。
        </p>
      </div>
      <button
        class="done-button"
        type="submit"
        :disabled="loading || picking || submitting"
      >
        {{ submitting ? '正在创建…' : '导入并安装' }}
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
