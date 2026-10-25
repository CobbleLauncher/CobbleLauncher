<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { t } from './i18n';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsActions from './ui/settings/SettingsActions.vue';
import SelectField, { type SelectFieldOption } from './ui/SelectField.vue';
import { computed, onMounted, ref, useId, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, LauncherPreferences, LaunchBehavior } from './games';
const props = withDefaults(defineProps<{ game?: Game; preferences?: LauncherPreferences; inlineActions?: boolean }>(), { game: undefined, preferences: undefined, inlineActions: true });
const emit = defineEmits<{ updated: [game: Game]; defaultsUpdated: [preferences: LauncherPreferences]; dirty: [value: boolean] }>();
const id = useId();
const width = ref<number | ''>('');
const height = ref<number | ''>('');
const windowInherit = ref(true);
const jvmInherit = ref(true);
const argumentsText = ref('');
const behavior = ref<LaunchBehavior | 'inherit'>('inherit');
const busy = ref(false);
const ready = ref(false);
const error = ref('');
const status = ref(false);
const savedDraft = ref('');
const inheritCustomOptions = computed<SelectFieldOption[]>(() => [
  { value: 'inherit', label: t('settingsLayout.inherit') },
  { value: 'custom', label: t('settingsLayout.custom') },
]);
const behaviorOptions = computed<SelectFieldOption[]>(() => [
  ...(props.game ? [{ value: 'inherit', label: t('settingsLayout.inherit') }] : []),
  { value: 'keep', label: t('runtime.keep') },
  { value: 'minimize', label: t('runtime.minimize') },
]);
function serialize() { return JSON.stringify({ width: width.value, height: height.value, windowInherit: windowInherit.value, jvmInherit: jvmInherit.value, argumentsText: argumentsText.value, behavior: behavior.value }); }
const dirty = computed(() => !!savedDraft.value && serialize() !== savedDraft.value);
watch(dirty, value => emit('dirty', value));
function cancel() {
  if (!savedDraft.value || busy.value) return;
  const value = JSON.parse(savedDraft.value);
  width.value = value.width; height.value = value.height; windowInherit.value = value.windowInherit; jvmInherit.value = value.jvmInherit; argumentsText.value = value.argumentsText; behavior.value = value.behavior;
  error.value = ''; status.value = false;
}
function setWindowMode(value: string) { if (value === 'inherit' || value === 'custom') windowInherit.value = value === 'inherit'; }
function setJvmMode(value: string) { if (value === 'inherit' || value === 'custom') jvmInherit.value = value === 'inherit'; }
function setBehavior(value: string) { if (value === 'inherit' || value === 'keep' || value === 'minimize') behavior.value = value; }
function hydrate(defaults?: LauncherPreferences) {
  const options = props.game?.runtime_overrides;
  const window = props.game ? options?.window_size : defaults?.window_size;
  width.value = window?.width ?? ''; height.value = window?.height ?? '';
  windowInherit.value = options?.window_size == null;
  jvmInherit.value = options?.jvm_args == null;
  argumentsText.value = (props.game ? options?.jvm_args : defaults?.jvm_args)?.join('\n') ?? '';
  behavior.value = props.game ? options?.launch_behavior ?? 'inherit' : defaults?.launch_behavior ?? 'keep';
  ready.value = true; savedDraft.value = serialize();
}
async function load() {
  if (props.game || props.preferences) { hydrate(props.preferences); return; }
  try { hydrate(await invoke<LauncherPreferences>('get_launcher_preferences')); }
  catch { error.value = t('runtime.loadError'); }
}
async function save() {
  if (busy.value || !ready.value) return;
  error.value = ''; status.value = false;
  const windowSize = props.game && windowInherit.value ? null : { width: width.value === '' ? null : width.value, height: height.value === '' ? null : height.value };
  if (windowSize && ((windowSize.width != null && (!Number.isInteger(windowSize.width) || windowSize.width < 320 || windowSize.width > 16384)) || (windowSize.height != null && (!Number.isInteger(windowSize.height) || windowSize.height < 240 || windowSize.height > 16384)))) { error.value = t('runtime.dimensionsError'); return; }
  const jvmArgs = props.game && jvmInherit.value ? null : argumentsText.value.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
  busy.value = true;
  try {
    if (props.game) emit('updated', await invoke<Game>('set_game_advanced_runtime', { id: props.game.id, windowSize, jvmArgs, launchBehavior: behavior.value === 'inherit' ? null : behavior.value }));
    else emit('defaultsUpdated', await invoke<LauncherPreferences>('set_default_advanced_runtime', { windowSize, jvmArgs, launchBehavior: behavior.value }));
    savedDraft.value = serialize(); status.value = true;
  } catch (cause) { error.value = t('runtime.saveError', { error: String(cause) }); }
  finally { busy.value = false; }
}
watch(() => [props.game?.id, props.game?.runtime_overrides, props.preferences], (value, previous) => { if (!dirty.value || value[0] !== previous[0]) hydrate(props.preferences); });
onMounted(() => { void load(); });

