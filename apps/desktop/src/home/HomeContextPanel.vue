<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem } from 'reka-ui';
import type { Game, LocalWorld } from '../games';
import type { CommandAction } from '../commands/registry';
import { afterHomePaint } from '../after-home-paint';
import { locale, t } from '../i18n';
import { readHomeLayout, writeHomeLayout } from '../home-layout';
import { snapshotTimestamp } from '../world-snapshots';
import { homeWorldResources, type HomeWorldResources } from './world-resources';
import { toolPresentation } from './tool-presentation';
import AppIcon from '../ui/AppIcon.vue';
const props = withDefaults(defineProps<{ game: Game; active: boolean; commands: CommandAction[]; loading?: boolean; showTools?: boolean; unavailableTools?: number }>(), { showTools: true, unavailableTools: 0 });
const emit = defineEmits<{ section: [section: 'worlds' | 'screenshots', folderName?: string]; mesh: [mode: 'host' | 'join']; tools: []; configureTools: [] }>();
const worlds = ref<LocalWorld[]>([]), reading = ref(false), failed = ref(false), busy = ref(''), actionError = ref(false);
const summary = ref<HomeWorldResources | null>(null), summaryFailed = ref(false), loadingVisible = ref(false);
const toolsLoadingVisible = ref(false);
const layout = ref(readHomeLayout()), menuOpen = ref('');
const loadResources = homeWorldResources();
let revision = 0, cancel: (() => void) | undefined, loadingTimer: ReturnType<typeof setTimeout> | undefined;
let toolsLoadingTimer: ReturnType<typeof setTimeout> | undefined;
const recentWorld = computed(() => [...worlds.value].sort((a,b) => (b.last_played ?? 0) - (a.last_played ?? 0))[0]);
const backupUnavailable = computed(() => summaryFailed.value || summary.value?.backup_unavailable);
const lastPlayed = computed(() => recentWorld.value?.last_played ? new Date(recentWorld.value.last_played).toLocaleDateString(locale.value) : '');
const backupDate = computed(() => {
  const timestamp = summary.value?.last_backup ? snapshotTimestamp(summary.value.last_backup) : NaN;
  return Number.isFinite(timestamp) ? new Date(timestamp).toLocaleDateString(locale.value) : '';
});
function refreshLayout() { layout.value = readHomeLayout(); }
function togglePinned(command: CommandAction) {
  const current = readHomeLayout(), pinned = current.quickIds.includes(command.id);
  if (!pinned && current.quickIds.length >= 6) { emit('configureTools'); return; }
  writeHomeLayout({ ...current, quickIds: pinned ? current.quickIds.filter(id => id !== command.id) : [...current.quickIds, command.id] });
  refreshLayout();
}
function refresh() {
  cancel?.(); clearTimeout(loadingTimer); loadingVisible.value = false; const current = ++revision;
  if (!props.active || props.game.status !== 'installed') {
    reading.value = false;
    if (props.game.status !== 'installed') { worlds.value = []; summary.value = null; }
    return;
  }
  reading.value = true; failed.value = false;
  loadingTimer = setTimeout(() => { if (current === revision && reading.value) loadingVisible.value = true; }, 180);
  const id = props.game.id;
  cancel = afterHomePaint(() => { void (async () => {
    try {
      const value = await invoke<LocalWorld[]>('list_game_worlds', { id });
      if (current !== revision) return;
      const previous = recentWorld.value;
      worlds.value = value;
      const world = recentWorld.value;
      if (!world || previous?.folder_name !== world.folder_name || previous?.last_played !== world.last_played) summary.value = null;
      summaryFailed.value = false;
      if (world) {
        try { const result = await loadResources(id, world); if (current === revision) summary.value = result; }
        catch { if (current === revision) summaryFailed.value = true; }
      }
    } catch { if (current === revision) failed.value = true; }
    finally { if (current === revision) { reading.value = false; loadingVisible.value = false; clearTimeout(loadingTimer); } }
  })(); });
}
watch(() => props.game.id, () => { worlds.value = []; summary.value = null; summaryFailed.value = false; actionError.value = false; menuOpen.value = ''; });
watch(() => [props.game.id, props.game.status, props.game.launch_state, props.active], refresh, { immediate: true });
watch(() => props.loading, value => {
  clearTimeout(toolsLoadingTimer); toolsLoadingVisible.value = false;
  if (value) toolsLoadingTimer = setTimeout(() => { toolsLoadingVisible.value = true; }, 180);
}, { immediate: true });
onMounted(() => window.addEventListener('cobble-home-layout-changed', refreshLayout));
onBeforeUnmount(() => { ++revision; cancel?.(); clearTimeout(loadingTimer); clearTimeout(toolsLoadingTimer); window.removeEventListener('cobble-home-layout-changed', refreshLayout); });
async function run(action: CommandAction) {
  if (busy.value || action.enabled === false) return;
  busy.value = action.id; actionError.value = false;
  try { await action.run(); } catch { actionError.value = true; } finally { busy.value = ''; }
}
</script>
<template>
  <div class="home-context">
    <section
      class="home-context-worlds"
      :aria-busy="reading"
    >
      <header>
        <h2><AppIcon name="folder" />{{ t('home.dashboard.worlds') }}</h2><button
          class="text-button"
          @click="emit('section', 'worlds')"
        >
          {{ t('home.dashboard.openWorlds') }}<AppIcon name="chevron-right" />
        </button>
      </header>
      <div
        v-if="reading && !worlds.length"
        class="home-world-placeholder"
        role="status"
        :aria-label="t('navigation.loading')"
      >
        <AppIcon
          v-if="loadingVisible"
          name="loader-circle"
          class="ui-busy-icon"
        />
      </div>
      <div
        v-else-if="failed"
        class="context-error"
      >
        <p role="alert">
          {{ t('home.dashboard.worldsFailed') }}
        </p><button
          class="text-button"
          @click="refresh"
        >
          {{ t('home.dashboard.retry') }}
        </button>
      </div>
      <div
        v-else-if="recentWorld"
        class="home-world-summary"
      >
        <div
          class="home-world-art"
          aria-hidden="true"
        >
          <img
            v-if="summary?.thumbnail"
            :src="summary.thumbnail"
            alt=""
            @error="summary = summary && { ...summary, thumbnail: null }"
          ><AppIcon
            v-else
            name="blocks"
            :size="32"
          />
        </div><div>
          <button
            class="home-world-name text-button"
            :title="recentWorld.name"
            @click="emit('section', 'worlds', recentWorld.folder_name)"
          >
            {{ recentWorld.name }}
          </button><p v-if="lastPlayed">
            {{ t('home.lastPlayed', { date: lastPlayed }) }}
          </p><p
            class="home-backup-state"
          >
            <template v-if="summary || summaryFailed">
              <AppIcon :name="backupUnavailable ? 'alert-circle' : 'shield'" />{{ backupUnavailable ? t('home.dashboard.backupUnavailable') : backupDate ? t('home.dashboard.lastBackup', { date: backupDate }) : t('home.dashboard.noBackups') }}
            </template>
          </p><button
            class="text-button"
            @click="emit('section', 'worlds', recentWorld.folder_name)"
          >
            {{ t('home.dashboard.worldBackups') }}
          </button>
        </div>
      </div>
      <p
        v-else
        class="home-context-muted"
      >
        {{ t('home.dashboard.noWorlds') }}
      </p>
      <button
        class="text-button home-screenshots"
        @click="emit('section', 'screenshots')"
      >
        <AppIcon name="folder" />{{ t('gameDetails.screenshots') }}
      </button>
    </section>
    <section>
      <header><h2><AppIcon name="network" />{{ t('home.dashboard.multiplayer') }}</h2></header>
      <div class="home-invite-actions">
        <button
          class="secondary-button"
          @click="emit('mesh', 'host')"
        >
          <AppIcon name="share" />{{ t('home.dashboard.createInvite') }}
        </button><button
          class="text-button"
          @click="emit('mesh', 'join')"
        >
          {{ t('home.dashboard.joinInvite') }}
        </button>
      </div>
    </section>
    <section v-if="showTools !== false">
      <header>
        <h2>{{ t('home.sections.quick') }}</h2><button
          class="text-button"
          @click="emit('tools')"
        >
          {{ t('navigation.tools') }}<AppIcon name="chevron-right" />
        </button>
      </header>
      <div
        v-if="commands.length"
        class="home-context-tools"
      >
        <ContextMenuRoot
          v-for="command in commands"
          :key="command.id"
          @update:open="menuOpen = $event ? command.id : ''"
        >
          <ContextMenuTrigger as-child>
            <button
              :disabled="!!busy || command.enabled === false"
              :title="command.label"
              @click="run(command)"
            >
              <AppIcon
                :name="busy === command.id ? 'loader-circle' : toolPresentation(command).icon"
                :class="{ 'ui-busy-icon': busy === command.id }"
                :size="24"
              /><span>{{ toolPresentation(command).label }}</span>
            </button>
          </ContextMenuTrigger>
          <ContextMenuPortal v-if="menuOpen === command.id">
            <ContextMenuContent
              class="profile-menu"
              :collision-padding="12"
            >
              <ContextMenuItem
                class="profile-menu-item"
                @select="togglePinned(command)"
              >
                <AppIcon name="pin" />{{ t(layout.quickIds.includes(command.id) ? 'home.dashboard.unpinTool' : 'home.dashboard.pinTool') }}
              </ContextMenuItem>
              <ContextMenuItem
                class="profile-menu-item"
                @select="emit('configureTools')"
              >
                <AppIcon name="settings" />{{ t('home.dashboard.configureTools') }}
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenuPortal>
        </ContextMenuRoot>
      </div>
      <div
        v-else-if="loading"
        class="home-tools-placeholder"
        role="status"
        :aria-label="t('navigation.loading')"
      >
        <AppIcon
          v-if="toolsLoadingVisible"
          name="loader-circle"
          class="ui-busy-icon"
        />
      </div>
      <button
        v-else
        class="secondary-button"
        @click="emit('tools')"
      >
        <AppIcon name="puzzle" />{{ t('home.dashboard.chooseTools') }}
      </button>
      <button
        v-if="!loading && unavailableTools"
        class="text-button home-unavailable-tools"
        @click="emit('configureTools')"
      >
        <AppIcon name="settings" />{{ t('home.dashboard.unavailableTools', { count: unavailableTools }) }}
      </button>
      <p
        v-if="actionError"
        role="alert"
      >
        {{ t('home.quickActionFailed') }}
      </p>
    </section>
    <slot />
  </div>
