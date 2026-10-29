<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import AppIcon from './AppIcon.vue';
defineOptions({ inheritAttrs: false });
const props = withDefaults(defineProps<{
  id: string;
  modelValue: string;
  label: string;
  hint?: string;
  validator: (value: string) => string | null;
  disabled?: boolean;
  limit?: number;
  type?: 'text' | 'url';
}>(), { hint: '', disabled: false, type: 'text', limit: undefined });
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const input = ref<HTMLInputElement>();
const touched = ref(false);
const attempted = ref(false);
const composing = ref(false);
const error = computed(() => (touched.value || attempted.value) && !composing.value ? props.validator(props.modelValue) : null);
const showCount = computed(() => props.limit !== undefined && props.modelValue.length >= props.limit - 4);
let animation: Animation | undefined;
function blur() { if (props.modelValue.length) touched.value = true; }
function focus(select = false) { input.value?.focus(); if (select) input.value?.select(); }
function validate() {
  if (props.disabled || composing.value) return false;
  attempted.value = true;
  if (!props.validator(props.modelValue)) return true;
  focus();
  animation?.cancel();
  if (!input.value?.closest('[data-motion="reduced"]') && !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
    animation = input.value?.animate?.([
      { transform: 'translateX(0)' }, { transform: 'translateX(-4px)' },
      { transform: 'translateX(4px)' }, { transform: 'translateX(-2px)' }, { transform: 'translateX(0)' },
    ], { duration: 220, easing: 'ease-out' });
  }
  return false;
}
onBeforeUnmount(() => animation?.cancel());
defineExpose({ validate, focus });
</script>
<template>
  <div class="validated-field">
    <div class="validated-field-heading">
      <label :for="id">{{ label }}</label>
      <span
        v-if="showCount"
        class="validated-field-count"
        aria-hidden="true"
      >{{ modelValue.length }} / {{ limit }}</span>
    </div>
    <div class="validated-field-control">
      <input
        :id="id"
        ref="input"
        class="validated-field-input"
        v-bind="$attrs"
        :value="modelValue"
        :type="type"
        :disabled="disabled"
        :aria-invalid="!!error"
        :aria-describedby="hint || error ? `${id}-feedback` : undefined"
        autocomplete="off"
        spellcheck="false"
        @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
        @blur="blur"
        @compositionstart="composing = true"
        @compositionend="composing = false"
      >
      <AppIcon
        v-if="error"
        name="alert-circle"
        class="validated-field-icon"
      />
    </div>
    <p
      v-if="hint || error"
      :id="`${id}-feedback`"
      class="validated-field-feedback"
      :class="{ 'has-error': error }"
      aria-live="polite"
      aria-atomic="true"
    >
      {{ error || hint }}
    </p>
  </div>
</template>
<style scoped>
.validated-field{display:grid;gap:8px;min-width:0}
.validated-field-heading{display:flex;align-items:baseline;justify-content:space-between;gap:12px}
.validated-field-heading label{font-size:var(--ui-text-label,14px);font-weight:500;line-height:var(--ui-leading-label,20px)}
.validated-field-count{font-size:var(--ui-text-caption,12px);color:var(--ui-text-muted);font-variant-numeric:tabular-nums;white-space:nowrap}
.validated-field-control{position:relative;min-width:0}
.validated-field-control input{display:block;width:100%;min-height:var(--ui-control-height,40px);font:inherit;font-size:var(--ui-text-body,14px);line-height:var(--ui-leading-label,1.43);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md,8px);background:var(--ui-surface);color:var(--ui-text);padding:4px 36px 4px 12px;transition:border-color 120ms ease,box-shadow 120ms ease}
.validated-field-control input:hover:not(:disabled):not([aria-invalid=true]){border-color:var(--ui-border-strong)}
.validated-field-control input:focus-visible{outline:2px solid var(--ui-focus);outline-offset:2px}
.validated-field-control input[aria-invalid=true]{border-color:var(--ui-danger);background:color-mix(in srgb,var(--ui-danger) 4%,var(--ui-surface))}
.validated-field-control input[aria-invalid=true]:focus-visible{outline-color:var(--ui-danger)}
.validated-field-icon{position:absolute;right:12px;top:50%;transform:translateY(-50%);color:var(--ui-danger);pointer-events:none}
.validated-field-feedback{margin:0;min-height:18px;font-size:var(--ui-text-caption,12px);line-height:var(--ui-leading-caption,18px);color:var(--ui-text-muted);overflow-wrap:anywhere}
.validated-field-feedback.has-error{color:var(--ui-danger)}
</style>
