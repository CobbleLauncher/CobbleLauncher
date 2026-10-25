<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowUp, ArrowDown, GripVertical } from '@lucide/vue';
import { defaultHomeLayout, moveHomeSection, readHomeLayout, writeHomeLayout, type HomeLayout, type HomeSection } from './home-layout';
import type { HomeGame } from './library-home';
import type { CommandAction } from './commands/registry';
import { savedMarketItems } from './market-saved';
import { t } from './i18n';
import SwitchField from './ui/SwitchField.vue';
import SelectField from './ui/SelectField.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import ConfirmationDialog from './ui/ConfirmationDialog.vue';
import AppIcon from './ui/AppIcon.vue';
import HomeQuickActions from './settings/HomeQuickActions.vue';
const props = withDefaults(defineProps<{ commands?: CommandAction[]; games?: HomeGame[] }>(), { commands: () => [], games: () => [] });
const emit = defineEmits<{ dirty: [value: boolean] }>();
function copyLayout(value: HomeLayout): HomeLayout { return { ...value, order: [...value.order], hidden: [...value.hidden], quickIds: [...value.quickIds] }; }
const saved = ref(readHomeLayout());
const layout = ref(copyLayout(saved.value));
const dirty = computed(() => JSON.stringify(layout.value) !== JSON.stringify(saved.value));
watch(dirty, value => emit('dirty', value), { immediate: true });
const resetOpen = ref(false);
const dragging = ref<HomeSection | null>(null);
const dropTarget = ref<HomeSection | null>(null);
const announcement = ref('');
const previewGames = computed(() => props.games.slice(0, 2).map(game => game.name));
const quickLabels = computed(() => layout.value.quickIds.map(id => props.commands.find(command => command.id === id)?.label).filter((label): label is string => !!label));
const counts = computed(() => ({ favorite: savedMarketItems.value.filter(item => item.collection === 'favorite').length, later: savedMarketItems.value.filter(item => item.collection === 'later').length }));
function refresh() { if (!dirty.value) { saved.value = readHomeLayout(); layout.value = copyLayout(saved.value); } }
onMounted(() => window.addEventListener('cobble-home-layout-changed', refresh));
onBeforeUnmount(() => window.removeEventListener('cobble-home-layout-changed', refresh));
function save() { const next = copyLayout(layout.value); saved.value = next; writeHomeLayout(next); }
function cancel() { layout.value = copyLayout(saved.value); announcement.value = ''; }
function toggle(section: HomeSection) { layout.value = { ...layout.value, hidden: layout.value.hidden.includes(section) ? layout.value.hidden.filter(value => value !== section) : [...layout.value.hidden, section] }; }
function announce(section: HomeSection) { announcement.value = t('settingsLayout.homeLayout.moved', { name: t(`home.sections.${section}`), position: layout.value.order.indexOf(section) + 1, total: layout.value.order.length }); }
function move(section: HomeSection, direction: -1 | 1) { layout.value = moveHomeSection(layout.value, section, direction); announce(section); }
function keyboardMove(event: KeyboardEvent, section: HomeSection) {
  if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
  event.preventDefault(); move(section, event.key === 'ArrowUp' ? -1 : 1);
}
function dragStart(event: DragEvent, section: HomeSection) { dragging.value = section; event.dataTransfer?.setData('text/plain', section); if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'; }
function dragEnd() { dragging.value = null; dropTarget.value = null; }
function drop(event: DragEvent, target: HomeSection) {
  event.preventDefault();
  const source = dragging.value;
  if (source && source !== target) {
    const position = layout.value.order.indexOf(target);
    const order = layout.value.order.filter(section => section !== source);
    order.splice(position, 0, source); layout.value = { ...layout.value, order }; announce(source);
  }
  dragEnd();
}
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
    <div class="home-layout-preview">
      <div class="home-layout-preview-caption">
        <AppIcon name="monitor" /><span>{{ t('settingsLayout.homeLayout.preview') }}</span>
      </div>
      <div
        class="home-layout-featured"
        aria-hidden="true"
      >
        <span class="home-layout-game-icon"><AppIcon
          name="puzzle"
          :size="26"
        /></span><strong>{{ games[0]?.name ?? t('settingsLayout.previewGame') }}</strong><AppIcon name="play" />
      </div>
      <p
        id="home-layout-help"
        class="home-layout-help"
      >
        {{ t('settingsLayout.homeLayout.help') }}
      </p>
      <ol class="home-layout-sections">
        <li
          v-for="(section, index) in layout.order"
          :key="section"
          :data-section="section"
          :class="{ 'is-hidden': layout.hidden.includes(section), 'is-dragging': dragging === section, 'is-drop-target': dropTarget === section && dragging !== section, 'is-drop-after': dragging && layout.order.indexOf(dragging) < index }"
          @dragover.prevent="dropTarget = section"
          @drop="drop($event, section)"
        >
          <div class="home-layout-section-controls">
            <button
              type="button"
              class="home-layout-drag"
              draggable="true"
              :aria-label="t('settingsLayout.homeLayout.reorder', { name: t(`home.sections.${section}`) })"
              aria-describedby="home-layout-help"
              @dragstart="dragStart($event, section)"
              @dragend="dragEnd"
              @keydown="keyboardMove($event, section)"
            >
              <GripVertical :size="18" />
            </button>
            <h3>{{ t(`home.sections.${section}`) }}</h3>
            <SwitchField
              :model-value="!layout.hidden.includes(section)"
              :label="t('settingsLayout.homeLayout.visible', { name: t(`home.sections.${section}`) })"
              @update:model-value="toggle(section)"
            />
            <div class="home-layout-order-actions">
              <button
                type="button"
                class="text-button"
                :disabled="index === 0"
                :aria-label="t('home.moveUp', { name: t(`home.sections.${section}`) })"
                @click="move(section, -1)"
              >
                <ArrowUp :size="16" />
              </button>
              <button
                type="button"
                class="text-button"
                :disabled="index === layout.order.length - 1"
                :aria-label="t('home.moveDown', { name: t(`home.sections.${section}`) })"
                @click="move(section, 1)"
              >
                <ArrowDown :size="16" />
              </button>
            </div>
          </div>
          <div
            class="home-layout-section-sample"
            :class="{ 'is-card-view': section === 'other' && layout.libraryView === 'cards' }"
            aria-hidden="true"
          >
            <template v-if="section === 'other'">
              <span
                v-for="name in previewGames.length ? previewGames : [t('settingsLayout.previewGame')]"
                :key="name"
                class="home-layout-mini-game"
              ><AppIcon name="puzzle" />{{ name }}</span>
            </template>
            <template v-else-if="section === 'quick'">
              <span
                v-for="name in quickLabels"
                :key="name"
                class="home-layout-mini-action"
              >{{ name }}</span><span
                v-if="!quickLabels.length"
                class="home-layout-empty"
              >{{ t('settingsLayout.homeLayout.emptyQuick') }}</span>
            </template>
            <template v-else>
              <span class="home-layout-mini-action">{{ t('home.marketFavorites', { count: counts.favorite }) }}</span><span class="home-layout-mini-action">{{ t('home.marketLater', { count: counts.later }) }}</span>
            </template>
          </div>
        </li>
      </ol>
    </div>
    <p
      class="sr-only"
      role="status"
      aria-live="polite"
    >
      {{ announcement }}
    </p>
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
.home-layout-settings{display:grid;gap:var(--ui-space-4);max-width:1000px}.home-layout-settings>:deep(.settings-row){padding-top:0;border:0}.home-layout-preview{overflow:hidden;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1)}.home-layout-preview-caption{display:flex;align-items:center;gap:var(--ui-space-2);padding:var(--ui-space-3) var(--ui-space-4);border-bottom:1px solid var(--ui-border);background:var(--ui-surface-2);color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.home-layout-featured{display:flex;align-items:center;gap:var(--ui-space-3);margin:var(--ui-space-4);padding:var(--ui-space-4);border-radius:var(--ui-radius-lg);background:var(--tint)}.home-layout-game-icon{display:grid;place-items:center;width:48px;height:48px;border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-focus);flex:none}.home-layout-featured strong{flex:1;overflow-wrap:anywhere;font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold)}.home-layout-help{margin:0;padding:0 var(--ui-space-4) var(--ui-space-2);color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.home-layout-sections{list-style:none;margin:0;padding:0}.home-layout-sections>li{padding:var(--ui-space-3) var(--ui-space-4);border-top:1px solid var(--ui-border)}.home-layout-sections>li.is-drop-target{box-shadow:inset 0 3px var(--ui-focus)}.home-layout-sections>li.is-drop-target.is-drop-after{box-shadow:inset 0 -3px var(--ui-focus)}.home-layout-sections>li.is-dragging{opacity:.5}.home-layout-section-controls{display:flex;align-items:center;gap:var(--ui-space-3)}.home-layout-section-controls h3{flex:1;min-width:0;margin:0;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:var(--ui-weight-semibold)}.home-layout-drag,.home-layout-order-actions button{display:grid;place-items:center;flex:none;width:var(--ui-control-compact);height:var(--ui-control-height);padding:0;border:0;border-radius:var(--ui-radius-sm);background:transparent;color:var(--ui-text-muted)}.home-layout-drag{cursor:grab}.home-layout-drag:active{cursor:grabbing}.home-layout-drag:hover,.home-layout-order-actions button:hover:not(:disabled){color:var(--ui-text);background:var(--ui-surface-hover)}.home-layout-order-actions{display:flex}.home-layout-order-actions button:disabled{opacity:.3}.home-layout-section-sample{display:flex;flex-wrap:wrap;gap:var(--ui-space-2);margin:var(--ui-space-2) 0 0 calc(var(--ui-control-compact) + var(--ui-space-3))}.home-layout-section-sample.is-card-view{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}.is-hidden .home-layout-section-sample{opacity:.35}.home-layout-mini-game,.home-layout-mini-action{display:flex;align-items:center;gap:var(--ui-space-2);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);overflow-wrap:anywhere}.home-layout-mini-game{flex:1 1 220px;background:var(--ui-surface-2)}.home-layout-mini-game>.app-icon{flex:none;color:var(--ui-text-muted)}.home-layout-empty{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.home-layout-reset{justify-self:start;min-height:var(--ui-control-height)}
@media(max-width:600px){.home-layout-section-controls{gap:var(--ui-space-2)}.home-layout-sections>li{padding-inline:var(--ui-space-3)}.home-layout-section-sample{margin-left:0}.home-layout-order-actions button{width:28px}.home-layout-featured{margin:var(--ui-space-3);padding:var(--ui-space-3)}.home-layout-section-sample.is-card-view{grid-template-columns:1fr}}
</style>
