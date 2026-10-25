<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { TabsList, TabsTrigger } from 'reka-ui';
import AppIcon from '../ui/AppIcon.vue';
import SelectField from '../ui/SelectField.vue';
import type { AppIconName } from '../ui/icons';
import type { SettingsCategory } from './categories';
import { t } from '../i18n';
const props = defineProps<{ categories: SettingsCategory[]; active: string }>();
const emit = defineEmits<{ select: [value: string] }>();
const icons: Record<string, AppIconName> = { general: 'palette', 'home-layout': 'monitor', shortcuts: 'keyboard', downloads: 'hard-drive', runtime: 'cpu', accounts: 'user', about: 'info' };
const options = computed(() => props.categories.map(category => ({ value: category.id, label: category.labelKey ? t(category.labelKey) : category.label })));
</script>
<template>
  <div class="settings-navigation">
    <TabsList
      class="settings-category-tabs"
      :aria-label="t('settingsLayout.category')"
    >
      <TabsTrigger
        v-for="category in categories"
        :key="category.id"
        :value="category.id"
        class="settings-category-tab"
      >
        <AppIcon :name="icons[category.id] ?? 'puzzle'" />
        {{ category.labelKey ? t(category.labelKey) : category.label }}
      </TabsTrigger>
    </TabsList>
    <div class="settings-category-select">
      <SelectField
        id="settings-category"
        :model-value="active"
        :options="options"
        :label="t('settingsLayout.category')"
        @update:model-value="emit('select', $event)"
      />
    </div>
  </div>
</template>
<style scoped>
.settings-navigation{min-width:0;position:sticky;top:0;align-self:start}
.settings-category-tabs{display:flex;flex-direction:column;gap:4px;padding-right:20px}
.settings-category-tab{display:flex;align-items:center;gap:12px;width:100%;min-height:var(--ui-control-height);padding:10px 12px;border:0;border-radius:var(--ui-radius-md);background:transparent;color:var(--ui-text-muted);font:inherit;font-size:var(--ui-text-label);text-align:start}
.settings-category-tab:hover{background:var(--ui-surface-hover)}
.settings-category-tab[data-state=active]{background:var(--tint);color:var(--ui-text);font-weight:var(--ui-weight-semibold)}
.settings-category-tab[data-state=active] .app-icon{color:var(--ui-focus)}
.settings-category-tab:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}
.settings-category-select{display:none}
@media(max-width:760px){.settings-navigation{position:static}.settings-category-tabs{display:none}.settings-category-select{display:block;max-width:360px}}
</style>
