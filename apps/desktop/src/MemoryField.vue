<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import NumericSlider from './ui/NumericSlider.vue';
import { t } from './i18n';
defineProps<{ modelValue: number; label: string; id: string; disabled: boolean }>();
defineEmits<{ 'update:modelValue': [value: number] }>();
// Preserve familiar allocation stops while numeric entry remains exact in MiB.
const amounts = [512, 1024, 2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768, 49152, 65536, 98304, 131072, 196608, 262144];
</script>
<template>
  <NumericSlider
    :id="id"
    class="memory-field"
    :model-value="modelValue"
    :label="t('memoryControl.adjust', {label})"
    :value-label="label"
    :min="512"
    :max="262144"
    :step="1"
    :stops="amounts"
    :disabled="disabled"
    unit="MiB"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <template #markers>
      <div
        class="memory-limits"
        aria-hidden="true"
      >
        <span>512 MiB</span><span>256 GiB</span>
      </div>
    </template>
  </NumericSlider>
</template>
<style scoped>
.memory-field{--numeric-slider-value-width:132px;min-width:0}
.memory-limits{display:flex;justify-content:space-between;gap:var(--ui-space-2);color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
@media(max-width:420px){.memory-field{--numeric-slider-value-width:120px}}
</style>
