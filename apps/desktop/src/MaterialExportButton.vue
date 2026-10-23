<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SelectField from './ui/SelectField.vue';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';

const props = defineProps<{ resourceId: string; disabled?: boolean }>();
const busy = ref(false);
const format = ref<'csv' | 'text'>('csv');
const emit = defineEmits<{ error: [message: string] }>();
async function exportMaterials() {
  if (busy.value || props.disabled) return;
  busy.value = true;
  try {
    await invoke<boolean>('export_blueprint_materials', { resourceId: props.resourceId, format: format.value });
  } catch (cause) {
    emit('error', String(cause));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <SelectField
    :id="`material-export-${resourceId}`"
    v-model="format"
    :options="[{ value: 'csv', label: 'CSV' }, { value: 'text', label: t('blueprints.materialExportText') }]"
    :label="t('blueprints.materialExportFormat')"
    :disabled="busy || disabled"
  />
  <button
    data-material-export
    class="text-button"
    :disabled="busy || disabled"
    @click="exportMaterials"
  >
    {{ t('blueprints.exportAllMaterials') }}
  </button>
</template>
