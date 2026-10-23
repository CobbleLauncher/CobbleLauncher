<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem } from 'reka-ui';
import { invoke } from '@tauri-apps/api/core';
import type { Game, LocalMod } from './games';
import type { ModsActivity } from './game-mods';
import AppIcon from './ui/AppIcon.vue';
import ModUpdates from './ModUpdates.vue';
import InstalledModDetails from './InstalledModDetails.vue';
import { locale, t } from './i18n';
const props = defineProps<{ game: Game; locked: boolean; revision?: number }>();
const emit = defineEmits<{ activity: [value: ModsActivity] }>();
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
const query = ref('');
const stateFilter = ref<'all' | 'enabled' | 'disabled'>('all');
const sourceFilter = ref<'all' | 'modrinth' | 'curseforge' | 'local'>('all');
const sources = ref<Map<string, string>>(new Map());
const readingSources = ref(false);
const sourceError = ref('');
let sourceRequest = 0;
const enabledCount = computed(() => mods.value.filter(mod => mod.enabled).length);
const hasFilter = computed(() => !!query.value.trim() || stateFilter.value !== 'all' || sourceFilter.value !== 'all');
const canonicalFile = (filename: string) => filename.replace(/\.disabled$/i, '');
async function readSources() {
  const current = ++sourceRequest;
  const id = props.game.id;
  readingSources.value = true; sourceError.value = ''; sources.value = new Map();
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
  } catch { if (current === sourceRequest) sourceError.value = t('gameMods.sourcesFailed'); }
  finally { if (current === sourceRequest) readingSources.value = false; }
}
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const deleting = ref<string | null>(null);
const selectedMods = ref<string[]>([]);
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
    if (sourceFilter.value !== 'all' && (readingSources.value || sourceError.value || (sources.value.get(canonicalFile(mod.file_name)) ?? 'local') !== sourceFilter.value)) return false;
    return [mod.name, mod.file_name, mod.version, mod.loader, ...mod.authors].some(value => value?.toLocaleLowerCase().includes(search));
  });
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
watch(() => props.game.id, () => { ++batchOperation; ++sourceRequest; sources.value = new Map(); readingSources.value = false; sourceError.value = ''; stateFilter.value = 'all'; sourceFilter.value = 'all'; selectedMod.value = null; mods.value = []; selectedMods.value = []; batchDeleting.value = false; busy.value = false; query.value = ''; tab.value = 'installed'; void refresh(); }, { immediate: true });
onUnmounted(() => { ++request; ++sourceRequest; ++batchOperation; });
</script>
<template>
  <div
    v-if="game.loader_version"
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
        <input
          v-model="query"
          type="search"
          :aria-label="t('gameMods.search')"
          :placeholder="t('gameMods.searchPlaceholder')"
          :disabled="loading"
        >
        <div class="mod-filters">
          <select
            v-model="stateFilter"
            :aria-label="t('gameMods.filterState')"
            :disabled="busy || loading"
          >
            <option value="all">
              {{ t('gameMods.allStates') }}
            </option>
            <option value="enabled">
              {{ t('gameMods.enabledCount', {count: enabledCount}) }}
            </option>
            <option value="disabled">
              {{ t('gameMods.disabledCount', {count: mods.length - enabledCount}) }}
            </option>
          </select>
          <select
            v-model="sourceFilter"
            :aria-label="t('gameMods.filterSource')"
            :disabled="busy || loading"
          >
            <option value="all">
              {{ t('gameMods.allSources') }}
            </option>
            <option value="modrinth">
              Modrinth
            </option>
            <option value="curseforge">
              CurseForge
            </option>
            <option value="local">
              {{ t('gameMods.localFiles') }}
            </option>
          </select>
        </div>

        <button
          class="secondary-button"
          :disabled="busy || loading"
          :aria-label="t('gameMods.refresh')"
          @click="refresh()"
        >
          <AppIcon :name="loading ? 'loader-circle' : 'refresh-cw'" />
        </button>
        <div class="mods-library-actions">
          <button
            class="secondary-button"
            :disabled="contentInstalling || busy"
            @click="tab = 'updates'"
          >
            <AppIcon name="refresh-cw" />{{ t('gameMods.updates') }}
          </button>
          <button
            class="secondary-button"
            :disabled="locked || busy || loading"
            @click="mutate('import_game_mods')"
          >
            {{ busy ? t('gameMods.working') : t('gameMods.importMods') }}
          </button>
        </div>
      </div>
      <div
        v-if="mods.length"
        class="mod-selection-toolbar installed-selection"
      >
        <p class="mods-count">
          {{ hasFilter && !readingSources && !sourceError ? t('gameMods.filteredCount', {count: filtered.length, total: mods.length}) : t('gameMods.installedCount', {count: mods.length}) }}
        </p>
        <label class="mod-selection-label"><input
          type="checkbox"
          :aria-label="t('gameMods.selectAll')"
          :checked="allVisibleSelected"
          :indeterminate="filtered.some((mod) => selectedMods.includes(mod.file_name)) && !allVisibleSelected"
          :disabled="locked || busy || loading || !filtered.length"
          @change="selectVisible(($event.target as HTMLInputElement).checked)"
        ><span>{{ selectedMods.length ? t('gameMods.selected', { count: selectedMods.length }) : t('gameMods.select') }}</span></label>
        <div
          v-if="selectedMods.length"
          class="mod-batch-actions"
        >
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
        v-if="loading"
        class="content-empty"
        role="status"
      >
        {{ t('gameMods.reading') }}
      </p>
      <ul
        v-else-if="filtered.length"
        class="mod-list"
        :aria-label="t('gameMods.local')"
      >
        <ContextMenuRoot
          v-for="mod in filtered"
          :key="mod.file_name"
        >
          <ContextMenuTrigger as-child>
            <li class="mod-row">
              <input
                class="mod-selection-checkbox"
                type="checkbox"
                :aria-label="t('gameMods.selectMod', { name: mod.name })"
                :checked="selectedMods.includes(mod.file_name)"
                :disabled="locked || busy || loading"
                @change="selectMod(mod.file_name, ($event.target as HTMLInputElement).checked)"
              >
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
                <p
                  v-if="mod.description"
                  class="mod-description"
                >
                  {{ mod.description }}
                </p>
                <p class="mod-filename">
                  {{ mod.file_name }}
                </p>
              </div>
              <div class="mod-actions">
                <button
                  class="mod-toggle"
                  role="switch"
                  :aria-label="t('gameMods.enable', { name: mod.name })"
                  :aria-checked="mod.enabled"
                  :disabled="locked || busy || loading"
                  @click="runModAction(mod, 'toggle')"
                >
                  <span />
                </button>
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
                <button
                  v-else
                  class="secondary-button"
                  :aria-label="t('gameMods.removeMod', { name: mod.name })"
                  :disabled="locked || busy || loading"
                  @click="runModAction(mod, 'remove')"
                >
                  <AppIcon name="trash" />
                </button>
              </div>
            </li>
          </ContextMenuTrigger>
          <ContextMenuPortal>
            <ContextMenuContent
              class="profile-menu"
              :collision-padding="12"
            >
              <ContextMenuItem
                v-for="action in modActions(mod)"
                :key="action.id"
                class="profile-menu-item"
                :class="{ 'danger-text': action.id === 'remove' }"
                :disabled="action.disabled"
                @select="runModAction(mod, action.id)"
              >
                {{ action.label }}
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenuPortal>
        </ContextMenuRoot>
      </ul>
      <p
        v-else-if="!readingSources && !sourceError"
        class="content-empty"
      >
        {{ hasFilter ? t('gameMods.noMatches') : t('gameMods.empty') }}
      </p>
    </div>
  </div>
