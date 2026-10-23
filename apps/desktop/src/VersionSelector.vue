<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import { computed, ref, watch } from 'vue';
import { ComboboxRoot, ComboboxAnchor, ComboboxInput, ComboboxTrigger, ComboboxContent, ComboboxPortal, ComboboxViewport, ComboboxItem, ComboboxEmpty } from 'reka-ui';
import AppIcon from './ui/AppIcon.vue';
import { visibleVersions, type VersionOption } from './version-selector';
const props = defineProps<{ id: string; modelValue: string; options: VersionOption[]; disabled?: boolean; currentValue?: string; placeholder?: string }>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const query = ref('');
const limit = ref(5);
const open = ref(false);
const visible = computed(() => visibleVersions(props.options, query.value, limit.value, [props.modelValue, props.currentValue ?? '']));
watch(query, () => { limit.value = 5; });
watch(() => JSON.stringify(props.options), () => { query.value = ''; limit.value = 5; });
watch(open, () => { query.value = ''; limit.value = 5; });
function select(value: unknown) {
  if (typeof value === 'string') emit('update:modelValue', value);
}
</script>
<template>
  <ComboboxRoot
    v-model:open="open"
    :model-value="modelValue"
    :disabled="disabled"
    :ignore-filter="true"
    :open-on-click="true"
    class="version-selector"
    @update:model-value="select"
  >
    <ComboboxAnchor class="version-selector-anchor">
      <ComboboxInput
        :id="id"
        :display-value="(value: unknown) => options.find(option => option.value === value)?.label ?? ''"
        :placeholder="placeholder ?? t('versions.search')"
        @input="query = ($event.target as HTMLInputElement).value"
      />
      <ComboboxTrigger
        type="button"
        :aria-label="t('versions.expand')"
        class="version-selector-trigger"
      >
        <AppIcon name="chevron-down" />
      </ComboboxTrigger>
    </ComboboxAnchor>
    <ComboboxPortal>
      <ComboboxContent
        class="version-selector-content"
        position="popper"
        align="start"
        :side-offset="5"
        :collision-padding="10"
      >
        <ComboboxViewport class="version-selector-viewport">
          <ComboboxEmpty class="version-selector-empty">
            {{ t('versions.empty') }}
          </ComboboxEmpty>
          <ComboboxItem
            v-for="option in visible.options"
            :key="option.value"
            :value="option.value"
            class="version-selector-option"
          >
            {{ option.label }}{{ option.value === currentValue ? t('versions.current') : '' }}
          </ComboboxItem>
        </ComboboxViewport>
        <button
          v-if="visible.more"
          type="button"
          class="text-button version-selector-more"
          @click="limit += 20"
        >
          {{ t('versions.more') }}
        </button>
      </ComboboxContent>
    </ComboboxPortal>
  </ComboboxRoot>
</template>
<!-- Reka forwards these roots without Vue scope attributes; every selector is component-prefixed. -->
<style>
.version-selector { position: relative; width: 100%; min-width: 0; }
.version-selector-anchor { display: flex; align-items: center; position: relative; }
.version-selector-anchor input { width: 100%; min-width: 0; box-sizing: border-box; padding-right: 36px; }
.version-selector-trigger svg { width: 16px; height: 16px; }
.version-selector-trigger { position: absolute; right: 4px; background: transparent; border: 0; color: inherit; padding: 8px; cursor: pointer; }
.version-selector-content { z-index: 120; width: var(--reka-combobox-trigger-width); max-width: var(--reka-combobox-content-available-width); max-height: min(320px, var(--reka-combobox-content-available-height)); display: flex; flex-direction: column; background: var(--panel); color: var(--ink); border: 1px solid var(--line); border-radius: 10px; box-shadow: 0 8px 24px #0002; overflow: hidden; }
.version-selector-viewport { min-height: 0; max-height: 260px; overflow-y: auto; }
.version-selector-option { padding: 10px 12px; cursor: pointer; overflow-wrap: anywhere; }
.version-selector-option[data-highlighted] { background: var(--hover); outline: none; }
.version-selector-option[data-state="checked"] { font-weight: 600; }
.version-selector-empty { padding: 12px; }
.version-selector-more { width: 100%; flex: none; padding: 10px; }
</style>
