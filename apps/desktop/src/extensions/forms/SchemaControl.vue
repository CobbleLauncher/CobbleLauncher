<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, inject } from 'vue';
import type { ControlElement } from '@jsonforms/core';
import { rendererProps, useJsonFormsControl } from '@jsonforms/vue';
import SettingsRow from '../../ui/settings/SettingsRow.vue';
const props = defineProps(rendererProps<ControlElement>());
const { control, handleChange } = useJsonFormsControl(props);
const translate = inject<(key: string, fallback?: string) => string | undefined>('schemaTranslate', (_key, fallback) => fallback);
const options = computed(() => control.value.schema.enum?.map(value => ({
  value, label: translate(`${control.value.path}.${String(value)}`, String(value)),
})) ?? []);
const label = computed(() => control.value.label || control.value.schema.title || control.value.path);
function update(event: Event) {
  const input = event.target as HTMLInputElement;
  const schema = control.value.schema;
  const value = options.value.length ? JSON.parse(input.value)
    : schema.type === 'boolean' ? input.checked
    : schema.type === 'number' || schema.type === 'integer' ? (input.value === '' ? undefined : input.valueAsNumber)
    : input.value;
  handleChange(control.value.path, value);
}
</script>
<template>
  <SettingsRow
    v-if="control.visible"
    :label="label"
    :class="{ 'schema-hidden-label': control.uischema.label === false }"
    :input-id="control.id"
    :error="control.errors || undefined"
  >
    <select
      v-if="options.length"
      :id="control.id"
      :value="JSON.stringify(control.data)"
      :disabled="!control.enabled"
      :aria-invalid="!!control.errors"
      @change="update"
    >
      <option
        v-for="option in options"
        :key="JSON.stringify(option.value)"
        :value="JSON.stringify(option.value)"
      >
        {{ option.label }}
      </option>
    </select>
    <input
      v-else-if="control.schema.type === 'boolean'"
      :id="control.id"
      type="checkbox"
      :checked="control.data === true"
      :disabled="!control.enabled"
      @change="update"
    >
    <input
      v-else
      :id="control.id"
      :type="control.schema.type === 'number' || control.schema.type === 'integer' ? 'number' : 'text'"
      :value="control.data ?? ''"
      :min="control.schema.minimum"
      :max="control.schema.maximum"
      :step="control.schema.type === 'integer' ? 1 : 'any'"
      :maxlength="control.schema.maxLength"
      :disabled="!control.enabled"
      :aria-invalid="!!control.errors"
      @input="update"
    >
  </SettingsRow>
</template>

<style scoped>
.schema-hidden-label :deep(.settings-row-label){position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)}
.schema-hidden-label{grid-template-columns:1fr}
</style>
