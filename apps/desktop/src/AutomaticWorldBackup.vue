<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import SwitchField from './ui/SwitchField.vue';
const props = defineProps<{ gameId: string; revision?: number }>();
type Preference = 'undecided' | 'enabled' | 'disabled';
type Status = { preference: Preference; last_backup: string | null; last_error: string | null; world_backups?: Record<string, string> };
const emit = defineEmits<{ recent: [value: Record<string, string>] }>();
const status = ref<Status | null>(null);
const busy = ref(false);
const error = ref('');
let request = 0;
const recent = computed(() => {
  if (!status.value?.last_backup) return '';
  const date = new Date(status.value.last_backup);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString(locale.value);
});
async function load() {
  const current = ++request;
  try {
    const value = await invoke<Status>('get_automatic_world_backup', { gameId: props.gameId });
    if (current === request) { status.value = value; emit('recent', value.world_backups ?? {}); error.value = ''; }
  } catch (cause) { if (current === request) error.value = String(cause); }
}
async function choose(preference: Preference) {
  if (busy.value) return;
  const current = request;
  busy.value = true; error.value = '';
  try {
    const value = await invoke<Status>('set_automatic_world_backup', { gameId: props.gameId, preference });
    if (current === request) { status.value = value; emit('recent', value.world_backups ?? {}); }
  } catch (cause) { if (current === request) error.value = String(cause); }
  finally { busy.value = false; }
}
watch(() => [props.gameId, props.revision], () => { status.value = null; void load(); }, { immediate: true });
</script>
<template>
  <div class="automatic-world-backup">
    <template v-if="status?.preference === 'undecided'">
      <p>{{ t('worlds.autoPrompt') }}</p>
      <button
        class="secondary-button"
        :disabled="busy"
        @click="choose('enabled')"
      >
        {{ t('worlds.autoEnable') }}
      </button>
      <button
        class="text-button"
        :disabled="busy"
        @click="choose('disabled')"
      >
        {{ t('worlds.autoDecline') }}
      </button>
    </template>
    <template v-else-if="status">
      <label>{{ t('worlds.autoTitle') }}
        <SwitchField
          :model-value="status.preference === 'enabled'"
          :label="t('worlds.autoTitle')"
          :disabled="busy"
          @update:model-value="choose($event ? 'enabled' : 'disabled')"
        />
      </label>
      <p
        v-if="recent"
        class="setting-hint"
      >
        {{ t('worlds.autoRecent') }} · {{ recent }}
      </p>
      <p
        v-if="status.last_error"
        class="inline-error"
        role="status"
      >
        {{ t('worlds.autoFailed') }}: {{ status.last_error }}
      </p>
    </template>
    <p
      v-if="error"
      class="inline-error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
<style scoped>
.automatic-world-backup { margin-block: 12px; }
.automatic-world-backup p { margin-block: 6px; }
.automatic-world-backup label { display: flex; align-items: center; gap: 12px; }

</style>