</template>
<style scoped>
.game-mods-panel .content-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;margin:18px 0 12px}.mods-count{margin:0;color:var(--muted);font-size:12px}.mods-library-actions{display:flex;gap:8px;flex-wrap:wrap;justify-content:flex-end}.mods-library-actions button,.mods-view-bar button{display:inline-flex;align-items:center;gap:7px;min-height:36px;padding:8px 12px}.mods-view-bar{margin-top:18px}.game-mods-panel .content-toolbar{margin:12px 0}.game-mods-panel .content-toolbar button{display:inline-flex;align-items:center;justify-content:center;min-width:38px;min-height:38px}.mod-actions{display:flex;align-items:center;gap:10px}.mod-actions .secondary-button{display:inline-flex;align-items:center;justify-content:center;min-width:34px;min-height:34px;padding:7px 10px}.mod-batch-actions{display:flex;flex-wrap:wrap;gap:7px}.mod-batch-actions button{min-height:32px;padding:6px 10px;font-size:12px}.game-mods-panel .mod-selection-toolbar{flex-wrap:wrap;gap:12px}.game-mods-panel .mod-info{min-width:0}.game-mods-panel .mod-row{gap:12px}.game-mods-panel :deep(.mod-updates){margin-top:16px}
@media(max-width:560px){.game-mods-panel .content-heading{align-items:flex-start}.mods-library-actions{gap:6px}.mods-library-actions button{font-size:12px;padding:7px 10px}.mod-actions{gap:7px}.game-mods-panel .mod-row{flex-wrap:wrap}.game-mods-panel .mod-row .mod-info{flex:1;min-width:160px}.mod-batch-actions{width:100%}}
</style>
<style scoped>
.mod-details-link{appearance:none;border:0;background:none;color:inherit;font:inherit;padding:0;text-align:left;cursor:pointer;overflow-wrap:anywhere}.mod-details-link:hover{text-decoration:underline}.mod-details-link:focus-visible{outline:2px solid var(--accent);outline-offset:4px;border-radius:3px}
.mod-filters{display:flex;flex-wrap:wrap;gap:var(--ui-space-2);margin:0}.mod-filters select{min-width:0;min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label);max-width:100%}
</style>

<style scoped>
.mods-toolbar{display:flex;align-items:center;flex-wrap:wrap;gap:8px}.mods-toolbar>input{flex:1 1 180px;min-width:120px}.mods-toolbar .mods-library-actions{margin-left:auto}.mods-count{font-size:14px}.mods-library-actions button,.mods-view-bar button{min-height:40px}.mods-toolbar :deep(select){font-size:14px}.mod-batch-actions button{min-height:36px}
</style>
