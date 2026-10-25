<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, markRaw, provide } from 'vue';
import { JsonForms, type JsonFormsChangeEvent } from '@jsonforms/vue';
import { isControl, rankWith, uiTypeIs, or, type JsonSchema, type UISchemaElement, type Translator } from '@jsonforms/core';
import type { ErrorObject } from 'ajv';
import SchemaControl from './SchemaControl.vue';
import SchemaLayout from './SchemaLayout.vue';
import { schemaUiRules } from './ui-schema-rules';
const props = defineProps<{ translations?: Record<string, string>; schema: JsonSchema; uiSchema?: UISchemaElement; data: Record<string, unknown>; disabled?: boolean; errors?: ErrorObject[] }>();
const translate: Translator = <D extends string | undefined = undefined>(key: string, fallback?: D) =>
  (props.translations?.[key] ?? fallback) as D extends string ? string : string | undefined;
provide('schemaTranslate', translate);
const i18n = computed(() => ({ translate }));
const safeUiSchema = computed(() => schemaUiRules(props.uiSchema));
const emit = defineEmits<{ change: [data: Record<string, unknown>] }>();
const renderers = Object.freeze([
  { tester: rankWith(1, isControl), renderer: markRaw(SchemaControl) },
  { tester: rankWith(1, or(uiTypeIs('VerticalLayout'), uiTypeIs('HorizontalLayout'), uiTypeIs('Group'))), renderer: markRaw(SchemaLayout) },
]);
function changed(event: JsonFormsChangeEvent) { emit('change', event.data); }
</script>
<template>
  <JsonForms
    :i18n="i18n"
    :schema="schema"
    :uischema="safeUiSchema"
    :data="data"
    :readonly="disabled"
    :renderers="renderers"
    validation-mode="NoValidation"
    :additional-errors="errors"
    @change="changed"
  />
</template>
