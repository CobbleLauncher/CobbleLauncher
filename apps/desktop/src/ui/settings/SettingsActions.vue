<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from '../../i18n';
import AppIcon from '../AppIcon.vue';
defineProps<{ dirty: boolean; busy?: boolean; disabled?: boolean }>();
defineEmits<{ apply: []; cancel: [] }>();
</script>
<template>
  <div
    v-if="dirty"
    class="settings-actions"
  >
    <button
      type="button"
      class="secondary-button"
      :disabled="disabled || busy"
      @click="$emit('apply')"
    >
      <AppIcon
        v-if="busy"
        name="loader-circle"
        class="ui-busy-icon"
      />
      {{ busy ? t('settingsLayout.applying') : t('settingsLayout.apply') }}
    </button>
    <button
      type="button"
      class="text-button"
      :disabled="busy"
      @click="$emit('cancel')"
    >
      {{ t('settingsLayout.cancel') }}
    </button>
  </div>
</template>
<style scoped>
.settings-actions{display:flex;align-items:center;gap:var(--ui-space-3,12px);flex-wrap:wrap;margin-top:var(--ui-space-4,16px)}
.settings-actions>.secondary-button{min-height:var(--ui-control-height,40px);font-family:inherit;font-size:var(--ui-text-label,13px)}
.settings-actions>.text-button{min-height:var(--ui-control-compact,32px);font-family:inherit;font-size:var(--ui-text-label,13px)}
</style>
