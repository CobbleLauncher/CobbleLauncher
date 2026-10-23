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
  if (typeof value === 'string') emit('update:modelValue', value);
}
</script>

<template>
  <SelectRoot
    :model-value="modelValue"
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
                :value="option.value"
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
.select-field-trigger{display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;min-width:0;min-height:40px;padding:8px 12px;border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md,8px);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:14px;line-height:1.4;text-align:left;cursor:pointer}
.select-field-trigger:focus-visible{outline:var(--ui-focus-ring,2px solid var(--ui-focus));outline-offset:var(--ui-focus-offset,3px)}
.select-field-trigger:disabled{opacity:.55;cursor:not-allowed}
.select-field-trigger>svg{width:16px;height:16px;flex:none;color:var(--muted)}
.select-field-content{z-index:140;min-width:var(--reka-select-trigger-width);max-width:min(480px,var(--reka-select-content-available-width));max-height:min(320px,var(--reka-select-content-available-height));overflow:hidden;border:1px solid var(--ui-border);border-radius:var(--ui-radius-md,8px);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-menu,0 8px 24px #0002)}
.select-field-viewport{max-height:min(320px,var(--reka-select-content-available-height));padding:4px;overflow-y:auto}
.select-field-label{padding:7px 9px 4px;color:var(--muted);font-size:12px;line-height:1.5}
.select-field-item{position:relative;display:flex;align-items:center;min-height:36px;padding:6px 34px 6px 9px;border-radius:5px;outline:0;font-size:14px;line-height:1.45;overflow-wrap:anywhere;cursor:pointer}
.select-field-item[data-highlighted]{background:var(--hover)}
.select-field-item[data-disabled]{opacity:.5;cursor:not-allowed}
.select-field-indicator{position:absolute;right:9px;display:flex;align-items:center}.select-field-indicator svg{width:16px;height:16px}
.select-field-scroll{display:grid;place-items:center;width:100%;height:24px;border:0;background:var(--ui-surface-1);color:var(--muted)}
.select-field-scroll svg{width:15px;height:15px}.select-field-scroll-up svg{transform:rotate(180deg)}
</style>