</template>
<style scoped>
.home-context{min-width:0;display:flex;flex-direction:column;gap:24px;padding:8px 0}.home-context section+section{border-top:1px solid var(--ui-border);padding-top:24px}.home-context header{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:16px}.home-context h2{display:flex;align-items:center;gap:8px;font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600;margin:0}.home-context button{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:var(--ui-control-height);font:inherit;font-size:var(--ui-text-label)}.home-context .text-button{padding:4px 8px}.home-world-summary{display:flex;align-items:center;gap:16px}.home-world-summary>div:last-child{min-width:0}.home-context .home-world-name{justify-content:flex-start;text-align:left;padding:0;min-height:0;font-size:var(--ui-text-body);color:var(--ui-text);display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-weight:500;overflow-wrap:anywhere}.home-context p{display:flex;align-items:center;gap:8px;margin:8px 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.home-backup-state{min-height:18px}.home-world-art img{width:100%;height:100%;object-fit:cover;border-radius:inherit}.home-world-placeholder,.home-tools-placeholder{display:flex;align-items:center;justify-content:center;min-height:96px;color:var(--ui-text-muted)}.home-world-art{display:grid;place-items:center;width:96px;height:72px;flex:none;background:var(--tint);border-radius:8px;color:var(--ui-focus)}.home-screenshots{margin-top:8px}.home-invite-actions{display:flex;align-items:center;gap:16px;flex-wrap:wrap;padding:16px;background:var(--tint);border-radius:8px}.home-context-tools{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px}.home-context-tools button{flex-direction:column;gap:10px;min-width:0;padding:12px 8px;border:0;border-radius:8px;background:transparent;color:var(--ui-text)}.home-context-tools button:hover{background:var(--ui-surface-hover)}.home-context-tools span{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere;font-size:var(--ui-text-label)}.home-context-tools .app-icon{color:var(--ui-focus)}.context-error{display:flex;align-items:center;gap:12px}
</style>
