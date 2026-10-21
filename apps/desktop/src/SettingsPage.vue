<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { applyTheme, readTheme, type Theme } from './theme';
import { defaultDownloadSettings, readDownloadSettings, saveDownloadSettings, validateDownloadSettings } from './download-settings';
const theme = ref<Theme>(readTheme());
const options = [{ value: 'system', label: '跟随系统' }, { value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }];
const emit = defineEmits<{ saved: [] }>();
const downloadSettings = ref(defaultDownloadSettings());
const settingsError = ref('');
const settingsReady = ref(false);
const savingSettings = ref(false);
const sourceOptions = [{ value: 'official', label: '官方' }, { value: 'bmclapi', label: 'BMCLAPI' }, { value: 'custom', label: '自定义镜像' }];
async function loadSettings() {
  settingsError.value = '';
  settingsReady.value = false;
  try { downloadSettings.value = await readDownloadSettings(); settingsReady.value = true; }
  catch { settingsError.value = '无法读取下载源设置，请返回后重试'; }
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
  try { await saveDownloadSettings(downloadSettings.value); emit('saved'); }
  catch { settingsError.value = '下载源设置保存失败，请重试'; }
  finally { savingSettings.value = false; }
}
function changeTheme(value: unknown) {
  if (value === 'system' || value === 'light' || value === 'dark') { theme.value = value; applyTheme(value); }
}

const native = isTauri();
type JavaRuntimeInfo = { version: string; major: number; arch: string; vendor: string; executable: string };
type JavaSelection = { mode: 'automatic' | 'custom'; executable: string | null; runtime: JavaRuntimeInfo | null };
const javaSelection = ref<JavaSelection>({ mode: 'automatic', executable: null, runtime: null });
const javaReady = ref(false);
const javaBusy = ref(false);
const javaError = ref('');
async function loadJava() {
  if (!native) return;
  try { javaSelection.value = await invoke<JavaSelection>('get_java_settings'); javaReady.value = true; }
  catch { javaError.value = '无法读取已保存的 Java，请重新选择或恢复自动选择'; }
  finally { javaReady.value = true; }
}
async function selectJava() {
  javaBusy.value = true;
  javaError.value = '';
  try {
    const selected = await invoke<JavaSelection | null>('pick_java_executable');
    if (selected) javaSelection.value = selected;
  } catch (cause) { javaError.value = `无法使用所选 Java，请选择有效的 Java 可执行文件：${String(cause)}`; }
  finally { javaBusy.value = false; }
}
async function automaticJava() {
  javaBusy.value = true;
  javaError.value = '';
  try { javaSelection.value = await invoke<JavaSelection>('use_automatic_java'); }
  catch { javaError.value = '无法切换为自动选择，请重试'; }
  finally { javaBusy.value = false; }
}
onMounted(() => { void loadSettings(); void loadJava(); });
</script>
<template>
  <div class="settings-page page-content">
    <h2 class="setting-label">
      外观
    </h2><RadioGroupRoot
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
      class="settings-section"
      aria-labelledby="download-label"
    >
      <h2
        id="download-label"
        class="setting-label"
      >
        下载源
      </h2>
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

    <section
      class="settings-section"
      aria-labelledby="java-heading"
    >
      <h2
        id="java-heading"
        class="setting-label"
      >
        Java 运行环境
      </h2>
      <div class="java-selection">
        <div
          class="java-icon"
          aria-hidden="true"
        >
          J
        </div>
        <div class="java-details">
          <h3>{{ javaSelection.mode === 'custom' ? (javaSelection.runtime ? `Java ${javaSelection.runtime.major}` : '已选择 Java') : '自动选择' }}</h3>
          <p class="setting-hint">
            {{ javaSelection.mode === 'custom' ? [javaSelection.runtime?.vendor, javaSelection.runtime?.version, javaSelection.runtime?.arch].filter(Boolean).join(' · ') : '按游戏版本选择' }}
          </p>
          <p
            v-if="javaSelection.executable"
            class="java-path"
          >
            {{ javaSelection.executable }}
          </p>
        </div>
      </div>
      <div class="java-actions">
        <button
          class="secondary-button"
          :disabled="javaBusy || !javaReady || !native"
          @click="selectJava"
        >
          {{ javaBusy ? '请稍候…' : '选择 Java' }}
        </button>
        <button
          v-if="javaSelection.mode === 'custom' || javaError"
          class="text-button"
          :disabled="javaBusy"
          @click="automaticJava"
        >
          自动选择
        </button>
      </div>
      <p
        v-if="!native"
        class="setting-hint"
      >
        在桌面应用中可选择本机 Java。
      </p>
      <p
        v-if="javaError"
        class="settings-error"
        role="alert"
      >
        {{ javaError }}
      </p>
    </section>
    <div class="about">
      <span>Cobble Launcher</span><span>0.1.0</span>
    </div><button
      class="done-button"
      :disabled="!settingsReady || savingSettings"
      @click="finishSettings"
    >
      {{ savingSettings ? '保存中…' : '保存并返回' }}
    </button>
  </div>
</template>
