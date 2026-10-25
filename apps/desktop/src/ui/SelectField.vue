<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import {
  SelectRoot, SelectTrigger, SelectValue, SelectIcon, SelectPortal, SelectContent,
  SelectViewport, SelectGroup, SelectLabel, SelectItem, SelectItemText, SelectItemIndicator,
  SelectScrollUpButton, SelectScrollDownButton,
} from 'reka-ui';
import AppIcon from './AppIcon.vue';

export type SelectFieldOption = { value: string; label: string; disabled?: boolean; group?: string };
const props = defineProps<{
  id: string;
  modelValue: string;
  options: SelectFieldOption[];
  label: string;
  disabled?: boolean;
  placeholder?: string;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
// Reka reserves the empty string for clearing. Keep the public field contract
// while assigning an internal value that cannot collide with a caller's options.
const emptyValue = computed(() => {
  let value = '__cobble_empty_selection__';
  while (props.options.some(option => option.value === value)) value += '_';
  return value;
});
const internalValue = computed(() => props.modelValue === '' && props.options.some(option => option.value === '') ? emptyValue.value : props.modelValue);
const groups = computed(() => {
  const result: { label?: string; options: SelectFieldOption[] }[] = [];
  for (const option of props.options) {
    const last = result.at(-1);
    if (last && last.label === option.group) last.options.push(option);
    else result.push({ label: option.group, options: [option] });
  }
  return result;
});
const selectedLabel = computed(() => props.options.find(option => option.value === props.modelValue)?.label ?? props.placeholder ?? '');
function update(value: unknown) {
  if (typeof value === 'string') emit('update:modelValue', value === emptyValue.value ? '' : value);
}
</script>

<template>
  <SelectRoot
    :model-value="internalValue"
    :disabled="disabled"
    @update:model-value="update"
  >
    <SelectTrigger
      :id="id"
      class="select-field-trigger"
      :aria-label="label"
    >
      <SelectValue :placeholder="placeholder">
        {{ selectedLabel }}
      </SelectValue>
      <SelectIcon as-child>
        <AppIcon name="chevron-down" />
      </SelectIcon>
    </SelectTrigger>
    <SelectPortal>
      <SelectContent
        class="select-field-content"
        position="popper"
        align="start"
        :side-offset="4"
        :collision-padding="8"
      >
        <SelectScrollUpButton class="select-field-scroll">
          <AppIcon
            name="chevron-down"
            class="select-field-scroll-up"
          />
        </SelectScrollUpButton>
        <SelectViewport class="select-field-viewport">
          <template
            v-for="(group, index) in groups"
            :key="`${group.label ?? 'options'}-${index}`"
          >
            <SelectGroup>
              <SelectLabel
                v-if="group.label"
                class="select-field-label"
              >
                {{ group.label }}
              </SelectLabel>
              <SelectItem
                v-for="option in group.options"
                :key="option.value"
                :value="option.value === '' ? emptyValue : option.value"
                :disabled="option.disabled"
                class="select-field-item"
              >
                <SelectItemText>{{ option.label }}</SelectItemText>
                <SelectItemIndicator class="select-field-indicator">
                  <AppIcon name="check" />
                </SelectItemIndicator>
              </SelectItem>
            </SelectGroup>
          </template>
        </SelectViewport>
        <SelectScrollDownButton class="select-field-scroll">
          <AppIcon name="chevron-down" />
        </SelectScrollDownButton>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style>
.select-field-trigger{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);width:100%;min-width:0;min-height:var(--ui-control-height);padding:max(var(--ui-space-1),calc((var(--ui-control-height) - var(--ui-text-label) * var(--ui-leading-control) - 2px) / 2)) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label);line-height:var(--ui-leading-control);text-align:left;cursor:pointer;transition:border-color var(--ui-duration-fast),background-color var(--ui-duration-fast)}
.select-field-trigger:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.select-field-trigger:disabled{opacity:.55;cursor:not-allowed}
.select-field-trigger>svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm);flex:none;color:var(--muted)}
.select-field-content{z-index:140;min-width:var(--reka-select-trigger-width);max-width:min(480px,var(--reka-select-content-available-width));max-height:min(320px,var(--reka-select-content-available-height));overflow:hidden;border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-menu)}
.select-field-viewport{max-height:min(320px,var(--reka-select-content-available-height));padding:var(--ui-space-1);overflow-y:auto}
.select-field-label{padding:var(--ui-space-2) var(--ui-space-2) var(--ui-space-1);color:var(--muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.select-field-item{position:relative;display:flex;align-items:center;min-height:var(--ui-control-height);padding:var(--ui-space-1) var(--ui-space-6) var(--ui-space-1) var(--ui-space-2);border-radius:var(--ui-radius-sm);outline:0;font-size:var(--ui-text-label);line-height:var(--ui-leading-control);overflow-wrap:anywhere;cursor:pointer}
.select-field-item[data-highlighted]{background:var(--hover)}
.select-field-item[data-disabled]{opacity:.5;cursor:not-allowed}
.select-field-indicator{position:absolute;right:var(--ui-space-2);display:flex;align-items:center}.select-field-indicator svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm)}
.select-field-scroll{display:grid;place-items:center;width:100%;height:var(--ui-space-5);border:0;background:var(--ui-surface-1);color:var(--muted)}
.select-field-scroll svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm)}.select-field-scroll-up svg{transform:rotate(180deg)}
</style>
