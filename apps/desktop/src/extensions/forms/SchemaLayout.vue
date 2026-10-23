<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, inject } from 'vue';
import type { GroupLayout, Layout } from '@jsonforms/core';
import { DispatchRenderer, rendererProps, useJsonFormsLayout } from '@jsonforms/vue';
import SettingsSection from '../../ui/settings/SettingsSection.vue';
const props = defineProps(rendererProps<Layout>());
const { layout } = useJsonFormsLayout(props);
const translate = inject<(key: string, fallback?: string) => string | undefined>('schemaTranslate', (_key, fallback) => fallback);
const label = computed(() => {
  const value = (layout.value.uischema as GroupLayout).label;
  return value ? translate(value, value) : undefined;
});
</script>
<template>
  <component
    :is="layout.uischema.type === 'Group' ? SettingsSection : 'div'"
    v-if="layout.visible"
    :title="layout.uischema.type === 'Group' ? label : undefined"
    class="schema-layout"
    :class="{ 'schema-horizontal': layout.uischema.type === 'HorizontalLayout' }"
  >
    <DispatchRenderer
      v-for="(element, index) in layout.uischema.elements"
      :key="index"
      :schema="layout.schema"
      :uischema="element"
      :path="layout.path"
      :enabled="layout.enabled"
      :renderers="layout.renderers"
      :cells="layout.cells"
    />
  </component>
</template>
<style scoped>
.schema-layout{display:grid;gap:var(--ui-space-3);min-width:0}.schema-horizontal{grid-template-columns:repeat(auto-fit,minmax(min(240px,100%),1fr))}
</style>
