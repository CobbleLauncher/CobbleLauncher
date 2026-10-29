<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref } from 'vue';
import { t } from '../i18n';
import { validateDownloadSettings } from '../download-settings';
import ValidatedTextField from '../ui/ValidatedTextField.vue';
const props = defineProps<{ modelValue: string; disabled: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: string]; use: [url: string]; cancel: [] }>();
const field = ref<InstanceType<typeof ValidatedTextField>>();
function validate(value: string) {
  return validateDownloadSettings({ source: 'custom', mirror: value }) ? t('settingsLayout.customMirrorInvalid') : null;
}
function submit() {
  if (props.disabled || !field.value?.validate()) return false;
  emit('use', props.modelValue.trim());
  return true;
}
defineExpose({ submit });
</script>
<template>
  <form
    class="settings-mirror-draft"
    novalidate
    @submit.prevent="submit"
  >
    <ValidatedTextField
      id="mirror-url"
      ref="field"
      :model-value="modelValue"
      type="url"
      :label="t('settingsLayout.mirror')"
      :hint="t('settingsLayout.mirrorHelp')"
      :placeholder="t('settingsLayout.mirrorPlaceholder')"
      :validator="validate"
      :disabled="disabled"
      @update:model-value="emit('update:modelValue', $event)"
    />
    <div class="mirror-actions">
      <button
        type="submit"
        class="secondary-button"
        :disabled="disabled"
      >
        {{ t('settingsLayout.useMirror') }}
      </button>
      <button
        type="button"
        class="text-button"
        :disabled="disabled"
        @click="emit('cancel')"
      >
        {{ t('settingsLayout.cancel') }}
      </button>
    </div>
  </form>
</template>
<style scoped>
.settings-mirror-draft{display:grid;gap:12px;margin-top:12px}
.mirror-actions{display:flex;gap:12px;align-items:center;flex-wrap:wrap}
</style>
