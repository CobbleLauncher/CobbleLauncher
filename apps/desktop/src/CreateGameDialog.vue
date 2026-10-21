<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription, DialogClose, RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import type { Game, GameVersion } from './games';

const emit = defineEmits<{ created: [game: Game] }>();
const open = ref(false);
const name = ref('我的世界');
const version = ref('');
const versions = ref<GameVersion[]>([]);
const source = ref(defaultDownloadSettings());
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
  open.value = true;
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
  if (value !== 'official' && value !== 'bmclapi' && value !== 'custom') return;
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
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_game', { name: name.value.trim(), version: version.value, source: { ...source.value }, useDefaultDirectory: useDefaultDirectory.value });
    emit('created', game);
    open.value = false;
  } catch (cause) { error.value = `无法创建游戏：${String(cause)}`; }
  finally { submitting.value = false; }
}

defineExpose({ show });
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="dialog-overlay" />
      <DialogContent class="settings-dialog create-dialog">
        <div class="dialog-header">
          <DialogTitle>创建游戏</DialogTitle>
          <DialogClose
            class="icon-button"
            aria-label="关闭创建游戏"
          >
            ×
          </DialogClose>
        </div>
        <DialogDescription class="create-description">
          从一个全新的原版世界开始。
        </DialogDescription>
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
            :disabled="loading || submitting"
          >
            {{ submitting ? '正在创建…' : '创建并安装' }}
          </button>
        </form>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
