<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from '../ui/AppIcon.vue';
import { t } from '../i18n';
import { onUnmounted, ref, watch } from 'vue';
const props = defineProps<{ dirty: boolean; busy?: boolean; error?: string; savedRevision?: number }>();
defineEmits<{ save: []; discard: [] }>();
const acknowledged = ref(false);
let timer: ReturnType<typeof setTimeout> | undefined;
function dismiss() { clearTimeout(timer); acknowledged.value = false; }
watch(() => props.savedRevision, (value, previous) => {
  dismiss();
  if (!value || value === previous || props.dirty || props.error) return;
  acknowledged.value = true; timer = setTimeout(dismiss, 2400);
});
watch(() => props.dirty, dirty => { if (dirty) dismiss(); });
onUnmounted(dismiss);
</script>
<template>
  <div
    v-if="dirty || acknowledged"
    class="settings-save-bar"
    role="region"
    :aria-label="t(acknowledged && !dirty ? 'common.saved' : 'settingsLayout.editor.unsaved')"
  >
    <p :role="error ? 'alert' : 'status'">
      <AppIcon
        v-if="acknowledged && !dirty"
        name="check"
        class="save-success-icon"
      />
      {{ error || t(acknowledged && !dirty ? 'common.saved' : 'settingsLayout.editor.unsaved') }}
    </p>
    <div
      v-if="dirty"
      class="settings-save-bar-actions"
    >
      <button
        type="button"
        class="text-button"
        :disabled="busy"
        @click="$emit('discard')"
      >
        {{ t('settingsLayout.discard') }}
      </button>
      <button
        type="button"
        class="primary-button"
        :disabled="busy"
        @click="$emit('save')"
      >
        <AppIcon
          v-if="busy"
          name="loader-circle"
          class="ui-busy-icon"
        />{{ busy ? t('settingsLayout.applying') : t('settingsLayout.editor.save') }}
      </button>
    </div>
  </div>
</template>
<style scoped>
.settings-save-bar{min-height:66px}.settings-save-bar p{display:flex;align-items:center;gap:8px}.save-success-icon{color:var(--ui-success)}
.settings-save-bar{position:sticky;bottom:var(--ui-space-4);z-index:20;display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-4);margin-top:var(--ui-space-5);padding:var(--ui-space-3) var(--ui-space-4);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1);box-shadow:var(--ui-shadow-menu)}.settings-save-bar p{margin:0;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);overflow-wrap:anywhere}.settings-save-bar p[role=alert]{color:var(--ui-danger)}.settings-save-bar-actions{display:flex;align-items:center;gap:var(--ui-space-3);flex-shrink:0}.settings-save-bar button{display:inline-flex;align-items:center;justify-content:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font:inherit;font-size:var(--ui-text-label)}
@media(max-width:600px){.settings-save-bar{flex-wrap:wrap;gap:var(--ui-space-2);padding:var(--ui-space-3)}.settings-save-bar-actions{margin-left:auto}}
</style>
