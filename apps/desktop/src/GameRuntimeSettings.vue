<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, JavaSelection, LauncherPreferences, RuntimeOverrides } from './games';
import AdvancedRuntimeSettings from './AdvancedRuntimeSettings.vue';
import MemoryField from './MemoryField.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsActions from './ui/settings/SettingsActions.vue';
import AppIcon from './ui/AppIcon.vue';
import SelectField, { type SelectFieldOption } from './ui/SelectField.vue';
import { locale, t } from './i18n';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle } from 'reka-ui';
const props = defineProps<{ game: Game }>();
const emit = defineEmits<{ updated: [game: Game] }>();
type JavaMode = RuntimeOverrides['java']['mode'];
type DetectedJava = { id: string; version: string; major: number; arch: string; vendor: string; executable: string; recommended: boolean };
const preferences = ref<LauncherPreferences | null>(null), defaultJava = ref<JavaSelection | null>(null);
const preferencesError = ref(''), defaultJavaError = ref(''), javaError = ref(''), memoryError = ref('');
const javaBusy = ref(false), memoryBusy = ref(false), groupBusy = ref(false), partial = ref(false);
const savedJava = ref<RuntimeOverrides['java']>(props.game.runtime_overrides?.java ?? { mode: 'inherit' });
const savedMemory = ref<number | null>(props.game.runtime_overrides?.memory_mb ?? null);
const javaMode = ref<JavaMode>(savedJava.value.mode), detectedDraft = ref<DetectedJava | null>(null);
const memoryMode = ref<'inherit' | 'custom'>(savedMemory.value == null ? 'inherit' : 'custom');
const memoryDraft = ref(savedMemory.value ?? 4096);
const javaList = ref<{ runtimes: DetectedJava[]; requiredMajor: number; recommendedId: string | null } | null>(null);
const scanningJava = ref(false), advancedDirty = ref(false);
const advanced = ref<InstanceType<typeof AdvancedRuntimeSettings> | null>(null);
const locked = computed(() => props.game.status !== 'installed' || !!props.game.content_installing || (props.game.launch_state ?? 'idle') !== 'idle');
const basicBusy = computed(() => javaBusy.value || memoryBusy.value || groupBusy.value);
const pending = computed(() => basicBusy.value || !!advanced.value?.busy);
let generation = 0;
function displayPath(path?: string | null): string | null {
  if (path?.startsWith('\\\\?\\UNC\\')) return `\\\\${path.slice(8)}`;
  return path?.startsWith('\\\\?\\') ? path.slice(4) : path ?? null;
}
const javaPath = computed(() => displayPath(javaMode.value === 'custom' ? detectedDraft.value?.executable ?? savedJava.value.executable : javaMode.value === 'inherit' ? defaultJava.value?.executable : null));
const javaDirty = computed(() => javaMode.value !== savedJava.value.mode || (javaMode.value === 'custom' && displayPath(detectedDraft.value?.executable ?? savedJava.value.executable) !== displayPath(savedJava.value.executable)));
const memoryDirty = computed(() => (memoryMode.value === 'inherit' ? null : memoryDraft.value) !== savedMemory.value);
const groupDirty = computed(() => javaDirty.value || memoryDirty.value);
const javaGroups = computed(() => {
  const groups = new Map<number, DetectedJava[]>();
  for (const runtime of javaList.value?.runtimes ?? []) groups.set(runtime.major, [...(groups.get(runtime.major) ?? []), runtime]);
  return [...groups].sort(([a], [b]) => b - a);
});
const detectedJavaOptions = computed<SelectFieldOption[]>(() => javaGroups.value.flatMap(([major, runtimes]) => runtimes.map(runtime => ({ value: runtime.id, label: `${runtime.version} · ${runtime.vendor} · ${runtime.arch}`, disabled: runtime.major !== javaList.value?.requiredMajor, group: `Java ${major}` }))));
const javaModeOptions = computed<SelectFieldOption[]>(() => [{ value: 'inherit', label: t('settingsLayout.inherit') }, { value: 'automatic', label: t('settingsLayout.javaAutomatic') }, ...(javaMode.value === 'custom' || savedJava.value.mode === 'custom' ? [{ value: 'custom', label: t('settingsLayout.selectedJava') }] : [])]);
const memoryModeOptions = computed<SelectFieldOption[]>(() => [{ value: 'inherit', label: t('settingsLayout.inherit') }, { value: 'custom', label: t('settingsLayout.custom') }]);
async function scanJava(rescan = false) {
  if (scanningJava.value) return;
  const current = generation; scanningJava.value = true; javaError.value = '';
  try { const result = await invoke<typeof javaList.value>('list_game_java_runtimes', { id: props.game.id, rescan }); if (current === generation) javaList.value = result; }
  catch { if (current === generation) javaError.value = t('settingsLayout.javaFailed'); }
  finally { if (current === generation) scanningJava.value = false; }
}
function selectDetectedJava(runtimeId: string) {
  if (basicBusy.value || locked.value) return;
  const runtime = javaList.value?.runtimes.find(item => item.id === runtimeId);
  if (!runtime || runtime.major !== javaList.value?.requiredMajor) return;
  detectedDraft.value = runtime; javaMode.value = 'custom'; javaError.value = ''; partial.value = false;
}
function changeJava(value: string) {
  if (basicBusy.value || locked.value || !['inherit', 'automatic', 'custom'].includes(value)) return;
  javaMode.value = value as JavaMode; detectedDraft.value = null; javaError.value = ''; partial.value = false;
}
async function pickJava() {
  if (basicBusy.value || locked.value) return;
  const current = generation; javaBusy.value = true; javaError.value = '';
  try {
    const candidate = await invoke<DetectedJava | null>('pick_game_java_candidate', { id: props.game.id });
    if (candidate && current === generation) {
      detectedDraft.value = candidate; javaMode.value = 'custom'; partial.value = false;
      if (javaList.value && !javaList.value.runtimes.some(item => item.id === candidate.id)) javaList.value.runtimes.push(candidate);
    }
  } catch { if (current === generation) javaError.value = t('settingsLayout.javaFailed'); }
  finally { javaBusy.value = false; }
}
async function loadPreferences() {
  const current = generation; preferencesError.value = '';
  try { const result = await invoke<LauncherPreferences>('get_launcher_preferences'); if (current === generation) { preferences.value = result; if (!memoryDirty.value && memoryMode.value === 'inherit') memoryDraft.value = result.memory_mb; } }
  catch { if (current === generation) preferencesError.value = t('settingsLayout.loadFailed'); }
}
async function loadDefaultJava() {
  const current = generation; defaultJavaError.value = '';
  try { const result = await invoke<JavaSelection>('get_java_settings'); if (current === generation) defaultJava.value = result; }
  catch { if (current === generation) defaultJavaError.value = t('settingsLayout.loadFailed'); }
}
function validMemory(): boolean {
  memoryError.value = '';
  if (memoryMode.value === 'custom' && (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144)) { memoryError.value = t('settingsLayout.memoryInvalid'); return false; }
  return true;
}
async function saveJava(): Promise<boolean> {
  if (!javaDirty.value) return true;
  if (javaBusy.value || locked.value) return false;
  const current = generation; javaBusy.value = true; javaError.value = '';
  try {
    const game = javaMode.value === 'custom' && detectedDraft.value
      ? await invoke<Game>('set_game_java_runtime', { id: props.game.id, runtimeId: detectedDraft.value.id })
      : await invoke<Game>('set_game_java_mode', { id: props.game.id, mode: javaMode.value });
    if (current !== generation) return false;
    savedJava.value = game.runtime_overrides?.java ?? { mode: javaMode.value, executable: detectedDraft.value?.executable };
    emit('updated', game);
    if (javaDirty.value) { javaError.value = t('settingsLayout.saveFailed'); return false; }
    detectedDraft.value = null;
    return true;
  } catch { if (current === generation) javaError.value = t('settingsLayout.saveFailed'); return false; }
  finally { javaBusy.value = false; }
}
async function saveMemory(): Promise<boolean> {
  if (!memoryDirty.value) return true;
  if (memoryBusy.value || locked.value || !validMemory()) return false;
  const current = generation; const target = memoryMode.value === 'inherit' ? null : memoryDraft.value; memoryBusy.value = true;
  try {
    const game = await invoke<Game>('set_game_memory', { id: props.game.id, memoryMb: target });
    if (current !== generation) return false;
    savedMemory.value = game.runtime_overrides?.memory_mb === undefined ? target : game.runtime_overrides.memory_mb;
    emit('updated', game);
    if (memoryDirty.value) { memoryError.value = t('settingsLayout.saveFailed'); return false; }
    return true;
  } catch { if (current === generation) memoryError.value = t('settingsLayout.saveFailed'); return false; }
  finally { memoryBusy.value = false; }
}
async function saveGroup(): Promise<boolean> {
  if (pending.value || locked.value || !validMemory()) return false;
  groupBusy.value = true; partial.value = false;
  const changedJava = javaDirty.value;
  try {
    if (!await saveJava()) return false;
    if (!await saveMemory()) { partial.value = changedJava && !javaDirty.value; return false; }
    return !groupDirty.value;
  } finally { groupBusy.value = false; }
}
function cancelGroup() {
  if (basicBusy.value) return;
  javaMode.value = savedJava.value.mode; detectedDraft.value = null;
  memoryMode.value = savedMemory.value == null ? 'inherit' : 'custom'; memoryDraft.value = savedMemory.value ?? preferences.value?.memory_mb ?? 4096;
  javaError.value = ''; memoryError.value = ''; partial.value = false;
}
watch(() => [props.game.id, props.game.runtime_overrides?.java, props.game.runtime_overrides?.memory_mb] as const, (value, previous) => {
  const switched = value[0] !== previous[0]; const keepJava = javaDirty.value && !switched; const keepMemory = memoryDirty.value && !switched;
  if (switched) { ++generation; javaError.value = ''; memoryError.value = ''; partial.value = false; javaList.value = null; scanningJava.value = false; void scanJava(); }
  savedJava.value = props.game.runtime_overrides?.java ?? { mode: 'inherit' };
  savedMemory.value = props.game.runtime_overrides?.memory_mb ?? null;
  if (!keepJava) { javaMode.value = savedJava.value.mode; detectedDraft.value = null; }
  if (!keepMemory) { memoryMode.value = savedMemory.value == null ? 'inherit' : 'custom'; memoryDraft.value = savedMemory.value ?? preferences.value?.memory_mb ?? 4096; }
});
onMounted(() => { void loadPreferences(); void loadDefaultJava(); void scanJava(); });
const leavePrompt = ref(false);
let leaveResolver: ((value: boolean) => void) | null = null;
function canLeave() { return !groupDirty.value && !advancedDirty.value && !pending.value; }
async function confirmLeave(): Promise<boolean> {
  if (pending.value) return false;
  if (canLeave()) return true;
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
function discard() { if (pending.value) return; cancelGroup(); advanced.value?.cancel(); finishLeave(true); }
async function applyAndLeave() {
  if (pending.value || locked.value) return;
  if (groupDirty.value && !await saveGroup()) { finishLeave(false); return; }
  if (advancedDirty.value) await advanced.value?.save();
  await nextTick(); finishLeave(canLeave());
}
onUnmounted(() => { ++generation; leaveResolver?.(false); });
defineExpose({ canLeave, confirmLeave });
</script>
<template>
  <div class="game-runtime-options">
    <SettingsSection>
      <form
        novalidate
        class="basic-runtime-group"
        @submit.prevent="saveGroup"
      >
        <SettingsRow
          :label="t('settingsLayout.java')"
          input-id="game-java-mode"
          :error="javaError || (javaMode === 'inherit' ? defaultJavaError : '')"
        >
          <SelectField
            id="game-java-mode"
            :model-value="javaMode"
            :options="javaModeOptions"
            :label="t('settingsLayout.java')"
            :disabled="basicBusy || locked"
            @update:model-value="changeJava"
          />
          <p
            v-if="javaMode === 'inherit' && defaultJava"
            class="setting-hint"
          >
            {{ t('settingsLayout.globalJava', { value: defaultJava.mode === 'automatic' ? t('settingsLayout.javaAutomatic') : defaultJava.runtime ? `Java ${defaultJava.runtime.major}` : t('settingsLayout.selectedJava') }) }}
          </p>
          <p
            v-if="javaPath"
            class="runtime-java-path"
            :title="javaPath ?? undefined"
          >
            <AppIcon name="folder" />{{ javaPath }}
          </p>
          <SettingsDisclosure
            class="runtime-manual-java"
            :title="t('settingsLayout.manualJava')"
          >
            <div
              v-if="javaList"
              class="runtime-manual-controls"
            >
              <label for="game-detected-java">{{ t('settingsLayout.installedJava') }} · Java {{ javaList.requiredMajor }}</label>
              <SelectField
                id="game-detected-java"
                :model-value="detectedDraft?.id ?? ''"
                :options="detectedJavaOptions"
                :label="t('settingsLayout.installedJava')"
                :placeholder="javaList.runtimes.length ? t('settingsLayout.chooseInstalled') : t('settingsLayout.noJava')"
                :disabled="basicBusy || locked || scanningJava || javaList.runtimes.length === 0"
                @update:model-value="selectDetectedJava"
              />
            </div>
            <div class="runtime-inline-actions">
              <button
                type="button"
                class="text-button"
                :disabled="scanningJava || basicBusy || locked"
                @click="scanJava(true)"
              >
                {{ scanningJava ? t('settingsLayout.scanning') : t('settingsLayout.scan') }}
              </button><button
                type="button"
                class="secondary-button"
                :disabled="basicBusy || locked"
                @click="pickJava"
              >
                {{ t('settingsLayout.chooseJava') }}
              </button>
            </div>
          </SettingsDisclosure>
        </SettingsRow>
        <SettingsRow
          :label="t('settingsLayout.memory')"
          input-id="game-memory-mode"
          :error="memoryError || preferencesError"
        >
          <SelectField
            id="game-memory-mode"
            v-model="memoryMode"
            :options="memoryModeOptions"
            :label="t('settingsLayout.memory')"
            :disabled="basicBusy || locked"
          />
          <p
            v-if="memoryMode === 'inherit' && preferences"
            class="setting-hint"
          >
            {{ t('settingsLayout.currentMemory', { value: preferences.memory_mb }) }}
          </p>
          <MemoryField
            v-if="memoryMode === 'custom'"
            id="game-memory"
            v-model="memoryDraft"
            :label="t('settingsLayout.gameMemory')"
            :disabled="basicBusy || locked"
          />
        </SettingsRow>
        <p
          v-if="partial"
          role="status"
          class="ui-warning"
        >
          {{ locale === 'en-US' ? 'Java saved. Memory changes are still unsaved.' : 'Java 已保存，内存改动尚未保存。' }}
        </p>
        <SettingsActions
          class="basic-runtime-actions"
          :dirty="groupDirty"
          :busy="basicBusy"
          :disabled="locked || !!advanced?.busy"
          @apply="saveGroup"
          @cancel="cancelGroup"
        />
      </form>
    </SettingsSection>
    <fieldset
      class="runtime-advanced-group"
      :disabled="basicBusy || locked"
      :inert="basicBusy || locked"
    >
      <AdvancedRuntimeSettings
        ref="advanced"
        :game="game"
        @updated="emit('updated', $event)"
        @dirty="advancedDirty = $event"
      />
    </fieldset>
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="runtime-dialog-overlay" /><DialogContent class="runtime-leave-dialog">
          <DialogTitle>{{ t('settingsLayout.discardTitle') }}</DialogTitle><div class="runtime-inline-actions">
            <button
              class="secondary-button"
              :disabled="pending || locked"
              @click="applyAndLeave"
            >
              {{ t('settingsLayout.apply') }}
            </button><button
              class="text-button"
              :disabled="pending"
              @click="discard"
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
.runtime-advanced-group{border:0;margin:0;padding:0;min-width:0}
.game-runtime-options{max-width:900px}.runtime-java-path{display:flex;align-items:center;gap:var(--ui-space-2);min-width:0;padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-2);font-family:var(--ui-font-mono);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--ui-text-muted);margin:var(--ui-space-2) 0}.runtime-java-path svg{flex:none;width:var(--ui-icon-sm);height:var(--ui-icon-sm)}.runtime-manual-controls{display:grid;gap:var(--ui-space-2);padding-top:var(--ui-space-3)}.runtime-manual-controls label{font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.runtime-inline-actions{display:flex;gap:var(--ui-space-3);align-items:center;flex-wrap:wrap;margin-top:var(--ui-space-3)}.game-runtime-options :deep(.memory-field){margin-top:var(--ui-space-4)}.game-runtime-options :deep(.memory-field>label){font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}
.runtime-leave-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));padding:var(--ui-space-5);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1);z-index:101}.runtime-leave-dialog h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);margin:0 0 var(--ui-space-4)}.runtime-dialog-overlay{position:fixed;inset:0;background:#0006;z-index:100}
</style>