defineExpose({ save, cancel, busy });
</script>
<template>
  <SettingsDisclosure
    class="advanced-runtime-settings"
    icon="sliders-horizontal"
    bordered
    :title="t('runtime.advanced')"
  >
    <form
      class="advanced-runtime-form"
      @submit.prevent="inlineActions && save()"
    >
      <SettingsRow
        :label="t('runtime.window')"
        :input-id="game ? `${id}-window-mode` : undefined"
      >
        <SelectField
          v-if="game"
          :id="`${id}-window-mode`"
          :model-value="windowInherit ? 'inherit' : 'custom'"
          :options="inheritCustomOptions"
          :label="t('runtime.window')"
          :disabled="busy || !ready"
          @update:model-value="setWindowMode"
        />
        <div
          v-if="!game || !windowInherit"
          class="runtime-window-size"
        >
          <label :for="`${id}-width`">{{ t('runtime.width') }}<input
            :id="`${id}-width`"
            v-model.number="width"
            type="number"
            min="320"
            max="16384"
            :placeholder="t('runtime.gameDefault')"
            :disabled="busy || !ready"
          ></label>
          <label :for="`${id}-height`">{{ t('runtime.height') }}<input
            :id="`${id}-height`"
            v-model.number="height"
            type="number"
            min="240"
            max="16384"
            :placeholder="t('runtime.gameDefault')"
            :disabled="busy || !ready"
          ></label>
        </div>
      </SettingsRow>
      <SettingsRow
        :label="t('runtime.afterLaunch')"
        :input-id="`${id}-behavior`"
      >
        <SelectField
          :id="`${id}-behavior`"
          :model-value="behavior"
          :options="behaviorOptions"
          :label="t('runtime.afterLaunch')"
          :disabled="busy || !ready"
          @update:model-value="setBehavior"
        />
      </SettingsRow>
      <SettingsRow
        :label="t('runtime.extraJvm')"
        :input-id="game ? `${id}-jvm-mode` : `${id}-jvm`"
      >
        <SelectField
          v-if="game"
          :id="`${id}-jvm-mode`"
          :model-value="jvmInherit ? 'inherit' : 'custom'"
          :options="inheritCustomOptions"
          :label="t('runtime.extraJvm')"
          :disabled="busy || !ready"
          @update:model-value="setJvmMode"
        />
        <template v-if="!game || !jvmInherit">
          <textarea
            :id="`${id}-jvm`"
            v-model="argumentsText"
            rows="4"
            :aria-label="t('runtime.extraJvm')"
            :disabled="busy || !ready"
            :aria-describedby="`${id}-jvm-help`"
            spellcheck="false"
          /><p
            :id="`${id}-jvm-help`"
            class="setting-hint"
          >
            {{ t('runtime.jvmHelp') }}
          </p>
        </template>
      </SettingsRow>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <button
        v-if="!ready"
        type="button"
        class="text-button"
        @click="load"
      >
        {{ t('runtime.retry') }}
      </button>
      <SettingsActions
        v-if="inlineActions"
        :dirty="dirty"
        :busy="busy"
        :disabled="!ready"
        @apply="save"
        @cancel="cancel"
      />
      <p
        v-if="status"
        role="status"
        class="setting-hint"
      >
        {{ t('runtime.saved') }}
      </p>
    </form>
  </SettingsDisclosure>
</template>
<style scoped>
.advanced-runtime-settings{margin-top:24px}.advanced-runtime-form{padding-top:8px}.runtime-window-size{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-top:10px}.runtime-window-size label{display:grid;gap:8px;font-size:12px}textarea{margin-top:10px;resize:vertical;font:inherit}
</style>
