<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { applyTheme, readTheme, type Theme } from './theme';
import MemoryField from './MemoryField.vue';
import type { JavaSelection, LauncherPreferences } from './games';
import { defaultDownloadSettings, readDownloadSettings, saveDownloadSettings, validateDownloadSettings } from './download-settings';
const theme = ref<Theme>(readTheme());
const options = [{ value: 'system', label: '跟随系统' }, { value: 'light', label: '浅色' }, { value: 'dark', label: '深色' }];
const emit = defineEmits<{ saved: [] }>();
const categories = ['外观', '下载与目录', '默认运行设置', '关于'] as const;
const activeCategory = ref<(typeof categories)[number]>('外观');
function selectCategory(category: (typeof categories)[number]) { activeCategory.value = category; }
function moveCategory(event: KeyboardEvent) {
  const index = categories.indexOf(activeCategory.value);
  let next: number;
  if (event.key === 'ArrowRight') next = (index + 1) % categories.length;
  else if (event.key === 'ArrowLeft') next = (index - 1 + categories.length) % categories.length;
  else if (event.key === 'Home') next = 0;
  else if (event.key === 'End') next = categories.length - 1;
  else return;
  event.preventDefault();
  activeCategory.value = categories[next] ?? categories[0];
  document.getElementById(`settings-tab-${next}`)?.focus();
}
const downloadSettings = ref(defaultDownloadSettings());
const loadedDownloadSettings = ref<ReturnType<typeof defaultDownloadSettings> | null>(null);
const hasDownloadChanges = computed(() => loadedDownloadSettings.value !== null && (
  downloadSettings.value.source !== loadedDownloadSettings.value.source ||
  downloadSettings.value.mirror !== loadedDownloadSettings.value.mirror
));
const settingsError = ref('');
const settingsReady = ref(false);
const savingSettings = ref(false);
const sourceOptions = [{ value: 'automatic', label: '自动（推荐）' }, { value: 'official', label: '官方' }, { value: 'bmclapi', label: 'BMCLAPI' }, { value: 'custom', label: '自定义镜像' }];
async function loadSettings() {
  settingsError.value = '';
  settingsReady.value = false;
  try {
    downloadSettings.value = await readDownloadSettings();
    loadedDownloadSettings.value = { ...downloadSettings.value };
    settingsReady.value = true;
  }
  catch { settingsError.value = '无法读取下载源设置，请返回后重试'; }
}
function changeSource(value: unknown) {
  if (value === 'automatic' || value === 'official' || value === 'bmclapi' || value === 'custom') {
    downloadSettings.value.source = value;
    settingsError.value = '';
  }
}
async function finishSettings() {
  if (!settingsReady.value || savingSettings.value) return;
  settingsError.value = validateDownloadSettings(downloadSettings.value) ?? '';
  if (settingsError.value) return;
  savingSettings.value = true;
  try {
    await saveDownloadSettings(downloadSettings.value);
    loadedDownloadSettings.value = { ...downloadSettings.value };
    emit('saved');
  }
  catch { settingsError.value = '下载源设置保存失败，请重试'; }
  finally { savingSettings.value = false; }
}
function changeTheme(value: unknown) {
  if (value === 'system' || value === 'light' || value === 'dark') { theme.value = value; applyTheme(value); }
}

