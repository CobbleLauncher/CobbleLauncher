<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { defaultHomeLayout, readHomeLayout, writeHomeLayout, type HomeLayout, type HomeSection } from './home-layout';
import type { HomeGame } from './library-home';
import type { CommandAction } from './commands/registry';
import { t } from './i18n';
import SwitchField from './ui/SwitchField.vue';
import SelectField from './ui/SelectField.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import HomeQuickActions from './settings/HomeQuickActions.vue';
withDefaults(defineProps<{ commands?: CommandAction[]; games?: HomeGame[] }>(), { commands: () => [], games: () => [] });
const emit = defineEmits<{ dirty: [value: boolean] }>();
function copyLayout(value: HomeLayout): HomeLayout { return { ...value, order: [...value.order], hidden: [...value.hidden], quickIds: [...value.quickIds] }; }
const saved = ref(readHomeLayout());
const layout = ref(copyLayout(saved.value));
const dirty = computed(() => JSON.stringify(layout.value) !== JSON.stringify(saved.value));
watch(dirty, value => emit('dirty', value), { immediate: true });
const resetOpen = ref(false);
function refresh() { if (!dirty.value) { saved.value = readHomeLayout(); layout.value = copyLayout(saved.value); } }
onMounted(() => window.addEventListener('cobble-home-layout-changed', refresh));
onBeforeUnmount(() => window.removeEventListener('cobble-home-layout-changed', refresh));
function save() { const next = copyLayout(layout.value); saved.value = next; writeHomeLayout(next); }
function cancel() { layout.value = copyLayout(saved.value); }
function toggle(section: HomeSection) { layout.value = { ...layout.value, hidden: layout.value.hidden.includes(section) ? layout.value.hidden.filter(value => value !== section) : [...layout.value.hidden, section] }; }
function reset() { layout.value = copyLayout(defaultHomeLayout); }
defineExpose({ dirty, save, cancel });
</script>
<template>
  <section
    class="home-layout-settings"
    :aria-label="t('settingsLayout.homeLayout.title')"
  >
    <SettingsRow
      :label="t('settingsLayout.homeLayout.view')"
      input-id="home-library-view"
    >
      <SelectField
        id="home-library-view"
        v-model="layout.libraryView"
        :label="t('settingsLayout.homeLayout.view')"
        :options="[{ value: 'list', label: t('home.listView') }, { value: 'cards', label: t('home.cardView') }]"
      />
    </SettingsRow>
    <SettingsRow
      v-for="section in (['other','quick','favorites'] as const)"
      :key="section"
      :data-section="section"
      :label="t(`home.sections.${section}`)"
    >
      <SwitchField
        :model-value="!layout.hidden.includes(section)"
        :label="t('settingsLayout.homeLayout.visible', { name: t(`home.sections.${section}`) })"
        @update:model-value="toggle(section)"
      />
    </SettingsRow>
    <SettingsDisclosure
      :title="t('settingsLayout.homeLayout.quickActions')"
      icon="puzzle"
      :value="`${layout.quickIds.length} / 6`"
      bordered
    >
      <HomeQuickActions
        v-model="layout.quickIds"
        :commands="commands"
      />
    </SettingsDisclosure>
    <button
      type="button"
      class="text-button home-layout-reset"
      @click="resetOpen = true"
    >
      {{ t('home.resetHome') }}
    </button>
    <ConfirmationDialog
      v-model:open="resetOpen"
      :title="t('settingsLayout.homeLayout.resetTitle')"
      :description="t('settingsLayout.editor.resetDraftDescription')"
      :confirm-label="t('settingsLayout.editor.restore')"
      @confirm="reset"
    />
  </section>
</template>
<style scoped>
.home-layout-settings{display:grid;gap:var(--ui-space-4);max-width:1000px}.home-layout-reset{justify-self:start;min-height:var(--ui-control-height)}
</style>
