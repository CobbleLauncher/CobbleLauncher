<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenuPortal, ContextMenuContent, ContextMenuItem, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import AppIcon from './ui/AppIcon.vue';
import type { HomeGame } from './library-home';
import { t } from './i18n';
import type { AppIconName } from './ui/icons';
const props = defineProps<{ game: HomeGame }>();
const emit = defineEmits<{ updated: [game: HomeGame]; manage: [id: string] }>();
const busy = ref(false);
const failed = ref(false);
const shortcutStatus = ref('');
const shortcutFailed = ref(false);
const actions = computed<{ id: string; icon: AppIconName; label: string; run: () => unknown }[]>(() => [
  ...(props.game.status === 'installed' ? [{ id: 'manage', icon: 'settings' as const, label: t('home.manage', { name: props.game.name }), run: () => emit('manage', props.game.id) }] : []),
  ...(props.game.status === 'installed' ? [{ id: 'shortcut', icon: 'external-link' as const, label: t('home.shortcutCreate'), run: () => createShortcut() }] : []),
  { id: 'pin', icon: 'pin', label: t(props.game.pinned ? 'home.unpin' : 'home.pin'), run: () => change('set_game_pinned') },
  { id: 'cover', icon: 'palette', label: t('home.chooseCover'), run: () => change('pick_game_cover') },
  ...(props.game.cover_ref ? [{ id: 'reset-cover', icon: 'x' as const, label: t('home.removeCover'), run: () => change('reset_game_cover') }] : []),
]);
async function createShortcut() {
  if (busy.value) return;
  busy.value = true; failed.value = false; shortcutStatus.value = ''; shortcutFailed.value = false;
  try { await invoke('create_game_shortcut', { id: props.game.id }); shortcutStatus.value = t('home.shortcutCreated'); }
  catch (cause) { shortcutFailed.value = true; shortcutStatus.value = t(String(cause).includes('SHORTCUT_UNSUPPORTED_PLATFORM') ? 'home.shortcutUnsupported' : 'home.shortcutFailed'); }
  finally { busy.value = false; }
}
async function change(command: string) {
  if (busy.value) return;
  busy.value = true;
  failed.value = false;
  try {
    const game = await invoke<HomeGame | null>(command, { id: props.game.id, ...(command === 'set_game_pinned' ? { pinned: !props.game.pinned } : {}) });
    if (game) emit('updated', game);
  } catch { failed.value = true; }
  finally { busy.value = false; }
}
</script>
<template>
  <div class="game-menu">
    <DropdownMenuRoot>
      <DropdownMenuTrigger
        class="game-menu-trigger"
        :disabled="busy"
        :aria-label="t('home.options', { name: game.name })"
      >
        <AppIcon name="more-horizontal" />
      </DropdownMenuTrigger>
      <DropdownMenuPortal>
        <DropdownMenuContent
          class="profile-menu"
          align="end"
          :side-offset="7"
          :collision-padding="12"
        >
          <DropdownMenuItem
            v-for="action in actions"
            :key="action.id"
            class="profile-menu-item"
            :disabled="busy"
            @select="action.run()"
          >
            <AppIcon :name="action.icon" />{{ action.label }}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenuPortal>
    </DropdownMenuRoot>
    <ContextMenuPortal>
      <ContextMenuContent
        class="profile-menu"
        :collision-padding="12"
      >
        <ContextMenuItem
          v-for="action in actions"
          :key="action.id"
          class="profile-menu-item"
          :disabled="busy"
          @select="action.run()"
        >
          <AppIcon :name="action.icon" />{{ action.label }}
        </ContextMenuItem>
      </ContextMenuContent>
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
.game-menu-notice{position:absolute;right:0;top:36px;min-width:180px;padding:8px 12px;background:var(--panel);border:1px solid var(--line);border-radius:8px;color:var(--ink);font-size:12px;z-index:2}.game-menu-notice.is-error{color:var(--danger)}
.game-menu{position:relative}.game-menu-trigger{display:grid;place-items:center;width:32px;height:32px;border:0;border-radius:8px;background:transparent;color:var(--muted)}.game-menu-trigger:hover{background:var(--hover);color:var(--ink)}.game-menu-trigger:disabled{opacity:.5}.game-menu-error{position:absolute;right:0;top:36px;min-width:180px;padding:8px 12px;background:var(--panel);border:1px solid var(--line);border-radius:8px;color:var(--danger);font-size:12px;z-index:2}
</style>
