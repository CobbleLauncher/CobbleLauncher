<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { t } from './i18n';
import { launcherUpdateErrorKey } from './launcher-updates';
import { isTauri } from '@tauri-apps/api/core';
import { launcherUpdateStatus, checkLauncherUpdate, saveLauncherUpdatePreferences, installLauncherUpdate, type LauncherUpdateStatus } from './launcher-updates';
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SwitchField from './ui/SwitchField.vue';
import SelectField from './ui/SelectField.vue';
const status = ref<LauncherUpdateStatus | null>(null);
const busy = ref(false);
const message = ref('');
async function run(action: () => Promise<LauncherUpdateStatus | void>) {
  busy.value = true; message.value = '';
  try { const result = await action(); if (result) status.value = result; }
  catch (error) { message.value = t(launcherUpdateErrorKey(error)); }
  finally { busy.value = false; }
}
function save(backgroundCheck: boolean, channel: string) { return run(() => saveLauncherUpdatePreferences(backgroundCheck, channel)); }
async function check() {
  await run(async () => { const result = await checkLauncherUpdate(); message.value = result.available_version ? '' : t('launcherUpdates.latest'); return result; });
}
const confirming = ref(false);
onMounted(() => { if (isTauri()) void run(launcherUpdateStatus); });
</script>
<template>
  <SettingsSection :title="t('launcherUpdates.title')">
    <template v-if="status">
      <SettingsRow :label="t('launcherUpdates.version')">
        <span>{{ status.version }}</span>
      </SettingsRow>
      <SettingsRow :label="t('launcherUpdates.background')">
        <SwitchField
          :model-value="status.background_check"
          :label="t('launcherUpdates.background')"
          :disabled="busy"
          @update:model-value="save($event, status!.channel)"
        />
      </SettingsRow>
      <SettingsRow :label="t('launcherUpdates.channel')">
        <SelectField
          id="launcher-update-channel"
          :model-value="status.channel"
          :label="t('launcherUpdates.channel')"
          :disabled="busy"
          :options="[{ value: 'stable', label: t('launcherUpdates.stable') }, { value: 'preview', label: t('launcherUpdates.preview') }]"
          @update:model-value="save(status!.background_check, $event)"
        />
      </SettingsRow>
      <p v-if="!status.configured">
        {{ t('launcherUpdates.unconfigured') }}
      </p>
      <p v-if="status.available_version">
        {{ t('launcherUpdates.available') }} {{ status.available_version }}
      </p>
      <p v-if="status.available_version && !status.can_install">
        {{ t('launcherUpdates.portable') }}
      </p>
      <button
        class="secondary-button"
        type="button"
        :disabled="busy || !status.configured"
        @click="check"
      >
        {{ busy ? t('launcherUpdates.busy') : t('launcherUpdates.check') }}
      </button>
      <button
        v-if="status.available_version && status.can_install && !confirming"
        class="primary-button"
        type="button"
        :disabled="busy"
        @click="confirming = true"
      >
        {{ t('launcherUpdates.install') }}
      </button>
      <div v-if="confirming">
        <p>{{ t('launcherUpdates.confirmText') }}</p>
        <button
          class="primary-button"
          type="button"
          :disabled="busy"
          @click="run(installLauncherUpdate)"
        >
          {{ t('launcherUpdates.confirm') }}
        </button>
        <button
          class="secondary-button"
          type="button"
          :disabled="busy"
          @click="confirming = false"
        >
          {{ t('launcherUpdates.cancel') }}
        </button>
      </div>
    </template>
    <p v-else-if="!message">
      {{ t('launcherUpdates.desktop') }}
    </p>
    <p
      v-if="message"
      role="status"
    >
      {{ message }}
    </p>
  </SettingsSection>
</template>
<style scoped>
p{font-size:var(--ui-text-small,13px);color:var(--ui-text-muted)}button{margin:8px 8px 0 0}
</style>
