<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import UpdateSettings from './UpdateSettings.vue';
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { computed, nextTick, onMounted, ref, watch, onUnmounted } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupRoot, RadioGroupItem, TabsRoot, TabsContent, DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle } from 'reka-ui';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsSaveBar from './settings/SettingsSaveBar.vue';
import GameDirectoryMigration from './settings/GameDirectoryMigration.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import type { HomeGame } from './library-home';
import AppIconButton from './ui/AppIconButton.vue';
import SettingsNavigation from './settings/SettingsNavigation.vue';
import SelectField from './ui/SelectField.vue';
import SwitchField from './ui/SwitchField.vue';
import { settingsCategories, type SettingsCategory } from './settings/categories';
import AppearanceSettings from './AppearanceSettings.vue';
import HomeLayoutSettings from './HomeLayoutSettings.vue';
import ShortcutSettings from './commands/ShortcutSettings.vue';
import type { CommandAction } from './commands/registry';
import MemoryField from './MemoryField.vue';
import AdvancedRuntimeSettings from './AdvancedRuntimeSettings.vue';
import { t, locale, localePreference, supportedLocales, setLocale, type LocalePreference } from './i18n';
import type { JavaSelection, LauncherPreferences } from './games';
import { defaultDownloadSettings, readDownloadSettings, saveDownloadSettings, validateDownloadSettings } from './download-settings';
const emit = defineEmits<{ saved: []; profiles: []; firstRun: []; reviewTerms: []; migrateGames: [] }>();
const props = defineProps<{ initialCategory?: string; extraCategories?: SettingsCategory[]; commands?: CommandAction[]; games?: HomeGame[]; beforeCategoryLeave?: () => Promise<boolean> }>();
const categories = computed(() => {
  const result = settingsCategories(props.extraCategories);
  result.splice(1, 0,
    { id: 'home-layout', label: t('settingsLayout.homeLayout.title') },
    { id: 'shortcuts', label: locale.value === 'zh-CN' ? '快捷键' : 'Shortcuts' });
  return result;
});
const activeCategory = ref(categories.value.some(item => item.id === props.initialCategory) ? props.initialCategory! : 'general');
const categoryTitle = computed(() => { const category = categories.value.find(item => item.id === activeCategory.value); return category?.labelKey ? t(category.labelKey) : category?.label ?? ''; });
const languageSaved = ref(localePreference.value);
const languageDraft = ref<LocalePreference>(languageSaved.value);
const languageDirty = computed(() => languageDraft.value !== languageSaved.value);
function stageLanguage(value: string) { if (value === 'system' || value === 'en-US' || value === 'zh-CN') languageDraft.value = value; }
const sectionNames = computed(() => locale.value === 'zh-CN' ? { downloads: '下载', storage: '存储', runtime: 'Java 与内存', language: '语言', diagnostics: '诊断' } : { downloads: 'Downloads', storage: 'Storage', runtime: 'Java and memory', language: 'Language', diagnostics: 'Diagnostics' });
const downloadSettings = ref(defaultDownloadSettings());
const loadedDownloadSettings = ref<ReturnType<typeof defaultDownloadSettings> | null>(null);
const hasDownloadChanges = computed(() => loadedDownloadSettings.value !== null && (
  downloadSettings.value.source !== loadedDownloadSettings.value.source ||
  downloadSettings.value.mirror !== loadedDownloadSettings.value.mirror
));
const settingsError = ref('');
const settingsReady = ref(false);
const savingSettings = ref(false);
const diagnosticBusy = ref(false); const diagnosticError = ref('');
async function diagnostics(exportZip = false) {
  diagnosticBusy.value = true; diagnosticError.value = '';
  try { await invoke(exportZip ? 'export_launcher_diagnostics' : 'open_launcher_logs'); }
  catch { diagnosticError.value = t('settings.diagnosticError'); }
  finally { diagnosticBusy.value = false; }
}
const cacheBytes = ref<number | null>(null);
const cacheBusy = ref(false);
const cacheError = ref('');
const tuning = ref({ adaptive: true, max_concurrent: 12, per_origin: 4 });
const tuningSaved = ref({ ...tuning.value });
const tuningDirty = computed(() => JSON.stringify(tuning.value) !== JSON.stringify(tuningSaved.value));
const tuningReady = ref(false); const tuningBusy = ref(false); const tuningError = ref('');
async function loadTuning() {
  if (!isTauri() || tuningReady.value) return;
  try { tuning.value = { adaptive: true, ...await invoke<Omit<typeof tuning.value, 'adaptive'> & { adaptive?: boolean }>('get_download_tuning') }; tuningSaved.value = { ...tuning.value }; tuningReady.value = true; }
  catch { tuningError.value = t('settingsLayout.loadFailed'); }
}
async function saveTuning() {
  tuningBusy.value = true; tuningError.value = '';
  try { await invoke('set_download_tuning', { tuning: tuning.value }); tuningSaved.value = { ...tuning.value }; }
  catch { tuningError.value = t('settingsLayout.tuningInvalid'); }
  finally { tuningBusy.value = false; }
}
async function updateCache(clear = false) {
  if (!isTauri() || cacheBusy.value) return;
  cacheBusy.value = true; cacheError.value = '';
  try { const result = await invoke<{ disk_bytes: number }>(clear ? 'clear_public_cache' : 'get_public_cache_stats'); cacheBytes.value = result.disk_bytes; }
  catch { cacheError.value = t('settingsLayout.cacheFailed'); }
  finally { cacheBusy.value = false; }
}
const editingMirror = ref(false);
const mirrorDraft = ref('');
const savedMirror = computed(() => validateDownloadSettings({ source: 'custom', mirror: downloadSettings.value.mirror }) === null ? downloadSettings.value.mirror : '');
const sourceOptions = computed(() => [
  { value: 'automatic', label: t('settingsLayout.recommended') }, { value: 'official', label: t('settingsLayout.official') }, { value: 'bmclapi', label: 'BMCLAPI' },
  ...(savedMirror.value ? [{ value: 'custom', label: new URL(savedMirror.value).hostname }] : []),
]);
function editMirror() { mirrorDraft.value = downloadSettings.value.mirror; editingMirror.value = true; settingsError.value = ''; }
async function addMirror() {
  const candidate = { source: 'custom' as const, mirror: mirrorDraft.value.trim() };
  settingsError.value = validateDownloadSettings(candidate) ? t('settingsLayout.customMirrorInvalid') : '';
  if (settingsError.value) return;
  downloadSettings.value.mirror = candidate.mirror; downloadSettings.value.source = 'custom';
  editingMirror.value = false;

}
async function removeMirror() {
  downloadSettings.value.mirror = '';
  if (downloadSettings.value.source === 'custom') downloadSettings.value.source = 'automatic';
  editingMirror.value = false;

}
async function loadSettings() {
  settingsError.value = '';
  settingsReady.value = false;
  try {
    downloadSettings.value = await readDownloadSettings();
    loadedDownloadSettings.value = { ...downloadSettings.value };
    settingsReady.value = true;
  }
  catch { settingsError.value = t('settingsLayout.loadFailed'); }
}
function changeSource(value: unknown) {
  if (value === 'automatic' || value === 'official' || value === 'bmclapi' || value === 'custom') {
    downloadSettings.value.source = value;
    settingsError.value = '';
  }
}
async function finishSettings() {
  if (!settingsReady.value || savingSettings.value) return;
  settingsError.value = validateDownloadSettings(downloadSettings.value) ? t('settingsLayout.customMirrorInvalid') : '';
  if (settingsError.value) return;
  savingSettings.value = true;
  try {
    await saveDownloadSettings(downloadSettings.value);
    loadedDownloadSettings.value = { ...downloadSettings.value };
    emit('saved');
  }
  catch { settingsError.value = t('settingsLayout.saveFailed'); }
  finally { savingSettings.value = false; }
}

