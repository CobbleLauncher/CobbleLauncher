<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import AppIconButton from './AppIconButton.vue';
import { computed, nextTick, ref } from 'vue';
import { t } from '../i18n';
defineOptions({ inheritAttrs: false });
const props = defineProps<{ id?: string; modelValue: string; label: string; placeholder?: string; disabled?: boolean; clearLabel?: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: string]; submit: [] }>();
const input = ref<HTMLInputElement>();
const clearLabel = computed(() => props.clearLabel ?? t('common.clearSearch'));
const composing = ref(false);
function changed(event: Event) { if (!composing.value) emit('update:modelValue', (event.target as HTMLInputElement).value); }
function composed(event: CompositionEvent) { composing.value = false; changed(event); }
let returnFocus: HTMLElement | null = null;
function focused(event: FocusEvent) {
  if (event.relatedTarget instanceof HTMLElement && !event.relatedTarget.closest('.search-field')) returnFocus = event.relatedTarget;
}
function keydown(event: KeyboardEvent) {
  if (props.disabled || event.defaultPrevented || event.isComposing || composing.value || event.keyCode === 229) return;
  if (event.key === 'Enter') { event.preventDefault(); emit('submit'); }
  if (event.key !== 'Escape') return;
  // Menus and dialogs own Escape before their nested search field does.
  if (input.value?.closest('[role="dialog"], [role="menu"], [role="listbox"], [data-reka-popper-content-wrapper]')) return;
  event.preventDefault(); event.stopPropagation();
  if (props.modelValue) void clear();
  else {
    input.value?.blur();
    if (returnFocus?.isConnected && !returnFocus.matches(':disabled')) returnFocus.focus({ preventScroll: true });
  }
}
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
      data-custom-clear="true"
      @input="changed"
      @keydown="keydown"
      @focus="focused"
      @compositionstart="composing = true"
      @compositionend="composed"
    >
    <AppIconButton
      v-if="modelValue"
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
