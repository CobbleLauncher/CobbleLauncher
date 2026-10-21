<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { DialogRoot, DialogTrigger, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription, DialogClose, RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { applyTheme, readTheme, type Theme } from './theme';
import { defaultDownloadSettings, readDownloadSettings, saveDownloadSettings, validateDownloadSettings } from './download-settings';
const theme = ref<Theme>(readTheme());
const native = isTauri();
const maximized = ref(false);
const error = ref('');
const options = [{ value: 'system', label: '跟随系统' }, { value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }];
const settingsOpen = ref(false);
const downloadSettings = ref(defaultDownloadSettings());
const settingsError = ref('');
const settingsReady = ref(false);
const savingSettings = ref(false);
const sourceOptions = [{ value: 'official', label: '官方' }, { value: 'bmclapi', label: 'BMCLAPI' }, { value: 'custom', label: '自定义镜像' }];
async function openSettings(open: boolean) {
  settingsOpen.value = open;
  if (!open) return;
  settingsError.value = '';
  settingsReady.value = false;
  try { downloadSettings.value = await readDownloadSettings(); settingsReady.value = true; }
  catch { settingsError.value = '无法读取下载源设置，请关闭后重试'; }
}
function changeSource(value: unknown) {
  if (value === 'official' || value === 'bmclapi' || value === 'custom') {
    downloadSettings.value.source = value;
    settingsError.value = '';
  }
}
async function finishSettings() {
  if (!settingsReady.value || savingSettings.value) return;
  settingsError.value = validateDownloadSettings(downloadSettings.value) ?? '';
  if (settingsError.value) return;
  savingSettings.value = true;
  try { await saveDownloadSettings(downloadSettings.value); settingsOpen.value = false; }
  catch { settingsError.value = '下载源设置保存失败，请重试'; }
  finally { savingSettings.value = false; }
}
function changeTheme(value: unknown) {
  if (value === 'system' || value === 'light' || value === 'dark') { theme.value = value; applyTheme(value); }
}
async function syncWindow() {
  if (native) { try { maximized.value = await getCurrentWindow().isMaximized(); } catch { /* Actions surface errors. */ } }
}
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!native) return;
  try { error.value = ''; await getCurrentWindow()[action](); await syncWindow(); }
  catch { error.value = '窗口操作失败，请重试'; }
}
onMounted(() => { applyTheme(theme.value); void syncWindow(); window.addEventListener('resize', syncWindow); });
onUnmounted(() => window.removeEventListener('resize', syncWindow));
</script>
<template>
  <div class="launcher">
    <header class="titlebar">
      <div
        class="drag-area"
        data-tauri-drag-region
      >
        <span
          class="brand-mark"
          aria-hidden="true"
        ><i /><i /><i /><i /></span><span class="brand">cobble<span class="brand-dot">.</span></span>
      </div>
      <div
        v-if="native"
        class="window-controls"
      >
        <button
          aria-label="最小化"
          title="最小化"
          @click="windowAction('minimize')"
        >
          <svg viewBox="0 0 16 16"><path d="M3 8h10" /></svg>
        </button>
        <button
          :aria-label="maximized ? '还原' : '最大化'"
          :title="maximized ? '还原' : '最大化'"
          @click="windowAction('toggleMaximize')"
        >
          <svg viewBox="0 0 16 16"><path
            v-if="maximized"
            d="M5 5V3h8v8h-2M3 5h8v8H3z"
          /><path
            v-else
            d="M3 3h10v10H3z"
          /></svg>
        </button>
        <button
          class="window-close"
          aria-label="关闭窗口"
          title="关闭窗口"
          @click="windowAction('close')"
        >
          <svg viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg>
        </button>
      </div>
    </header>
    <main>
      <div class="library-header">
        <div>
          <p class="overline">
            MINECRAFT JAVA
          </p><h1>游戏库<span class="count">0</span></h1>
        </div>
        <DialogRoot
          :open="settingsOpen"
          @update:open="openSettings"
        >
          <DialogTrigger
            class="settings-button"
            aria-label="打开设置"
            title="设置"
          >
            <svg viewBox="0 0 24 24"><path d="M4 7h16M4 17h16" /><circle
              cx="9"
              cy="7"
              r="3"
            /><circle
              cx="15"
              cy="17"
              r="3"
            /></svg>
          </DialogTrigger>
          <DialogPortal>
            <DialogOverlay class="dialog-overlay" /><DialogContent class="settings-dialog">
              <div class="dialog-header">
                <DialogTitle>设置</DialogTitle><DialogClose
                  class="icon-button"
                  aria-label="关闭设置"
                >
                  ×
                </DialogClose>
              </div><DialogDescription class="setting-label">
                外观
              </DialogDescription><RadioGroupRoot
                class="theme-options"
                :model-value="theme"
                aria-label="外观"
                @update:model-value="changeTheme"
              >
                <RadioGroupItem
                  v-for="option in options"
                  :key="option.value"
                  :value="option.value"
                  class="theme-option"
                >
                  <span
                    class="theme-preview"
                    :class="option.value"
                  ><i /><i /><i /></span>{{ option.label }}
                </RadioGroupItem>
              </RadioGroupRoot>
              <section
                class="download-section"
                aria-labelledby="download-label"
              >
                <h3
                  id="download-label"
                  class="setting-label"
                >
                  下载源
                </h3>
                <RadioGroupRoot
                  class="source-options"
                  :model-value="downloadSettings.source"
                  :disabled="!settingsReady || savingSettings"
                  aria-label="下载源"
                  @update:model-value="changeSource"
                >
                  <RadioGroupItem
                    v-for="option in sourceOptions"
                    :key="option.value"
                    :value="option.value"
                    class="source-option"
                  >
                    {{ option.label }}
                  </RadioGroupItem>
                </RadioGroupRoot>
                <div
                  v-if="downloadSettings.source === 'custom'"
                  class="mirror-field"
                >
                  <label for="mirror-url">镜像地址</label>
                  <input
                    id="mirror-url"
                    v-model="downloadSettings.mirror"
                    type="url"
                    placeholder="https://mirror.example.com/"
                    spellcheck="false"
                    autocomplete="off"
                    :disabled="savingSettings"
                    :aria-invalid="!!settingsError"
                    aria-describedby="mirror-help"
                    @input="settingsError = ''"
                  >
                  <p
                    id="mirror-help"
                    class="setting-hint"
                  >
                    填写 BMCLAPI 兼容镜像地址。
                  </p>
                </div>
                <p
                  v-if="downloadSettings.source === 'bmclapi'"
                  class="setting-hint"
                >
                  BMCLAPI 由 bangbang93 及贡献者提供，镜像文件版权归原作者所有。
                </p>
                <p
                  v-if="settingsError"
                  class="settings-error"
                  role="alert"
                >
                  {{ settingsError }}
                </p>
              </section>
              <div class="about">
                <span>Cobble Launcher</span><span>0.1.0</span>
              </div><button
                class="done-button"
                :disabled="!settingsReady || savingSettings"
                @click="finishSettings"
              >
                {{ savingSettings ? '保存中…' : '完成' }}
              </button>
            </DialogContent>
          </DialogPortal>
        </DialogRoot>
      </div>
      <section
        class="empty-library"
        aria-labelledby="empty-title"
      >
        <div
          class="island-scene"
          aria-hidden="true"
        >
          <svg
            viewBox="0 0 360 250"
            fill="none"
          ><ellipse
             class="island-shadow"
             cx="180"
             cy="214"
             rx="102"
             ry="16"
           /><path
             class="orbit"
             d="M32 153 180 70l148 83-148 82z"
           />
            <path
              class="stone-left"
              d="m75 143 105 60v-63L75 80z"
            /><path
              class="stone-right"
              d="m180 203 105-60V80l-105 60z"
            /><path
              class="grass-top"
              d="m75 80 105-60 105 60-105 60z"
            /><path
              class="grass-left"
              d="m75 80 105 60v18L75 98z"
            /><path
              class="grass-right"
              d="m180 140 105-60v18l-105 60z"
            />
            <path
              class="stone-detail"
              d="m90 125 24 14v17l-24-14zm42 37 28 16v16l-28-16zm75-3 24-14v17l-24 14zm40-39 23-13v17l-23 13z"
            />
            <path
              class="trunk-left"
              d="m173 77 9 5v34l-9-5z"
            /><path
              class="trunk-right"
              d="m182 82 9-5v34l-9 5z"
            />
            <path
              class="leaf-left"
              d="m142 38 40 23v42l-40-23z"
            /><path
              class="leaf-right"
              d="m182 61 40-23v42l-40 23z"
            /><path
              class="leaf-top"
              d="m142 38 40-23 40 23-40 23z"
            />
            <path
              class="flower"
              d="m116 87 7-4 7 4-7 4zm119 8 7-4 7 4-7 4z"
            /><path
              class="grass-stroke"
              d="M123 91v9m119-1v9"
            />
            <path
              class="spark"
              d="M70 40v12m-6-6h12M284 42v8m-4-4h8"
            />
          </svg>
        </div>
        <h2 id="empty-title">
          还没有游戏
        </h2><p class="empty-description">
          创建一个新游戏，或导入你喜欢的整合包。
        </p>
        <div class="start-actions">
          <button
            class="primary-button"
            disabled
            title="暂不可用"
            aria-describedby="feature-availability"
          >
            <span aria-hidden="true">＋</span>创建游戏
          </button><button
            class="secondary-button"
            disabled
            title="暂不可用"
            aria-describedby="feature-availability"
          >
            <svg viewBox="0 0 20 20"><path d="M10 3v9m-4-4 4 4 4-4M4 13v4h12v-4" /></svg>导入整合包
          </button>
        </div><span
          id="feature-availability"
          class="sr-only"
        >创建和导入暂不可用</span>
      </section>
      <p
        v-if="error"
        role="alert"
        class="window-error"
      >
        {{ error }}
      </p>
    </main>
  </div>
</template>
