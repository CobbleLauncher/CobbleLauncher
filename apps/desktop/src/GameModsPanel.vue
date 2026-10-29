<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue';
import { ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem, PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import { invoke } from '@tauri-apps/api/core';
import type { Game, LocalMod } from './games';
import { useContentReveal, type ContentReveal } from './content-reveal';
import type { ModsActivity } from './game-mods';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import SearchField from './ui/SearchField.vue';
import SelectField from './ui/SelectField.vue';
import SwitchField from './ui/SwitchField.vue';
import ModUpdates from './ModUpdates.vue';
import InstalledModDetails from './InstalledModDetails.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';
import { locale, t } from './i18n';
import { usePageWorkspace } from './use-page-workspace';
import { isContextMenuKey, navigateListWithArrows } from './list-keyboard';
const props = defineProps<{ game: Game; locked: boolean; revision?: number; reveal?: ContentReveal }>();
const emit = defineEmits<{ activity: [value: ModsActivity]; history: [] }>();
type ContentWorkspace = { query: string; stateFilter: 'all' | 'enabled' | 'disabled'; sourceFilter: 'all' | 'modrinth' | 'curseforge' | 'local'; page: number; selectedMods: string[] };
const workspace = shallowRef(usePageWorkspace<ContentWorkspace>('game-content', props.game.id, () => ({ query: '', stateFilter: 'all', sourceFilter: 'all', page: 1, selectedMods: [] })));
const mods = ref<LocalMod[]>([]);
const tab = ref<'installed' | 'updates'>('installed');
const selectedMod = ref<LocalMod | null>(null);
const modUpdates = ref<InstanceType<typeof ModUpdates> | null>(null);
function viewVersions() {
  if (!selectedMod.value || busy.value || loading.value) return;
  tab.value = 'updates';
  void modUpdates.value?.openInstalledVersion(selectedMod.value.file_name);
}
type ModAction = 'details' | 'versions' | 'toggle' | 'remove';
function modActions(mod: LocalMod) {
  return [
    { id: 'details' as const, label: t('gameMods.viewDetails', { name: mod.name }), disabled: busy.value || loading.value },
    { id: 'versions' as const, label: t('gameMods.versionsDependencies'), disabled: busy.value || loading.value },
    { id: 'toggle' as const, label: t(mod.enabled ? 'gameMods.disable' : 'gameMods.enable', { name: mod.name }), disabled: locked.value || busy.value || loading.value },
    { id: 'remove' as const, label: t('gameMods.removeMod', { name: mod.name }), disabled: locked.value || busy.value || loading.value },
  ];
}
function runModAction(mod: LocalMod, action: ModAction) {
  if (modActions(mod).find(item => item.id === action)?.disabled) return;
  if (action === 'details') selectedMod.value = mod;
  else if (action === 'versions') { selectedMod.value = mod; viewVersions(); }
  else if (action === 'toggle') void mutate('set_game_mod_enabled', { fileName: mod.file_name, enabled: !mod.enabled });
  else { deleting.value = mod.file_name; batchDeleting.value = false; }
}
const updatingMods = ref(false);
const recognizingMods = ref(false);
const contentInstalling = computed(() => updatingMods.value || !!props.game.content_installing);
const query = ref(workspace.value.query);
const stateFilter = ref(workspace.value.stateFilter);
const sourceFilter = ref(workspace.value.sourceFilter);
const sources = ref<Map<string, string>>(new Map());
const readingSources = ref(false);
const sourcesReady = ref(false);
const sourceError = ref('');
let sourceRequest = 0;
const enabledCount = computed(() => mods.value.filter(mod => mod.enabled).length);
const hasFilter = computed(() => !!query.value.trim() || stateFilter.value !== 'all' || sourceFilter.value !== 'all');
const activeFilterCount = computed(() => Number(stateFilter.value !== 'all') + Number(sourceFilter.value !== 'all'));
const canonicalFile = (filename: string) => filename.replace(/\.disabled$/i, '');
async function readSources() {
  const current = ++sourceRequest;
  const id = props.game.id;
  readingSources.value = true; sourceError.value = '';
  try {
    const [modrinth, curseforge] = await Promise.all([
      invoke<{ filename: string }[]>('list_managed_game_mods', { id }),
      invoke<{ filename: string }[]>('list_game_curseforge_mods', { id }),
    ]);
    if (current !== sourceRequest || id !== props.game.id) return;
    const index = new Map<string, string>();
    for (const [source, records] of [['modrinth', modrinth], ['curseforge', curseforge]] as const) {
      for (const record of records) {
        const filename = canonicalFile(record.filename);
        // Conflicting provenance is not confidently classified as either source.
        index.set(filename, index.has(filename) ? 'unknown' : source);
      }
    }
    sources.value = index;
    sourcesReady.value = true;
  } catch { if (current === sourceRequest) sourceError.value = t('gameMods.sourcesFailed'); }
  finally { if (current === sourceRequest) readingSources.value = false; }
}
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const deleting = ref<string | null>(null);
const selectedMods = computed({ get: () => workspace.value.selectedMods, set: (value: string[]) => { workspace.value.selectedMods = [...value]; } });
const batchDeleting = ref(false);
let batchOperation = 0;
const activity = computed<ModsActivity>(() => ({ busy: busy.value || updatingMods.value, installing: false, updating: updatingMods.value, recognizing: recognizingMods.value }));
watch(activity, value => emit('activity', value), { immediate: true });
watch(mods, (value) => { selectedMods.value = selectedMods.value.filter((file) => value.some((mod) => mod.file_name === file)); });
const allVisibleSelected = computed(() => filtered.value.length > 0 && filtered.value.every((mod) => selectedMods.value.includes(mod.file_name)));
function selectMod(file: string, checked: boolean) { selectedMods.value = checked ? [...new Set([...selectedMods.value, file])] : selectedMods.value.filter((value) => value !== file); batchDeleting.value = false; }
function selectVisible(checked: boolean) {
  const visible = new Set(filtered.value.map((mod) => mod.file_name));
  selectedMods.value = checked ? [...new Set([...selectedMods.value, ...visible])] : selectedMods.value.filter((file) => !visible.has(file));
  batchDeleting.value = false;
}
async function batch(command: 'set_game_mod_enabled' | 'remove_game_mod', enabled = false) {
  if (locked.value || busy.value || loading.value || !selectedMods.value.length) return;
  const targets = mods.value.filter((mod) => selectedMods.value.includes(mod.file_name));
  const id = props.game.id;
  const current = ++request;
  const operation = ++batchOperation;
  busy.value = true; batchDeleting.value = false; deleting.value = null; error.value = '';
  let currentName = '';
  try {
    for (const mod of targets) {
      if (current !== request || id !== props.game.id) return;
      if (locked.value) { error.value = t('gameMods.locked'); break; }
      currentName = mod.name;
      await invoke(command, { id, fileName: mod.file_name, ...(command === 'set_game_mod_enabled' ? { enabled } : {}) });
      if (current !== request || id !== props.game.id) return;
      selectedMods.value = selectedMods.value.filter((file) => file !== mod.file_name);
    }
  } catch (cause) { if (current === request && id === props.game.id) error.value = t('gameMods.batchError', { name: currentName, cause: String(cause) }); }
  finally {
    if (operation === batchOperation && id === props.game.id) {
      if (current === request) await refresh(true);
      if (operation === batchOperation && id === props.game.id) busy.value = false;
    }
  }
}
let request = 0;
const locked = computed(() => props.locked || contentInstalling.value);
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return mods.value.filter((mod) => {
    if (stateFilter.value !== 'all' && mod.enabled !== (stateFilter.value === 'enabled')) return false;
    if (sourceFilter.value !== 'all' && (!sourcesReady.value || (sources.value.get(canonicalFile(mod.file_name)) ?? 'local') !== sourceFilter.value)) return false;
    return [mod.name, mod.file_name, mod.version, mod.loader, ...mod.authors].some(value => value?.toLocaleLowerCase().includes(search));
  });
});
const { page: modPage, pageCount: modPageCount, pageItems: pageMods } = usePluginListPagination(filtered, [query, stateFilter, sourceFilter, () => props.game.id], 50);
const contextOpen = ref(false);
const contextMod = ref<LocalMod | null>(null);
const contextReturnTarget = shallowRef<HTMLElement>();
function prepareModContext(event: MouseEvent) {
  const row = event.target instanceof Element ? event.target.closest<HTMLElement>('.mod-row') : null;
  const mod = pageMods.value.find(mod => mod.file_name === row?.dataset.modFile);
  if (!mod) { event.preventDefault(); event.stopPropagation(); return; }
  contextMod.value = mod;
  const target = event.target instanceof Element ? event.target.closest<HTMLElement>('button,input,select,[tabindex]') : null;
  contextReturnTarget.value = target && row?.contains(target) ? target : row?.querySelector<HTMLElement>('.mod-details-link') ?? undefined;
}
function returnContextFocus(event: Event) {
  const target = contextReturnTarget.value;
  if (!target?.isConnected) return;
  event.preventDefault();
  const fallback = target.closest<HTMLElement>('.mod-row')?.querySelector<HTMLElement>('.mod-details-link');
  (target.matches(':disabled') ? fallback : target)?.focus({ preventScroll: true });
}
function prepareTouchContext(event: PointerEvent) { if (event.pointerType === 'touch' || event.pointerType === 'pen') prepareModContext(event); }
watch([query, stateFilter, sourceFilter, modPage, () => props.game.id, mods], () => { contextOpen.value = false; contextMod.value = null; });
modPage.value = workspace.value.page;
watch(query, value => { workspace.value.query = value; });
watch(stateFilter, value => { workspace.value.stateFilter = value; });
watch(sourceFilter, value => { workspace.value.sourceFilter = value; });
watch(modPage, value => { workspace.value.page = value; });
const revealRoot = ref<HTMLElement>();
const highlighted = useContentReveal(() => props.reveal, mods, revealRoot, async file => {
  tab.value = 'installed'; selectedMod.value = null; query.value = ''; stateFilter.value = 'all'; sourceFilter.value = 'all';
  await nextTick(); modPage.value = Math.max(1, Math.floor(filtered.value.findIndex(mod => mod.file_name === file) / 50) + 1);
});
async function refresh(preserveError = false) {
  if (!props.game.loader_version) return;
  const current = ++request;
  loading.value = true;
  if (!preserveError) error.value = '';
  try {
    const result = await invoke<LocalMod[]>('list_game_mods', { id: props.game.id });
    if (current === request) { mods.value = result; if (sourceFilter.value !== 'all') void readSources(); }
  } catch (cause) { if (current === request && (!preserveError || !error.value)) error.value = t('gameMods.readError', { cause: String(cause) }); }
  finally { if (current === request) loading.value = false; }
}
async function refreshContent() {
  await refresh();
}
function showInstalled() { tab.value = 'installed'; selectedMod.value = null; }
defineExpose({ refreshContent, showInstalled });
async function mutate(command: string, args: Record<string, unknown> = {}) {
  if (locked.value || busy.value || loading.value) return;
  const current = ++request;
  busy.value = true;
  error.value = '';
  try {
    const result = await invoke<LocalMod[] | null>(command, { id: props.game.id, ...args });
    if (current === request && result) mods.value = result;
  } catch (cause) { if (current === request) error.value = t('gameMods.updateError', { cause: String(cause) }); }
  finally { if (current === request) busy.value = false; }
}
function remove(mod: LocalMod) {
  if (locked.value || busy.value || loading.value) return;
  deleting.value = null;
  void mutate('remove_game_mod', { fileName: mod.file_name });
}
watch(locked, (value) => { if (value) { deleting.value = null; batchDeleting.value = false; } });
watch([stateFilter, sourceFilter], () => { selectedMods.value = []; batchDeleting.value = false; deleting.value = null; });
watch(sourceFilter, value => { if (value !== 'all') void readSources(); else { ++sourceRequest; readingSources.value = false; sourceError.value = ''; } });
watch(() => props.game.id, id => { ++batchOperation; ++sourceRequest; workspace.value = usePageWorkspace<ContentWorkspace>('game-content', id, () => ({ query: '', stateFilter: 'all', sourceFilter: 'all', page: 1, selectedMods: [] })); sources.value = new Map(); sourcesReady.value = false; readingSources.value = false; sourceError.value = ''; query.value = workspace.value.query; stateFilter.value = workspace.value.stateFilter; sourceFilter.value = workspace.value.sourceFilter; modPage.value = workspace.value.page; selectedMod.value = null; mods.value = []; selectedMods.value = [...workspace.value.selectedMods]; batchDeleting.value = false; deleting.value = null; busy.value = false; tab.value = 'installed'; void refresh(); });
onMounted(() => { void refresh(); });
onUnmounted(() => { ++request; ++sourceRequest; ++batchOperation; });
function handleModListKeydown(event: KeyboardEvent) {
  if (isContextMenuKey(event)) {
    const target = event.target;
    const row = target instanceof HTMLElement ? target.closest<HTMLElement>('.mod-row') : null;
    if (!row) return;
    event.preventDefault();
    const bounds = row.getBoundingClientRect();
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: bounds.left + bounds.width / 2, clientY: bounds.top + bounds.height / 2 }));
    return;
  }
  navigateListWithArrows(event, '.mod-details-link');
}
</script>
<template>
  <div
    v-if="game.loader_version"
    ref="revealRoot"
    class="game-mods-panel"
  >
    <div
      v-if="tab === 'updates'"
      class="mods-view-bar"
    >
      <button
        class="secondary-button"
        :disabled="contentInstalling || busy"
        @click="tab = 'installed'; selectedMod = null"
      >
        <AppIcon name="arrow-left" />{{ t('gameMods.installed') }}
      </button>
    </div>
    <ModUpdates
      v-show="tab === 'updates'"
      ref="modUpdates"
      :game="game"
      :locked="locked || busy || loading"
      :revision="revision"
      @busy="updatingMods = $event"
      @recognizing="recognizingMods = $event"
      @updated="refresh()"
    />
    <InstalledModDetails
      v-if="selectedMod && tab === 'installed'"
      :mod="selectedMod"
      :busy="busy || loading || contentInstalling"
      @back="selectedMod = null"
      @versions="viewVersions"
    />
    <div
      v-show="tab === 'installed' && !selectedMod"
    >
      <p
        v-if="readingSources"
        class="setting-hint"
        role="status"
      >
        {{ t('gameMods.readingSources') }}
      </p>
      <div
        v-if="sourceError"
        class="inline-error"
        role="alert"
      >
        <p>{{ sourceError }}</p><button
          class="text-button"
          :disabled="readingSources"
          @click="readSources"
        >
          {{ t('gameMods.retrySources') }}
        </button>
      </div>
      <p
        v-if="locked && !contentInstalling"
        class="setting-hint"
        role="status"
      >
        {{ t('gameMods.locked') }}
      </p>
      <div class="content-toolbar mods-toolbar">
        <div
          v-if="$slots['content-type']"
          class="mods-content-type"
        >
          <slot name="content-type" />
        </div>
        <SearchField
          v-model="query"
          class="mods-search"
          :label="t('gameMods.search')"
          :placeholder="t('gameMods.searchPlaceholder')"
          :disabled="loading"
        />
        <PopoverRoot>
          <PopoverTrigger
            class="secondary-button mods-filter-trigger"
            :disabled="busy || loading"
            :aria-label="t('gameMods.filters')"
          >
            <AppIcon name="settings" /><span>{{ t('gameMods.filters') }}</span><span
              v-if="activeFilterCount"
              class="count"
            >{{ activeFilterCount }}</span>
          </PopoverTrigger>
          <PopoverPortal>
            <PopoverContent
              class="mods-filter-popover"
              align="end"
              :side-offset="8"
              :collision-padding="12"
            >
              <label for="mods-state-filter">{{ t('gameMods.filterState') }}</label>
              <SelectField
                id="mods-state-filter"
                v-model="stateFilter"
                :label="t('gameMods.filterState')"
                :disabled="busy || loading"
                :options="[{value:'all',label:t('gameMods.allStates')},{value:'enabled',label:t('gameMods.enabledCount',{count:enabledCount})},{value:'disabled',label:t('gameMods.disabledCount',{count:mods.length-enabledCount})}]"
              />
              <label for="mods-source-filter">{{ t('gameMods.filterSource') }}</label>
              <SelectField
                id="mods-source-filter"
                v-model="sourceFilter"
                :label="t('gameMods.filterSource')"
                :disabled="busy || loading"
                :options="[{value:'all',label:t('gameMods.allSources')},{value:'modrinth',label:'Modrinth'},{value:'curseforge',label:'CurseForge'},{value:'local',label:t('gameMods.localFiles')}]"
              />
            </PopoverContent>
          </PopoverPortal>
        </PopoverRoot>
        <div class="mods-library-actions">
          <slot name="primary-action" />
          <DropdownMenuRoot>
            <DropdownMenuTrigger as-child>
              <AppIconButton
                icon="more-horizontal"
                :label="t('gameMods.moreActions')"
                :disabled="busy"
              />
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                class="profile-menu"
                align="end"
                :side-offset="8"
                :collision-padding="12"
              >
                <DropdownMenuItem
                  class="profile-menu-item"
                  :disabled="contentInstalling || busy"
                  @select="tab = 'updates'"
                >
                  <AppIcon name="refresh-cw" />{{ t('gameMods.updates') }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  class="profile-menu-item"
                  :disabled="locked || busy || loading"
                  @select="mutate('import_game_mods')"
                >
                  <AppIcon name="folder" />{{ t('gameMods.importMods') }}
                </DropdownMenuItem>
                <DropdownMenuItem
                  v-if="$slots.history"
                  class="profile-menu-item"
                  @select="emit('history')"
                >
                  <AppIcon name="info" />{{ t('modUpdates.history') }}
                </DropdownMenuItem>
                <DropdownMenuSeparator class="profile-menu-separator" />
                <DropdownMenuItem
                  class="profile-menu-item"
                  :disabled="busy || loading"
                  @select="refresh()"
                >
                  <AppIcon name="refresh-cw" />{{ t('gameMods.refresh') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
      </div>
      <slot name="history" />
      <div
        v-if="mods.length"
        class="mods-list-summary"
      >
        <label class="mod-selection-label"><input
          type="checkbox"
          :aria-label="t('gameMods.selectAll')"
          :checked="allVisibleSelected"
          :indeterminate="filtered.some((mod) => selectedMods.includes(mod.file_name)) && !allVisibleSelected"
          :disabled="locked || busy || loading || !filtered.length"
          @change="selectVisible(($event.target as HTMLInputElement).checked)"
        ><span class="sr-only">{{ t('gameMods.selectAll') }}</span></label>
        <p class="mods-count">
          {{ hasFilter && !readingSources && !sourceError ? t('gameMods.filteredCount', {count: filtered.length, total: mods.length}) : t('gameMods.installedCount', {count: mods.length}) }}
        </p>
        <span
          v-if="loading && mods.length"
          role="status"
          class="mods-refreshing"
        ><AppIcon
          name="loader-circle"
          class="ui-busy-icon"
        />{{ t('gameMods.reading') }}</span>
      </div>
      <div
        v-if="selectedMods.length"
        class="mod-selection-toolbar installed-selection"
      >
        <span>{{ t('gameMods.selected', { count: selectedMods.length }) }}</span>
        <div class="mod-batch-actions">
          <button
            class="secondary-button"
            :disabled="locked || busy || loading"
            @click="batch('set_game_mod_enabled', true)"
          >
            {{ t('gameMods.enableSelected') }}
          </button><button
            class="secondary-button"
            :disabled="locked || busy || loading"
            @click="batch('set_game_mod_enabled', false)"
          >
            {{ t('gameMods.disableSelected') }}
          </button><button
            class="secondary-button danger-text"
            :disabled="locked || busy || loading"
            @click="batchDeleting = true; deleting = null"
          >
            {{ t('gameMods.removeSelected') }}
          </button>
          <button
            class="text-button"
            :disabled="busy"
            @click="selectedMods = []; batchDeleting = false"
          >
            {{ t('gameMods.clearSelection') }}
          </button>
        </div>
      </div>
      <div
        v-if="batchDeleting"
        class="mod-batch-confirm"
      >
        <p>{{ t('gameMods.confirmBatch', { count: selectedMods.length }) }}</p><div class="profile-form-actions">
          <button
            class="secondary-button danger-text"
            :disabled="locked || busy || loading"
            @click="batch('remove_game_mod')"
          >
            {{ t('gameMods.confirmRemoveSelected') }}
          </button><button
            class="secondary-button"
            :disabled="busy"
            @click="batchDeleting = false"
          >
            {{ t('gameMods.cancel') }}
          </button>
        </div>
      </div>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <p
        v-if="loading && !mods.length"
        class="content-empty"
        role="status"
      >
        {{ t('gameMods.reading') }}
      </p>
      <ContextMenuRoot
        v-else-if="filtered.length"
        v-model:open="contextOpen"
      >
        <ContextMenuTrigger as-child>
          <ul
            class="mod-list"
            :aria-busy="loading || readingSources"
            :aria-label="t('gameMods.local')"
            @keydown="handleModListKeydown"
            @contextmenu.capture="prepareModContext"
            @pointerdown.capture="prepareTouchContext"
          >
            <li
              v-for="mod in pageMods"
              :key="mod.file_name"
              class="mod-row"
              :class="{ 'content-recent': highlighted.includes(mod.file_name) }"
              tabindex="-1"
              :data-content-file="mod.file_name"
              :data-mod-file="mod.file_name"
            >
              <label class="mod-select-cell"><input
                class="mod-selection-checkbox"
                type="checkbox"
                :aria-label="t('gameMods.selectMod', { name: mod.name })"
                :checked="selectedMods.includes(mod.file_name)"
                :disabled="locked || busy || loading"
                @change="selectMod(mod.file_name, ($event.target as HTMLInputElement).checked)"
              ></label>
              <div
                class="mod-info"
                :class="{ 'mod-disabled': !mod.enabled }"
              >
                <h3>
                  <button
                    class="mod-details-link"
                    :aria-label="t('gameMods.viewDetails', {name: mod.name})"
                    @click="runModAction(mod, 'details')"
                  >
                    {{ mod.name }}
                  </button><span
                    v-if="!mod.enabled"
                    class="mod-state"
                  >{{ t('gameMods.disabled') }}</span>
                  <span
                    v-if="mod.pack_owned || game.pack_name"
                    class="mod-state"
                  >{{ mod.pack_owned ? t('gameMods.packOwned') : t('gameMods.userAdded') }}</span>
                </h3>
                <p class="mod-meta">
                  {{ mod.version || t('gameMods.unknownVersion') }} · {{ mod.loader || t('gameMods.unknownLoader') }}<span v-if="mod.authors.length"> · {{ mod.authors.join(locale === 'zh-CN' ? '、' : ', ') }}</span>
                </p>
              </div>
              <div class="mod-actions">
                <SwitchField
                  :model-value="mod.enabled"
                  :label="t(mod.enabled ? 'gameMods.disable' : 'gameMods.enable', { name: mod.name })"
                  :disabled="locked || busy || loading"
                  @update:model-value="runModAction(mod, 'toggle')"
                />
                <template v-if="deleting === mod.file_name">
                  <button
                    class="secondary-button"
                    :disabled="locked || busy || loading"
                    @click="remove(mod)"
                  >
                    {{ t('gameMods.confirmRemove') }}
                  </button>
                  <button
                    class="secondary-button"
                    :disabled="busy"
                    @click="deleting = null"
                  >
                    {{ t('gameMods.cancel') }}
                  </button>
                </template>
                <AppIconButton
                  v-else
                  icon="trash"
                  :label="t('gameMods.removeMod', { name: mod.name })"
                  :disabled="locked || busy || loading"
                  @click="runModAction(mod, 'remove')"
                />
              </div>
            </li>
          </ul>
        </ContextMenuTrigger>
        <ContextMenuPortal v-if="contextOpen && contextMod">
          <!-- A positioned portal gives Floating UI the scaled offset parent for app zoom. -->
          <div style="position: fixed; inset: 0; z-index: 30; pointer-events: none">
            <ContextMenuContent
              class="profile-menu"
              position-strategy="absolute"
              style="pointer-events: auto"
              :collision-padding="12"
              @close-auto-focus="returnContextFocus"
            >
              <ContextMenuItem
                v-for="action in modActions(contextMod)"
                :key="action.id"
                class="profile-menu-item"
                :class="{ 'danger-text': action.id === 'remove' }"
                :disabled="action.disabled"
                @select="contextMod && runModAction(contextMod, action.id)"
              >
                {{ action.label }}
              </ContextMenuItem>
            </ContextMenuContent>
          </div>
        </ContextMenuPortal>
      </ContextMenuRoot>
      <PluginListPagination
        v-if="filtered.length"
        :page="modPage"
        :page-count="modPageCount"
        :total="filtered.length"
        @page="modPage = $event"
      />
      <div
        v-else-if="!loading && !readingSources && !sourceError"
        class="content-empty"
      >
        <p>{{ hasFilter ? t('gameMods.noMatches') : t('gameMods.empty') }}</p>
        <button
          v-if="hasFilter"
          class="text-button"
          @click="query = ''; stateFilter = 'all'; sourceFilter = 'all'"
        >
          {{ t('common.clearFilters') }}
        </button>
      </div>
    </div>
  </div>
</template>
<style scoped>
.mod-row.content-recent{background:var(--tint);box-shadow:inset 3px 0 var(--ui-focus)}
.mods-refreshing{display:inline-flex;align-items:center;gap:8px;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}
.game-mods-panel{min-width:0}.mods-toolbar{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);margin:0!important}.mods-content-type{flex:0 0 132px;min-width:0}.mods-search{flex:1 1 220px;min-width:160px}.mods-filter-trigger{display:inline-flex;gap:var(--ui-space-2);align-items:center;min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}.mods-library-actions{display:flex;align-items:center;gap:var(--ui-space-2);margin-left:auto}.mods-library-actions :deep(.ui-icon-button){width:var(--ui-control-height);height:var(--ui-control-height)}.mods-count{flex:1;margin:0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.mods-list-summary{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);min-height:var(--ui-control-compact);margin:var(--ui-space-3) 0 var(--ui-space-1)}.mod-selection-label,.mod-select-cell{display:flex;align-items:center;justify-content:center;flex:none;width:var(--ui-control-compact);gap:var(--ui-space-2);min-height:var(--ui-control-compact);font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.mod-select-cell{min-height:var(--ui-control-height)}.mod-selection-label input,.mod-selection-checkbox{width:16px;height:16px;margin:0;accent-color:var(--ui-focus)}
.game-mods-panel .mod-selection-toolbar{display:flex;align-items:center;justify-content:space-between;flex-wrap:wrap;gap:var(--ui-space-3);margin:var(--ui-space-2) 0;padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-2);font-size:var(--ui-text-caption)}.mod-batch-actions{display:flex;flex-wrap:wrap;align-items:center;gap:var(--ui-space-2)}.mod-batch-actions button{min-height:var(--ui-control-compact);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}
.game-mods-panel .mod-list{margin:0;padding:0;border-top:1px solid var(--ui-border)}.game-mods-panel .mod-row{display:flex;align-items:center;gap:var(--ui-space-3);padding:var(--ui-space-3) 0;min-height:64px;border-bottom:1px solid var(--ui-border)}.game-mods-panel .mod-info{min-width:0;flex:1}.game-mods-panel .mod-info h3{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);margin:0;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:600}.game-mods-panel .mod-meta{margin:var(--ui-space-1) 0 0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.game-mods-panel .mod-state{font-size:var(--ui-text-caption);font-weight:400;line-height:var(--ui-leading-caption)}.mod-selection-checkbox{align-self:center;flex:none}.mod-details-link{appearance:none;border:0;background:none;color:inherit;font:inherit;padding:0;text-align:left;cursor:pointer;overflow-wrap:anywhere}.mod-details-link:hover{text-decoration:underline;text-underline-offset:3px}.mod-details-link:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset);border-radius:var(--ui-radius-sm)}
.mod-actions{display:flex;align-items:center;gap:var(--ui-space-2)}.mod-actions :deep(.ui-icon-button){width:var(--ui-control-height);height:var(--ui-control-height)}.mod-actions .secondary-button{min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}.mods-view-bar{margin:0 0 var(--ui-space-4)}.mods-view-bar button{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3)}.game-mods-panel :deep(.mod-updates){margin-top:var(--ui-space-4)}
@media(max-width:720px){.mods-content-type{flex-basis:124px}.mods-search{flex-basis:calc(100% - 132px)}.mods-library-actions{margin-left:auto}.mods-filter-trigger{margin-right:auto}.game-mods-panel .mod-row{gap:var(--ui-space-2)}.mod-batch-actions{width:100%}}
@media(max-width:460px){.mod-actions{gap:var(--ui-space-1)}.mod-actions .secondary-button{padding:0 var(--ui-space-2)}.game-mods-panel .mod-row{flex-wrap:wrap}.game-mods-panel .mod-info{min-width:160px}.mod-actions{margin-left:auto}}
</style>
<style>
.mods-filter-popover{z-index:120;display:grid;gap:var(--ui-space-2);width:min(280px,calc(100vw - 32px));padding:var(--ui-space-4);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-menu)}.mods-filter-popover>label{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.mods-filter-popover>label:not(:first-child){margin-top:var(--ui-space-2)}
</style>