const native = isTauri();
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
const preferences = ref<LauncherPreferences | null>(null);
const preferencesBusy = ref(false);
const preferencesError = ref('');
const memoryDraft = ref(4096);
const memoryBusy = ref(false);
const memoryError = ref('');
const directoryBusy = ref(false);
const directoryError = ref('');
async function loadPreferences() {
  if (!native || preferencesBusy.value) return;
  preferencesBusy.value = true; preferencesError.value = '';
  try { preferences.value = await invoke<LauncherPreferences>('get_launcher_preferences'); memoryDraft.value = preferences.value.memory_mb; }
  catch { preferencesError.value = '无法读取默认设置，请重试'; }
  finally { preferencesBusy.value = false; }
}
async function saveMemory() {
  if (!native || !preferences.value || memoryBusy.value) return;
  memoryError.value = '';
  if (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144) { memoryError.value = '内存须为 512–262144 MiB 的整数'; return; }
  memoryBusy.value = true;
  try { preferences.value = await invoke<LauncherPreferences>('set_default_memory', { memoryMb: memoryDraft.value }); }
  catch { memoryError.value = '未能保存默认内存，请重试'; }
  finally { memoryBusy.value = false; }
}
async function changeDirectory(reset = false) {
  if (!native || directoryBusy.value || !preferences.value) return;
  directoryBusy.value = true; directoryError.value = '';
  try { const value = await invoke<LauncherPreferences | null>(reset ? 'reset_default_game_directory' : 'pick_default_game_directory'); if (value) preferences.value = value; }
  catch { directoryError.value = '未能更改默认目录，请重试'; }
  finally { directoryBusy.value = false; }
}
watch(activeCategory, (category) => {
  if (category === '默认运行设置' && !javaReady.value) void loadJava();
  if ((category === '默认运行设置' || category === '下载与目录') && !preferences.value) void loadPreferences();
});
onMounted(() => { void loadSettings(); });
</script>
<template>
  <div class="settings-page page-content">
    <div
      class="settings-tabs"
      role="tablist"
      aria-label="设置分类"
      @keydown="moveCategory"
    >
      <button
        v-for="(category, index) in categories"
        :id="`settings-tab-${index}`"
        :key="category"
        class="settings-tab"
        type="button"
        role="tab"
        :aria-selected="activeCategory === category"
        :aria-controls="`settings-panel-${index}`"
        :tabindex="activeCategory === category ? 0 : -1"
        @click="selectCategory(category)"
      >
        {{ category }}
      </button>
    </div>
    <section
      v-show="activeCategory === '外观'"
      id="settings-panel-0"
      class="settings-panel"
      role="tabpanel"
      aria-labelledby="settings-tab-0"
      tabindex="0"
    >
      <RadioGroupRoot
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
    </section>
    <section
      v-show="activeCategory === '下载与目录'"
      id="settings-panel-1"
      class="settings-panel settings-section"
      role="tabpanel"
      aria-labelledby="settings-tab-1"
      tabindex="0"
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
      <p
        v-if="downloadSettings.source === 'automatic'"
        class="setting-hint"
      >
        自动选择响应较快的下载源，失败时切换。
      </p>
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
      <div class="runtime-option-section directory-default">
        <h2 class="setting-label">
          默认游戏目录
        </h2>
        <p
          v-if="preferences"
          class="java-path default-game-path"
        >
          {{ preferences.game_directory }}
        </p>
        <div class="java-actions">
          <button
            class="secondary-button"
            :disabled="!preferences || directoryBusy || !native"
            @click="changeDirectory()"
          >
            {{ directoryBusy ? '请稍候…' : '更改目录' }}
          </button><button
            v-if="preferences?.custom_game_directory"
            class="text-button"
            :disabled="directoryBusy"
            @click="changeDirectory(true)"
          >
            恢复默认目录
          </button>
        </div>
        <p class="setting-hint">
          只影响以后创建的游戏，已有游戏不会搬迁。
        </p>
        <p
          v-if="directoryError"
          class="settings-error"
          role="alert"
        >
          {{ directoryError }}
        </p>
        <p
          v-if="preferencesError"
          class="settings-error"
          role="alert"
        >
          {{ preferencesError }} <button
            class="text-button"
            @click="loadPreferences"
          >
            重试
          </button>
        </p>
      </div>
    </section>

    <section
      v-show="activeCategory === '默认运行设置'"
      id="settings-panel-2"
      class="settings-panel settings-section"
      role="tabpanel"
      aria-labelledby="settings-tab-2"
      tabindex="0"
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
      <section
        class="runtime-option-section"
        aria-labelledby="default-memory-heading"
      >
        <h2
          id="default-memory-heading"
          class="setting-label"
        >
          最大内存
        </h2>
        <form
          novalidate
          @submit.prevent="saveMemory"
        >
          <MemoryField
            id="default-memory"
            v-model="memoryDraft"
            label="默认最大内存 (MiB)"
            :disabled="!preferences || memoryBusy || !native"
          /><p class="setting-hint">
            用于未单独设置内存的游戏。
          </p><button
            class="secondary-button runtime-save-button"
            type="submit"
            :disabled="!preferences || memoryBusy || memoryDraft === preferences.memory_mb || !native"
          >
            {{ memoryBusy ? '保存中…' : '保存默认内存' }}
          </button>
        </form>
        <p
          v-if="memoryError"
          class="settings-error"
          role="alert"
        >
          {{ memoryError }}
        </p>
        <p
          v-if="preferencesError"
          class="settings-error"
          role="alert"
        >
          {{ preferencesError }} <button
            class="text-button"
            @click="loadPreferences"
          >
            重试
          </button>
        </p>
      </section>
    </section>
    <section
      v-show="activeCategory === '关于'"
      id="settings-panel-3"
      class="settings-panel about"
      role="tabpanel"
      aria-labelledby="settings-tab-3"
      tabindex="0"
    >
      <h2 class="setting-label">
        关于
      </h2>
      <p>Cobble Launcher <span>0.1.0</span></p>
      <a
        href="https://github.com/CobbleLauncher/CobbleLauncher"
        target="_blank"
        rel="noreferrer"
      >项目主页</a>
      <p class="setting-hint">
        MIT 许可证
      </p>
    </section>
    <button
      v-if="activeCategory === '下载与目录'"
      class="done-button"
      :disabled="!settingsReady || savingSettings || !hasDownloadChanges"
      @click="finishSettings"
    >
      {{ savingSettings ? '保存中…' : '保存并返回' }}
    </button>
  </div>
</template>

<style scoped>
.settings-tabs{display:flex;gap:6px;overflow-x:auto;overscroll-behavior-inline:contain;border-bottom:1px solid var(--line);margin:0 0 22px;scrollbar-width:thin}
.settings-tab{flex:0 0 auto;padding:10px 13px;margin-bottom:-1px;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--muted);font:inherit;font-size:12px;white-space:nowrap}
.settings-tab[aria-selected=true]{border-bottom-color:var(--accent);color:var(--ink);font-weight:600}
.settings-tab:focus-visible{outline:2px solid var(--accent);outline-offset:-2px;border-radius:5px}
.settings-panel{min-width:0}
.about{display:flex;flex-direction:column;gap:10px;font-size:13px}
.about p{margin:0}
.about span,.about .setting-hint{color:var(--muted)}
.about a{color:var(--ink);text-underline-offset:3px}
</style>
