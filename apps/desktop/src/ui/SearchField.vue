<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import AppIconButton from './AppIconButton.vue';
import { nextTick, ref } from 'vue';
defineOptions({ inheritAttrs: false });
const props = defineProps<{ id?: string; modelValue: string; label: string; placeholder?: string; disabled?: boolean; clearLabel?: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: string]; submit: [] }>();
const input = ref<HTMLInputElement>();
async function clear() {
  if (props.disabled) return;
  emit('update:modelValue', '');
  await nextTick(); input.value?.focus();
}
</script>

<template>
  <div
    class="search-field"
    :class="$attrs.class"
  >
    <AppIcon
      name="search"
      class="search-field-icon"
    />
    <input
      :id="id"
      ref="input"
      class="search-field-input"
      type="search"
      data-list-search
      v-bind="{ ...$attrs, class: undefined }"
      :value="modelValue"
      :aria-label="label"
      :placeholder="placeholder"
      :disabled="disabled"
      :data-custom-clear="Boolean(clearLabel)"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      @keydown.enter.prevent="$emit('submit')"
    >
    <AppIconButton
      v-if="clearLabel && modelValue"
      class="search-field-clear"
      icon="x"
      size="compact"
      :label="clearLabel"
      :disabled="disabled"
      @click="clear"
    />
    <slot />
  </div>
</template>

<style scoped>
.search-field{position:relative;display:flex;align-items:center;min-width:0;gap:var(--ui-space-2);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text)}
.search-field:focus-within{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.search-field-icon{width:var(--ui-icon-sm);height:var(--ui-icon-sm);margin-left:var(--ui-space-3);color:var(--ui-text-muted)}
.search-field input{flex:1;box-sizing:border-box;width:100%;min-width:0;min-height:calc(var(--ui-control-height) - 2px);padding:var(--ui-space-1) var(--ui-space-3) var(--ui-space-1) 0;border:0;border-radius:inherit;background:transparent;color:inherit;font:inherit;font-size:var(--ui-text-label);line-height:var(--ui-leading-control)}
.search-field input:focus-visible{outline:none}
.search-field input[data-custom-clear="true"]::-webkit-search-cancel-button{display:none}
.search-field-clear{margin-right:var(--ui-space-1);border:0}
.search-field:has(input:disabled){opacity:.55;cursor:not-allowed}
</style>
