<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { ContextMenuPortal, ContextMenuContent, ContextMenuItem, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import type { HomeGame } from './library-home';
import { t } from './i18n';
import { createGameActions } from './game-actions';
const props = defineProps<{ game: HomeGame; adjustCover?: boolean; contextOpen?: boolean; contextReturnTarget?: HTMLElement }>();
const emit = defineEmits<{ updated: [game: HomeGame]; manage: [id: string]; adjustCover: [] }>();
const busy = ref(false);
const failed = ref(false);
const shortcutStatus = ref('');
const shortcutFailed = ref(false);
const menu = ref<HTMLElement>();
const dropdownReady = ref(false);
const dropdownOpen = ref(false);
async function openDropdown() {
  if (busy.value) return;
  dropdownReady.value = true;
  await nextTick();
  menu.value?.querySelector<HTMLButtonElement>('.game-menu-trigger')?.focus();
  dropdownOpen.value = true;
}
function dropdownKey(event: KeyboardEvent) {
  if (!['Enter', ' ', 'ArrowDown'].includes(event.key)) return;
  event.preventDefault(); void openDropdown();
}
function returnContextFocus(event: Event) {
  const target = props.contextReturnTarget;
  if (!target?.isConnected) return;
  event.preventDefault();
  const fallback = menu.value?.closest<HTMLElement>('.library-game')?.querySelector<HTMLElement>('.library-game-open');
  (target.matches(':disabled') ? fallback : target)?.focus({ preventScroll: true });
}
const actions = computed(() => {
  const items = createGameActions(props.game, { manage: id => emit('manage', id), updated: game => emit('updated', game) });
  if (props.adjustCover) items.splice(items.findIndex(action => action.id === 'game.cover') + 1, 0, {id: 'game.adjustCover', icon: 'palette', label: t('home.adjustCover'), run: () => emit('adjustCover')});
  return items;
});
async function run(action: (typeof actions.value)[number]) {
  if (busy.value) return;
  busy.value = true; failed.value = false; shortcutStatus.value = ''; shortcutFailed.value = false;
  try { await action.run(); if (action.id === 'game.shortcut') shortcutStatus.value = t('home.shortcutCreated'); }
  catch (cause) {
    if (action.id === 'game.shortcut') { shortcutFailed.value = true; shortcutStatus.value = t(String(cause).includes('SHORTCUT_UNSUPPORTED_PLATFORM') ? 'home.shortcutUnsupported' : 'home.shortcutFailed'); }
    else failed.value = true;
  }
  finally { busy.value = false; }
}
</script>
<template>
  <div
    ref="menu"
    class="game-menu"
  >
    <AppIconButton
      v-if="!dropdownReady"
      class="game-menu-trigger"
      icon="more-horizontal"
      :disabled="busy"
      :label="t('home.options', { name: game.name })"
      aria-haspopup="menu"
      :aria-expanded="false"
      @click="openDropdown"
      @keydown="dropdownKey"
    />
    <DropdownMenuRoot
      v-else
      v-model:open="dropdownOpen"
    >
      <DropdownMenuTrigger as-child>
        <AppIconButton
          class="game-menu-trigger"
          icon="more-horizontal"
          :disabled="busy"
          :label="t('home.options', { name: game.name })"
        />
      </DropdownMenuTrigger>
      <DropdownMenuPortal v-if="dropdownOpen">
        <!-- A positioned portal gives Floating UI the scaled offset parent for app zoom. -->
        <div style="position: fixed; inset: 0; z-index: 30; pointer-events: none">
          <DropdownMenuContent
            class="profile-menu"
            position-strategy="absolute"
            style="pointer-events: auto"
            align="end"
            :side-offset="7"
            :collision-padding="12"
          >
            <DropdownMenuItem
              v-for="action in actions"
              :key="action.id"
              class="profile-menu-item"
              :disabled="busy"
              @select="run(action)"
            >
              <AppIcon :name="action.icon ?? 'puzzle'" />{{ action.label }}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </div>
      </DropdownMenuPortal>
    </DropdownMenuRoot>
    <ContextMenuPortal v-if="contextOpen">
      <div style="position: fixed; inset: 0; z-index: 30; pointer-events: none">
        <ContextMenuContent
          class="profile-menu"
          position-strategy="absolute"
          style="pointer-events: auto"
          :collision-padding="12"
          @close-auto-focus="returnContextFocus"
        >
          <ContextMenuItem
            v-for="action in actions"
            :key="action.id"
            class="profile-menu-item"
            :disabled="busy"
            @select="run(action)"
          >
            <AppIcon :name="action.icon ?? 'puzzle'" />{{ action.label }}
          </ContextMenuItem>
        </ContextMenuContent>
      </div>
    </ContextMenuPortal>
    <span
      v-if="failed"
      class="game-menu-error"
      role="alert"
    >{{ t('home.appearanceError') }}</span>
    <span
      v-if="shortcutStatus"
      class="game-menu-notice"
      :class="{ 'is-error': shortcutFailed }"
      :role="shortcutFailed ? 'alert' : 'status'"
    >{{ shortcutStatus }}</span>
  </div>
</template>
<style scoped>
.game-menu{position:relative}.game-menu-trigger{width:var(--ui-control-height);height:var(--ui-control-height);border:0;background:transparent;color:var(--ui-text-muted)}.game-menu-trigger:hover{background:var(--ui-surface-hover);color:var(--ui-text)}.game-menu-trigger:disabled{opacity:.5}.game-menu-notice,.game-menu-error{position:absolute;right:0;top:calc(var(--ui-control-height) + var(--ui-space-1));min-width:180px;padding:var(--ui-space-2) var(--ui-space-3);background:var(--ui-surface-1);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);color:var(--ui-text);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);z-index:2}.game-menu-notice.is-error,.game-menu-error{color:var(--ui-danger)}
</style>
