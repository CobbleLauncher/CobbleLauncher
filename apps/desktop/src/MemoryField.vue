<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
defineProps<{ modelValue: number; label: string; id: string; disabled: boolean }>();
defineEmits<{ 'update:modelValue': [value: number] }>();
</script>
<template>
  <div class="form-field memory-field">
    <label :for="id">{{ label }}</label>
    <div
      class="memory-presets"
      role="group"
      :aria-label="`${label}预设`"
    >
      <button
        v-for="amount in [2048, 4096, 6144, 8192]"
        :key="amount"
        class="memory-preset"
        type="button"
        :aria-pressed="modelValue === amount"
        :disabled="disabled"
        @click="$emit('update:modelValue', amount)"
      >
        {{ amount / 1024 }} GiB
      </button>
    </div>
    <div class="memory-number">
      <input
        :id="id"
        type="number"
        :value="modelValue"
        min="512"
        max="262144"
        step="1"
        inputmode="numeric"
        :disabled="disabled"
        @input="$emit('update:modelValue', Number(($event.target as HTMLInputElement).value))"
      ><span>MiB</span>
    </div>
  </div>
</template>