const native = isTauri();
const javaSelection = ref<JavaSelection>({ mode: 'automatic', executable: null, runtime: null });
const javaSaved = ref<JavaSelection | null>(null);
const javaRuntimeId = ref<string | null>(null);
const javaDirty = computed(() => javaSaved.value !== null && (javaSelection.value.mode !== javaSaved.value.mode || javaSelection.value.executable !== javaSaved.value.executable));
const javaReady = ref(false);
const javaBusy = ref(false);
const javaError = ref('');
type DetectedJava = { id: string; major: number; vendor: string; version: string; arch: string; executable: string; recommended: boolean };
const javaRuntimes = ref<DetectedJava[]>([]);
const javaScanning = ref(false);
const javaGroups = computed(() => [...new Set(javaRuntimes.value.map(runtime => runtime.major))].sort((a, b) => b - a));
async function scanJava(rescan = false) {
  if (!native || javaScanning.value) return;
  javaScanning.value = true;
  try { javaRuntimes.value = (await invoke<{ runtimes: DetectedJava[] }>('list_java_runtimes', { rescan })).runtimes; }
  catch { if (!javaError.value) javaError.value = t('settingsLayout.javaFailed'); }
  finally { javaScanning.value = false; }
}
function chooseDetectedJava(runtimeId: string) {
  const runtime = javaRuntimes.value.find(runtime => runtime.id === runtimeId);
  if (!runtime || javaBusy.value) return;
  javaRuntimeId.value = runtime.id;
  javaSelection.value = { mode: 'custom', executable: runtime.executable, runtime };
  javaError.value = '';
}
async function loadJava() {
  if (!native) return;
  try { javaSelection.value = await invoke<JavaSelection>('get_java_settings'); javaSaved.value = { ...javaSelection.value }; javaReady.value = true; }
  catch { javaError.value = t('settingsLayout.loadFailed'); }
  finally { javaReady.value = true; }
}
async function selectJava() {
  if (javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try {
    const selected = await invoke<DetectedJava | null>('pick_java_candidate');
    if (selected) { javaRuntimeId.value = selected.id; javaSelection.value = { mode: 'custom', executable: selected.executable, runtime: selected }; }
  } catch { javaError.value = t('settingsLayout.javaFailed'); }
  finally { javaBusy.value = false; }
}
function automaticJava() { javaSelection.value = { mode: 'automatic', executable: null, runtime: null }; javaRuntimeId.value = null; javaError.value = ''; }
async function saveJava() {
  if (!javaDirty.value || javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try {
    javaSelection.value = await invoke<JavaSelection>(javaSelection.value.mode === 'automatic' ? 'use_automatic_java' : 'select_java_runtime', javaSelection.value.mode === 'automatic' ? undefined : { runtimeId: javaRuntimeId.value });
    javaSaved.value = { ...javaSelection.value };
  } catch { javaError.value = t('settingsLayout.javaFailed'); }
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
  try { preferences.value = await invoke<LauncherPreferences>('get_launcher_preferences'); memoryDraft.value = preferences.value.memory_mb; directoryDraft.value = preferences.value.game_directory; }
  catch { preferencesError.value = t('settingsLayout.loadFailed'); }
  finally { preferencesBusy.value = false; }
}
async function saveMemory() {
  if (!native || !preferences.value || memoryBusy.value) return;
  memoryError.value = '';
  if (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144) { memoryError.value = t('settingsLayout.memoryInvalid'); return; }
  memoryBusy.value = true;
  try { preferences.value = await invoke<LauncherPreferences>('set_default_memory', { memoryMb: memoryDraft.value }); }
  catch { memoryError.value = t('settingsLayout.saveFailed'); }
  finally { memoryBusy.value = false; }
}
const directoryReset = ref(false);
const directoryResetOpen = ref(false);
async function changeDirectory(reset = false) {
  if (!native || directoryBusy.value || !preferences.value) return;
  directoryBusy.value = true; directoryError.value = '';
  try {
    const value = await invoke<{ path: string } | null>(reset ? 'get_standard_game_directory' : 'pick_default_game_directory_candidate');
    if (value) { directoryDraft.value = value.path; directoryReset.value = reset && preferences.value.custom_game_directory; }
  } catch { directoryError.value = t('settingsLayout.directoryFailed'); }
  finally { directoryBusy.value = false; }
}
watch(activeCategory, (category) => {
  if (category === 'downloads') { void updateCache(); void loadTuning(); }
  if (category === 'runtime' && !javaReady.value) { void loadJava(); void scanJava(); }
  if ((category === 'runtime' || category === 'downloads') && !preferences.value) void loadPreferences();
}, { immediate: true });
onMounted(() => { void loadSettings(); });

const directoryDraft = ref('');
const directoryDirty = computed(() => !!preferences.value && (directoryReset.value || directoryDraft.value !== preferences.value.game_directory));
const copied = ref(false);
const advanced = ref<InstanceType<typeof AdvancedRuntimeSettings> | null>(null);
const advancedDirty = ref(false);
const appearance = ref<InstanceType<typeof AppearanceSettings> | null>(null);
const appearanceDirty = ref(false);
const memoryDirty = computed(() => !!preferences.value && memoryDraft.value !== preferences.value.memory_mb);
const homeLayout = ref<InstanceType<typeof HomeLayoutSettings> | null>(null);
const homeLayoutDirty = ref(false);
const shortcuts = ref<InstanceType<typeof ShortcutSettings> | null>(null);
const shortcutsDirty = ref(false);
const savingAll = ref(false);
const saveError = ref('');
const hasDrafts = computed(() => homeLayoutDirty.value || shortcutsDirty.value || languageDirty.value || javaDirty.value || appearanceDirty.value || hasDownloadChanges.value || editingMirror.value || tuningDirty.value || memoryDirty.value || directoryDirty.value || advancedDirty.value);
const pendingNative = computed(() => savingAll.value || savingSettings.value || tuningBusy.value || memoryBusy.value || directoryBusy.value || javaBusy.value || advanced.value?.busy);
function cancelDownloads() { if (loadedDownloadSettings.value) downloadSettings.value = { ...loadedDownloadSettings.value }; editingMirror.value = false; settingsError.value = ''; }
async function applyDirectory() {
  if (!directoryDirty.value || directoryBusy.value) return;
  directoryBusy.value = true; directoryError.value = '';
  try { preferences.value = await invoke<LauncherPreferences>(directoryReset.value ? 'reset_default_game_directory' : 'set_default_game_directory', directoryReset.value ? undefined : { path: directoryDraft.value.trim() }); directoryDraft.value = preferences.value.game_directory; directoryReset.value = false; }
  catch (cause) { directoryError.value = `${t('settingsLayout.directoryFailed')} ${String(cause)}`; }
  finally { directoryBusy.value = false; }
}
async function copyDirectory() {
  if (!preferences.value) return;
  try { await writeText(directoryDraft.value); copied.value = true; }
  catch { directoryError.value = t('settingsLayout.clipboardFailed'); }
}
function cancelAll() {
  appearance.value?.cancel(); homeLayout.value?.cancel(); shortcuts.value?.cancel();
  languageDraft.value = languageSaved.value;
  if (javaSaved.value) javaSelection.value = { ...javaSaved.value };
  javaRuntimeId.value = null; javaError.value = '';
  cancelDownloads(); tuning.value = { ...tuningSaved.value }; tuningError.value = '';
  if (preferences.value) { memoryDraft.value = preferences.value.memory_mb; directoryDraft.value = preferences.value.game_directory; }
  directoryReset.value = false; directoryError.value = ''; memoryError.value = '';
  advanced.value?.cancel(); saveError.value = '';
}
async function applyAll() {
  if (pendingNative.value) return false;
  savingAll.value = true; saveError.value = '';
  let complete = false;
  let group = 'downloads';
  try {
    if (editingMirror.value) { await addMirror(); if (editingMirror.value) return false; }
    if (hasDownloadChanges.value && validateDownloadSettings(downloadSettings.value)) { settingsError.value = t('settingsLayout.customMirrorInvalid'); return false; }
    if (memoryDirty.value && (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144)) { group = 'runtime'; memoryError.value = t('settingsLayout.memoryInvalid'); return false; }
    if (javaDirty.value) { group = 'runtime'; await saveJava(); if (javaDirty.value) return false; }
    if (advancedDirty.value) { group = 'runtime'; await advanced.value?.save(); await nextTick(); if (advancedDirty.value) return false; }
    if (hasDownloadChanges.value) { group = 'downloads'; await finishSettings(); if (hasDownloadChanges.value) return false; }
    if (tuningDirty.value) { group = 'downloads'; await saveTuning(); if (tuningDirty.value) return false; }
    if (memoryDirty.value) { group = 'runtime'; await saveMemory(); if (memoryDirty.value) return false; }
    if (directoryDirty.value) { group = 'downloads'; await applyDirectory(); if (directoryDirty.value) return false; }
    if (shortcutsDirty.value) { group = 'shortcuts'; if (!shortcuts.value?.save()) return false; }
    if (homeLayoutDirty.value) homeLayout.value?.save();
    if (languageDirty.value) { setLocale(languageDraft.value); languageSaved.value = languageDraft.value; }
    if (appearanceDirty.value) appearance.value?.save();
    await nextTick(); complete = !hasDrafts.value; return complete;
  } finally {
    const category = categories.value.find(category => category.id === group);
    saveError.value = complete ? '' : t('settingsLayout.editor.saveFailedCategory', { category: category?.labelKey ? t(category.labelKey) : category?.label ?? '' });
    savingAll.value = false;
  }
}
const categoryChanging = ref(false);
const leavePrompt = ref(false);
let leaveResolver: ((value: boolean) => void) | null = null;
function canLeave() { return !props.beforeCategoryLeave && !pendingNative.value && !hasDrafts.value; }
async function selectCategory(value: unknown) {
  if (typeof value !== 'string' || !categories.value.some(category => category.id === value) || value === activeCategory.value || pendingNative.value || categoryChanging.value) return;
  categoryChanging.value = true;
  try {
    if (props.beforeCategoryLeave && !await props.beforeCategoryLeave()) return;
    activeCategory.value = value;
  } finally { categoryChanging.value = false; }
}
async function confirmLeave(): Promise<boolean> {
  if (pendingNative.value) return false;
  if (props.beforeCategoryLeave && !await props.beforeCategoryLeave()) return false;
  if (!hasDrafts.value) return Promise.resolve(true);
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
async function applyAndLeave() { await applyAll(); if (!hasDrafts.value) finishLeave(true); else finishLeave(false); }
onUnmounted(() => { leaveResolver?.(false); });
watch(() => props.initialCategory, value => { if (value && categories.value.some(category => category.id === value)) void selectCategory(value); });
defineExpose({ canLeave, confirmLeave, openCategory: selectCategory });
</script>
<template>
  <div class="settings-page page-content settings-layout">
    <TabsRoot
      class="settings-workspace"
      orientation="vertical"
      activation-mode="manual"
      :model-value="activeCategory"
      :unmount-on-hide="false"
      @update:model-value="selectCategory"
    >
      <SettingsNavigation
        :categories="categories"
        :active="activeCategory"
        @select="selectCategory"
      />
      <div
        class="settings-main"
        :aria-busy="savingAll"
      >
        <header class="settings-category-heading">
          <h2>{{ categoryTitle }}</h2>
        </header>
        <div
          class="settings-panels"
          :inert="savingAll || undefined"
        >
          <TabsContent
            value="general"
            class="settings-category-content"
          >
            <AppearanceSettings
              ref="appearance"
              @dirty="appearanceDirty = $event"
            />
            <SettingsSection
              class="settings-language"
              icon="languages"
              :title="sectionNames.language"
            >
              <SelectField
                id="language"
                :model-value="languageDraft"
                :label="t('common.language')"
                :options="[{ value: 'system', label: t('common.system') }, ...supportedLocales]"
                @update:model-value="stageLanguage"
              />
            </SettingsSection>
          </TabsContent>
          <TabsContent
            value="home-layout"
            class="settings-category-content"
          >
            <HomeLayoutSettings
              ref="homeLayout"
              :commands="commands"
              :games="games"
              @dirty="homeLayoutDirty = $event"
            />
          </TabsContent>
          <TabsContent
            value="shortcuts"
            class="settings-category-content"
          >
            <ShortcutSettings
              ref="shortcuts"
              :commands="commands"
              staged
              @dirty="shortcutsDirty = $event"
            />
          </TabsContent>
          <TabsContent
            value="downloads"
            class="settings-category-content"
          >
            <SettingsSection
              :title="sectionNames.downloads"
              icon="download"
            >
              <SettingsRow
                :label="t('settingsLayout.downloadSource')"
                :error="settingsError"
                :help="downloadSettings.source === 'automatic' ? t('settingsLayout.automaticSource') : downloadSettings.source === 'bmclapi' ? t('settingsLayout.mirrorCredit') : undefined"
              >
                <RadioGroupRoot
                  class="settings-choice-group"
                  :model-value="downloadSettings.source"
                  :disabled="!settingsReady || savingSettings"
                  :aria-label="t('settingsLayout.downloadSource')"
                  @update:model-value="changeSource"
                >
                  <RadioGroupItem
                    v-for="option in sourceOptions"
                    :key="option.value"
                    :value="option.value"
                    class="settings-choice"
                  >
                    {{ option.label }}
                  </RadioGroupItem>
                </RadioGroupRoot>
                <div class="settings-inline-actions">
                  <button
                    class="text-button"
                    :disabled="!settingsReady || savingSettings"
                    @click="editMirror"
                  >
                    {{ downloadSettings.mirror ? t('settingsLayout.editMirror') : t('settingsLayout.addMirror') }}
                  </button>
                  <button
                    v-if="downloadSettings.mirror"
                    class="text-button"
                    :disabled="savingSettings"
                    @click="removeMirror"
                  >
                    {{ t('settingsLayout.removeMirror') }}
                  </button>
                </div>
                <form
                  v-if="editingMirror"
                  class="settings-mirror-draft"
                  @submit.prevent="addMirror"
                >
                  <label for="mirror-url">{{ t('settingsLayout.mirror') }}</label>
                  <input
                    id="mirror-url"
                    v-model="mirrorDraft"
                    type="url"
                    :placeholder="t('settingsLayout.mirrorPlaceholder')"
                    :disabled="savingSettings"
                    spellcheck="false"
                    autocomplete="off"
                  >
                  <p class="setting-hint">
                    {{ t('settingsLayout.mirrorHelp') }}
                  </p>
                  <div class="settings-inline-actions">
                    <button
                      type="submit"
                      class="text-button"
                    >
                      {{ t('settingsLayout.useMirror') }}
                    </button><button
                      type="button"
                      class="text-button"
                      @click="editingMirror = false"
                    >
                      {{ t('settingsLayout.cancel') }}
                    </button>
                  </div>
                </form>
                <button
                  v-if="!settingsReady && settingsError"
                  class="text-button"
                  @click="loadSettings"
                >
                  {{ t('settingsLayout.retry') }}
                </button>
              </SettingsRow>

              <SettingsRow
                :label="t('settingsLayout.advancedDownloads')"
                :help="t('settingsLayout.nextDownloads')"
                :error="tuningError"
              >
                <label
                  for="adaptive-downloads"
                  class="settings-inline-actions"
                >
                  <SwitchField
                    id="adaptive-downloads"
                    v-model="tuning.adaptive"
                    :label="t('settingsLayout.adaptiveDownloads')"
                    :disabled="!tuningReady || tuningBusy"
                  />
                  {{ t('settingsLayout.adaptiveDownloads') }}
                </label>
                <SettingsDisclosure
                  :title="t('settingsLayout.downloadLimits')"
                  icon="sliders-horizontal"
                  :value="`${tuning.max_concurrent} / ${tuning.per_origin}`"
                  :default-open="!tuning.adaptive"
                >
                  <div class="settings-number-pair">
                    <label for="total-downloads">{{ t('settingsLayout.totalDownloads') }}<input
                      id="total-downloads"
                      v-model.number="tuning.max_concurrent"
                      type="number"
                      min="1"
                      max="32"
                      :disabled="!tuningReady || tuningBusy"
                    ></label>
                    <label for="origin-downloads">{{ t('settingsLayout.originDownloads') }}<input
                      id="origin-downloads"
                      v-model.number="tuning.per_origin"
                      type="number"
                      min="1"
                      max="8"
                      :disabled="!tuningReady || tuningBusy"
                    ></label>
                  </div>
                </SettingsDisclosure>
                <button
                  v-if="!tuningReady && tuningError"
                  class="text-button"
                  @click="loadTuning"
                >
                  {{ t('settingsLayout.retry') }}
                </button>
              </SettingsRow>
            </SettingsSection>
            <SettingsSection
              :title="sectionNames.storage"
              icon="hard-drive"
            >
              <SettingsRow
                :label="t('settingsLayout.directory')"
                input-id="default-game-directory"
                :help="t('settingsLayout.directoryHelp')"
                :error="directoryError || preferencesError"
              >
                <div class="settings-path-control">
                  <input
                    id="default-game-directory"
                    v-model="directoryDraft"
                    :disabled="!preferences || directoryBusy || !native"
                    spellcheck="false"
                    autocomplete="off"
                    @input="directoryReset = false"
                  >
                  <AppIconButton
                    icon="folder"
                    :label="t('settingsLayout.browse')"
                    :disabled="!preferences || directoryBusy || !native"
                    @click="changeDirectory()"
                  />
                  <AppIconButton
                    icon="copy"
                    :label="t('settingsLayout.copy')"
                    :disabled="!preferences || directoryBusy"
                    @click="copyDirectory"
                  />
                  <AppIconButton
                    v-if="preferences?.custom_game_directory"
                    icon="refresh-cw"
                    :label="t('settingsLayout.reset')"
                    :disabled="directoryBusy"
                    @click="directoryResetOpen = true"
                  />
                </div>
                <button
                  v-if="preferencesError"
                  class="text-button"
                  @click="loadPreferences"
                >
                  {{ t('settingsLayout.retry') }}
                </button>
                <p
                  v-if="copied"
                  role="status"
                  class="setting-hint"
                >
                  {{ t('settingsLayout.copied') }}
                </p>
              </SettingsRow>

              <GameDirectoryMigration
                :disabled="!preferences || directoryDirty || directoryBusy || !native || savingSettings"
                @open="emit('migrateGames')"
              />
              <SettingsRow
                :label="t('settingsLayout.cache')"
                :help="t('settingsLayout.cacheHelp')"
                :error="cacheError"
              >
                <div class="settings-inline-actions">
                  <span v-if="cacheBytes !== null">{{ (cacheBytes / 1024 / 1024).toFixed(1) }} MiB</span><button
                    class="secondary-button"
                    :disabled="!native || cacheBusy"
                    @click="updateCache(true)"
                  >
                    {{ cacheBusy ? t('settingsLayout.wait') : t('settingsLayout.clearCache') }}
                  </button>
                </div>
              </SettingsRow>
            </SettingsSection>
          </TabsContent>
          <TabsContent
            value="runtime"
            class="settings-category-content"
          >
            <p class="setting-hint settings-scope">
              {{ t('settingsLayout.defaultsScope') }}
            </p>
            <SettingsSection
              :title="sectionNames.runtime"
              icon="cpu"
            >
              <SettingsRow
                :label="t('settingsLayout.java')"
                :error="javaError"
              >
                <div class="settings-inline-actions">
                  <span>{{ javaSelection.mode === 'automatic' ? t('settingsLayout.recommended') : javaSelection.runtime ? `Java ${javaSelection.runtime.major}` : t('settingsLayout.selectedJava') }}</span><button
                    v-if="javaSelection.mode === 'custom'"
                    class="text-button"
                    :disabled="javaBusy"
                    @click="automaticJava"
                  >
                    {{ t('settingsLayout.javaAutomatic') }}
                  </button>
                </div>
                <p
                  v-if="javaSelection.executable"
                  class="settings-path-text"
                  :title="javaSelection.executable ?? undefined"
                >
                  {{ javaSelection.executable }}
                </p>
                <SettingsDisclosure
                  class="settings-manual-java"
                  icon="folder"
                  :title="t('settingsLayout.manualJava')"
                >
                  <div class="settings-manual-content">
                    <label for="detected-java">{{ t('settingsLayout.installedJava') }}</label>
                    <SelectField
                      id="detected-java"
                      :label="t('settingsLayout.installedJava')"
                      :disabled="javaBusy || javaScanning"
                      :model-value="javaRuntimes.find(runtime => runtime.executable === javaSelection.executable)?.id ?? ''"
                      :placeholder="javaScanning ? t('settingsLayout.scanning') : javaRuntimes.length ? t('settingsLayout.chooseInstalled') : t('settingsLayout.noJava')"
                      :options="javaGroups.flatMap(major => javaRuntimes.filter(item => item.major === major).map(runtime => ({ value: runtime.id, label: `${runtime.version} · ${runtime.vendor} · ${runtime.arch}`, group: `Java ${major}` })))"
                      @update:model-value="chooseDetectedJava"
                    />
                    <div class="settings-inline-actions">
                      <button
                        class="text-button"
                        :disabled="javaScanning || javaBusy"
                        @click="scanJava(true)"
                      >
                        {{ t('settingsLayout.scan') }}
                      </button><button
                        class="secondary-button"
                        :disabled="javaBusy || !javaReady || !native"
                        @click="selectJava"
                      >
                        {{ t('settingsLayout.chooseJava') }}
                      </button>
                    </div>
                  </div>
                </SettingsDisclosure>
              </SettingsRow>
              <SettingsRow
                :label="t('settingsLayout.memory')"
                input-id="default-memory"
                :error="memoryError || preferencesError"
              >
                <MemoryField
                  id="default-memory"
                  v-model="memoryDraft"
                  :label="t('settingsLayout.defaultMemory')"
                  :disabled="!preferences || memoryBusy || !native"
                />
              </SettingsRow>
            </SettingsSection>
            <AdvancedRuntimeSettings
              v-if="preferences"
              ref="advanced"
              :preferences="preferences"
              :inline-actions="false"
              @defaults-updated="preferences = $event"
              @dirty="advancedDirty = $event"
            />
          </TabsContent>
          <TabsContent
            value="accounts"
            class="settings-category-content"
          >
            <SettingsSection>
              <SettingsRow :label="t('settings.accounts')">
                <button
                  class="secondary-button"
                  @click="emit('profiles')"
                >
                  {{ t('settingsLayout.manageProfiles') }}
                </button>
              </SettingsRow>
            </SettingsSection>
          </TabsContent>
          <TabsContent
            value="about"
            class="settings-category-content"
          >
            <UpdateSettings />
            <SettingsSection>
              <SettingsRow :label="locale === 'zh-CN' ? '首次使用' : 'Getting started'">
                <button
                  class="secondary-button"
                  @click="emit('firstRun')"
                >
                  {{ locale === 'zh-CN' ? '继续设置引导' : 'Resume setup' }}
                </button>
                <button
                  class="secondary-button"
                  @click="emit('reviewTerms')"
                >
                  {{ locale === 'zh-CN' ? '使用条款与隐私' : 'Terms and privacy' }}
                </button>
              </SettingsRow>
              <SettingsRow label="Cobble Launcher">
                <span>0.1.0</span><p class="setting-hint">
                  {{ t('settingsLayout.license') }}
                </p><a
                  href="https://github.com/CobbleLauncher/CobbleLauncher"
                  target="_blank"
                  rel="noreferrer"
                >{{ t('settingsLayout.project') }}</a>
              </SettingsRow>
            </SettingsSection>
            <SettingsSection>
              <SettingsRow
                :label="t('settingsLayout.diagnostics')"
                :error="diagnosticError"
              >
                <div class="settings-inline-actions">
                  <button
                    class="secondary-button"
                    :disabled="!native || diagnosticBusy"
                    @click="diagnostics()"
                  >
                    {{ t('settings.openLogs') }}
                  </button><button
                    class="secondary-button"
                    :disabled="!native || diagnosticBusy"
                    @click="diagnostics(true)"
                  >
                    {{ t('settings.exportDiagnostics') }}
                  </button>
                </div>
              </SettingsRow>
            </SettingsSection>
          </TabsContent>
          <TabsContent
            v-for="category in categories.filter(item => !['general','home-layout','shortcuts','downloads','runtime','accounts','about'].includes(item.id))"
            :key="category.id"
            :value="category.id"
            class="settings-category-content"
          >
            <slot :name="`category-${category.id}`" />
          </TabsContent>
        </div>
        <SettingsSaveBar
          :dirty="hasDrafts"
          :busy="pendingNative"
          :error="saveError"
          @save="applyAll"
          @discard="cancelAll"
        />
      </div>
    </TabsRoot>
    <ConfirmationDialog
      v-model:open="directoryResetOpen"
      :title="t('settingsLayout.editor.resetDirectoryTitle')"
      :description="t('settingsLayout.editor.resetDraftDescription')"
      :confirm-label="t('settingsLayout.editor.restore')"
      @confirm="changeDirectory(true)"
    />
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="settings-dialog-overlay" /><DialogContent
          class="settings-leave-dialog"
          :aria-describedby="undefined"
        >
          <DialogTitle>{{ t('settingsLayout.discardTitle') }}</DialogTitle><div class="settings-inline-actions">
            <button
              class="secondary-button"
              @click="applyAndLeave"
            >
              {{ t('settingsLayout.saveSettings') }}
            </button><button
              class="text-button"
              @click="cancelAll(); finishLeave(true)"
            >
              {{ t('settingsLayout.discard') }}
            </button><button
              class="text-button"
              @click="finishLeave(false)"
            >
              {{ t('settingsLayout.continueEditing') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
  </div>
</template>
<style scoped>
.settings-layout{max-width:100%;min-width:0}.settings-category-content{min-width:0}.settings-choice-group{display:flex;gap:8px;flex-wrap:wrap}.settings-choice{font:inherit;font-size:var(--ui-text-label);padding:10px 14px;border:1px solid var(--line);border-radius:8px;color:var(--muted);background:var(--panel)}.settings-choice[data-state=checked]{color:var(--ink);border-color:var(--accent);background:var(--tint)}.settings-inline-actions{display:flex;gap:12px;align-items:center;flex-wrap:wrap}.settings-row-control .settings-inline-actions{margin-top:10px}.settings-mirror-draft{display:grid;gap:8px;margin-top:12px}.settings-path-control{display:flex;gap:3px;align-items:center;min-width:0;border:1px solid var(--line);border-radius:8px;padding:3px;background:var(--panel)}.settings-path-control input{flex:1;width:0!important;min-width:0;font-family:inherit;border:0!important;background:transparent!important;text-overflow:ellipsis}.settings-path-text{font:inherit;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin:8px 0;color:var(--muted)}.settings-number-pair{display:grid;grid-template-columns:1fr 1fr;gap:14px}.settings-number-pair label{display:grid;gap:var(--ui-space-2);font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.settings-manual-content{display:grid;gap:10px;padding-top:14px}.settings-manual-content label{font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.settings-scope{font-size:12px;color:var(--muted);margin:14px 0}.settings-category-content :deep(.memory-field>label){display:none}.settings-category-content :deep(.memory-presets){margin-top:0}.settings-leave-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}.settings-leave-dialog h2{font-size:16px;margin:0 0 20px}.settings-dialog-overlay{position:fixed;inset:0;background:#0006;z-index:100}
</style>
<style scoped src="./settings/appearance.css"></style>
