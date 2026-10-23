<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, JavaSelection, LauncherPreferences } from './games';
import AdvancedRuntimeSettings from './AdvancedRuntimeSettings.vue';
import MemoryField from './MemoryField.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsActions from './ui/settings/SettingsActions.vue';
import AppIcon from './ui/AppIcon.vue';
import SelectField, { type SelectFieldOption } from './ui/SelectField.vue';
import { t } from './i18n';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle } from 'reka-ui';
const props = defineProps<{ game: Game }>();
const emit = defineEmits<{ updated: [game: Game] }>();
const preferences = ref<LauncherPreferences | null>(null);
const defaultJava = ref<JavaSelection | null>(null);
const preferencesError = ref('');
const defaultJavaError = ref('');
const javaBusy = ref(false);
const javaError = ref('');
type DetectedJava = { id: string; version: string; major: number; arch: string; vendor: string; executable: string; recommended: boolean };
const javaList = ref<{ runtimes: DetectedJava[]; requiredMajor: number; recommendedId: string | null } | null>(null);
const scanningJava = ref(false);
const javaGroups = computed(() => {
  const groups = new Map<number, DetectedJava[]>();
  for (const runtime of javaList.value?.runtimes ?? []) groups.set(runtime.major, [...(groups.get(runtime.major) ?? []), runtime]);
  return [...groups].sort(([a], [b]) => b - a);
});
const detectedJavaOptions = computed<SelectFieldOption[]>(() => javaGroups.value.flatMap(([major, runtimes]) => runtimes.map(runtime => ({
  value: runtime.id,
  label: `${runtime.version} · ${runtime.vendor} · ${runtime.arch}`,
  disabled: runtime.major !== javaList.value?.requiredMajor,
  group: `Java ${major}`,
}))));
async function scanJava(rescan = false) {
  if (scanningJava.value) return;
  scanningJava.value = true; javaError.value = '';
  try { javaList.value = await invoke('list_game_java_runtimes', { id: props.game.id, rescan }); }
  catch (error) { javaError.value = String(error); }
  finally { scanningJava.value = false; }
}
async function selectDetectedJava(runtimeId: string) {
  if (!runtimeId || javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try { emit('updated', await invoke<Game>('set_game_java_runtime', { id: props.game.id, runtimeId })); }
  catch (error) { javaError.value = String(error); }
  finally { javaBusy.value = false; }
}
const memoryBusy = ref(false);
const memoryError = ref('');
const memoryMode = ref<'inherit' | 'custom'>(props.game.runtime_overrides?.memory_mb == null ? 'inherit' : 'custom');
const memoryDraft = ref(props.game.runtime_overrides?.memory_mb ?? 4096);
const javaMode = computed(() => props.game.runtime_overrides?.java.mode ?? 'inherit');
const javaModeOptions = computed<SelectFieldOption[]>(() => [
  { value: 'inherit', label: t('settingsLayout.inherit') },
  { value: 'automatic', label: t('settingsLayout.automatic') },
  ...(javaMode.value === 'custom' ? [{ value: 'custom', label: t('settingsLayout.selectedJava') }] : []),
]);
const memoryModeOptions = computed<SelectFieldOption[]>(() => [
  { value: 'inherit', label: t('settingsLayout.inherit') },
  { value: 'custom', label: t('settingsLayout.custom') },
]);
const javaPath = computed(() => {
  const path = javaMode.value === 'custom' ? props.game.runtime_overrides?.java.executable : javaMode.value === 'inherit' ? defaultJava.value?.executable : null;
  if (path?.startsWith('\\\\?\\UNC\\')) return `\\\\${path.slice(8)}`;
  return path?.startsWith('\\\\?\\') ? path.slice(4) : path;
});
async function loadPreferences() {
  preferencesError.value = '';
  try { preferences.value = await invoke<LauncherPreferences>('get_launcher_preferences'); if (props.game.runtime_overrides?.memory_mb == null) memoryDraft.value = preferences.value.memory_mb; }
  catch { preferencesError.value = t('settingsLayout.loadFailed'); }
}
async function loadDefaultJava() {
  defaultJavaError.value = '';
  try { defaultJava.value = await invoke<JavaSelection>('get_java_settings'); }
  catch { defaultJavaError.value = t('settingsLayout.loadFailed'); }
}
async function setJava(mode: 'inherit' | 'automatic') {
  if (javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try { emit('updated', await invoke<Game>('set_game_java_mode', { id: props.game.id, mode })); }
  catch { javaError.value = t('settingsLayout.saveFailed'); }
  finally { javaBusy.value = false; }
}
async function changeJava(value: string) {
  if (value === javaMode.value) return;
  if (value === 'inherit' || value === 'automatic') await setJava(value);
}
async function pickJava() {
  if (javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try { const game = await invoke<Game | null>('pick_game_java_executable', { id: props.game.id }); if (game) emit('updated', game); }
  catch { javaError.value = t('settingsLayout.javaFailed'); }
  finally { javaBusy.value = false; }
}
async function saveMemory(inherit = false) {
  if (memoryBusy.value) return;
  memoryError.value = '';
  if (!inherit && (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144)) { memoryError.value = t('settingsLayout.memoryInvalid'); return; }
  memoryBusy.value = true;
  try { emit('updated', await invoke<Game>('set_game_memory', { id: props.game.id, memoryMb: inherit ? null : memoryDraft.value })); if (inherit) memoryMode.value = 'inherit'; }
  catch { memoryError.value = t('settingsLayout.saveFailed'); }
  finally { memoryBusy.value = false; }
}
watch(() => [props.game.id, props.game.runtime_overrides?.memory_mb] as const, ([, value]) => { memoryMode.value = value == null ? 'inherit' : 'custom'; memoryDraft.value = value ?? preferences.value?.memory_mb ?? 4096; });
const advancedDirty = ref(false);
const memoryDirty = computed(() => memoryMode.value === 'inherit' ? props.game.runtime_overrides?.memory_mb != null : props.game.runtime_overrides?.memory_mb !== memoryDraft.value);
function cancelMemory() { memoryMode.value = props.game.runtime_overrides?.memory_mb == null ? 'inherit' : 'custom'; memoryDraft.value = props.game.runtime_overrides?.memory_mb ?? preferences.value?.memory_mb ?? 4096; memoryError.value = ''; }
onMounted(() => { void loadPreferences(); void loadDefaultJava(); void scanJava(); });

const advanced = ref<InstanceType<typeof AdvancedRuntimeSettings> | null>(null);
const pending = computed(() => memoryBusy.value || javaBusy.value || !!advanced.value?.busy);
const leavePrompt = ref(false);
let leaveResolver: ((value: boolean) => void) | null = null;
function canLeave() { return !memoryDirty.value && !advancedDirty.value && !pending.value; }
async function confirmLeave(): Promise<boolean> {
  if (pending.value) return false;
  if (canLeave()) return true;
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
function discard() { cancelMemory(); advanced.value?.cancel(); finishLeave(true); }
async function applyAndLeave() {
  if (advancedDirty.value) await advanced.value?.save();
  if (memoryDirty.value) await saveMemory(memoryMode.value === 'inherit');
  await nextTick();
  finishLeave(canLeave());
}
onUnmounted(() => { leaveResolver?.(false); });
defineExpose({ canLeave, confirmLeave });
</script>
<template>
  <div class="game-runtime-options">
    <SettingsSection>
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
          :disabled="javaBusy"
          @update:model-value="changeJava"
        />
        <p
          v-if="javaMode === 'inherit' && defaultJava"
          class="setting-hint"
        >
          {{ t('settingsLayout.globalJava', { value: defaultJava.mode === 'automatic' ? t('settingsLayout.automatic') : defaultJava.runtime ? `Java ${defaultJava.runtime.major}` : t('settingsLayout.selectedJava') }) }}
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
              model-value=""
              :options="detectedJavaOptions"
              :label="t('settingsLayout.installedJava')"
              :placeholder="javaList.runtimes.length ? t('settingsLayout.chooseInstalled') : t('settingsLayout.noJava')"
              :disabled="javaBusy || scanningJava || javaList.runtimes.length === 0"
              @update:model-value="selectDetectedJava"
            />
          </div>
          <div class="runtime-inline-actions">
            <button
              class="text-button"
              :disabled="scanningJava || javaBusy"
              @click="scanJava(true)"
            >
              {{ scanningJava ? t('settingsLayout.scanning') : t('settingsLayout.scan') }}
            </button><button
              class="secondary-button"
              :disabled="javaBusy"
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
        <form
          novalidate
          @submit.prevent="saveMemory(memoryMode === 'inherit')"
        >
          <SelectField
            id="game-memory-mode"
            v-model="memoryMode"
            :options="memoryModeOptions"
            :label="t('settingsLayout.memory')"
            :disabled="memoryBusy"
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
            :disabled="memoryBusy"
          />
          <SettingsActions
            :dirty="memoryDirty"
            :busy="memoryBusy"
            @apply="saveMemory(memoryMode === 'inherit')"
            @cancel="cancelMemory"
          />
        </form>
      </SettingsRow>
    </SettingsSection>
    <AdvancedRuntimeSettings
      ref="advanced"
      :game="game"
      @updated="emit('updated', $event)"
      @dirty="advancedDirty = $event"
    />
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="runtime-dialog-overlay" /><DialogContent class="runtime-leave-dialog">
          <DialogTitle>{{ t('settingsLayout.discardTitle') }}</DialogTitle><div class="runtime-inline-actions">
            <button
              class="secondary-button"
              @click="applyAndLeave"
            >
              {{ t('settingsLayout.apply') }}
            </button><button
              class="text-button"
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
.runtime-select{position:relative;max-width:480px}.runtime-select select{appearance:none;padding-right:38px!important}.runtime-select>svg{position:absolute;right:12px;top:50%;width:16px;height:16px;transform:translateY(-50%);pointer-events:none;color:var(--muted)}
.runtime-java-path{display:flex;align-items:center;gap:8px;min-width:0;box-sizing:border-box;padding:9px 11px;border:1px solid var(--ui-border);border-radius:var(--ui-radius-md,8px);background:var(--ui-surface-2);font:inherit;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--muted);margin:10px 0}.runtime-java-path svg{flex:none;width:15px;height:15px}.runtime-manual-controls{display:grid;gap:10px;padding-top:14px}.runtime-manual-controls label{font-size:12px}.runtime-manual-controls .runtime-select{max-width:none}.runtime-inline-actions{display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-top:12px}.game-runtime-options :deep(.memory-field){margin-top:16px}.game-runtime-options :deep(.memory-field>label){font-size:12px}
.runtime-leave-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}.runtime-leave-dialog h2{font-size:16px;margin:0 0 20px}.runtime-dialog-overlay{position:fixed;inset:0;background:#0006;z-index:100}
</style>
