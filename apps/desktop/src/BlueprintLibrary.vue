<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Star } from '@lucide/vue';
import { t } from './i18n';
import type { SavedBlueprint } from './blueprints';
import BlueprintImportPanel from './BlueprintImportPanel.vue';
import BlueprintThumbnail from './BlueprintThumbnail.vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import ExtensionMenuItems from './ExtensionMenuItems.vue';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import SearchField from './ui/SearchField.vue';
import type { ResolvedCommand } from './extensions/contributions';
import { usePageWorkspace } from './use-page-workspace';
import { isContextMenuKey, navigateListWithArrows } from './list-keyboard';

const props = defineProps<{ items: SavedBlueprint[]; busy: boolean; initialGameId?: string; selectedId?: string | null }>();
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; open: [id: string]; changed: [items: SavedBlueprint[]]; command: [value: ResolvedCommand] }>();
const menuOpen = ref<string | null>(null);
const editing = ref<string | null>(null);
const name = ref('');
const removing = ref<string | null>(null);
const saving = ref(false);
const error = ref('');
const importId = ref<string | null>(null);
const workspace = usePageWorkspace('blueprint-library', props.initialGameId, () => ({ search: '', favoritesOnly: false }));
const search = ref(workspace.search);
const favoritesOnly = ref(workspace.favoritesOnly);
const list = ref<HTMLElement | null>(null);
watch(search, value => { workspace.search = value; });
watch(favoritesOnly, value => { workspace.favoritesOnly = value; });
watch([() => props.selectedId, () => props.items], async ([id]) => {
  if (!id) return;
  search.value = ''; favoritesOnly.value = false;
  await nextTick();
  const card = Array.from(list.value?.querySelectorAll<HTMLElement>('[data-blueprint-id]') ?? []).find(card => card.dataset.blueprintId === id);
  card?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' });
  card?.querySelector<HTMLButtonElement>('.blueprint-open')?.focus();
}, { flush: 'post', immediate: true });
function handleListKeydown(event: KeyboardEvent) {
  if (isContextMenuKey(event)) {
    if (props.busy || saving.value || (event.target instanceof HTMLElement && event.target.closest('input,textarea,select'))) return;
    const target = event.target;
    const row = target instanceof HTMLElement ? target.closest('li[data-blueprint-id]') : null;
    const trigger = row?.querySelector<HTMLButtonElement>('.blueprint-more');
    if (!trigger) return;
    event.preventDefault();
    menuOpen.value = row?.getAttribute('data-blueprint-id') ?? null;
    return;
  }
  navigateListWithArrows(event, '.blueprint-open');
}
function openContextMenu(event: MouseEvent, id: string) {
  if (props.busy || saving.value || (event.target instanceof HTMLElement && event.target.closest('input,textarea,select'))) return;
  event.preventDefault(); menuOpen.value = id;
}
async function favorite(item: SavedBlueprint) {
  if (saving.value) return;
  saving.value = true; error.value = '';
  try { emit('changed', await invoke<SavedBlueprint[]>('set_blueprint_favorite', { id: item.id, favorite: !item.favorite })); }
  catch (cause) { error.value = String(cause); }
  finally { saving.value = false; }
}
const filteredItems = computed(() => {
  const query = search.value.trim().toLocaleLowerCase();
  return props.items.filter(item => (!favoritesOnly.value || item.favorite) && (!query || [item.name, item.source?.author, item.source?.provider, item.format].some(value => value?.toLocaleLowerCase().includes(query))));
});
const validName = computed(() => {
  const value = name.value.trim();
  return value.length > 0 && value.length <= 120 && !/\p{Cc}/u.test(value);
});

function startRename(item: SavedBlueprint) {
  editing.value = item.id;
  name.value = item.name;
  removing.value = null;
  error.value = '';
}
function cancelRename() { editing.value = null; error.value = ''; }
async function rename(item: SavedBlueprint) {
  const nextName = name.value.trim();
  if (!validName.value || saving.value) return;
  saving.value = true; error.value = '';
  try {
    const items = await invoke<SavedBlueprint[]>('rename_blueprint', { id: item.id, name: nextName });
    emit('changed', items);
    editing.value = null;
  } catch (cause) { error.value = String(cause); }
  finally { saving.value = false; }
}
async function remove(item: SavedBlueprint) {
  if (saving.value) return;
  saving.value = true; error.value = '';
  try {
    const items = await invoke<SavedBlueprint[]>('remove_blueprint', { id: item.id });
    emit('changed', items);
    removing.value = null;
  } catch (cause) { error.value = String(cause); }
  finally { saving.value = false; }
}
</script>

<template>
  <div
    v-if="items.length"
    class="blueprint-library-search"
  >
    <SearchField
      id="blueprint-library-search"
      v-model="search"
      class="blueprint-search-field"
      :label="t('blueprints.searchSaved')"
      :placeholder="t('blueprints.searchSaved')"
      :clear-label="t('blueprints.clearSearch')"
    />
    <button
      class="text-button"
      type="button"
      :aria-pressed="favoritesOnly"
      @click="favoritesOnly = !favoritesOnly"
    >
      <Star
        :size="16"
        :stroke-width="1.75"
        aria-hidden="true"
      />{{ t('blueprints.favorites') }}
    </button>
  </div>
  <p
    v-if="items.length && !filteredItems.length"
    class="blueprint-library-empty"
  >
    {{ t('blueprints.noSearchResults') }}
  </p>
  <p
    v-if="error && !editing && !removing && !menuOpen"
    class="settings-error"
    role="alert"
  >
    {{ error }}
  </p>
  <ul
    ref="list"
    class="blueprint-library"
    @keydown="handleListKeydown"
  >
    <li
      v-for="item in filteredItems"
      :key="item.id"
      :data-blueprint-id="item.id"
      :class="{ 'blueprint-located': item.id === selectedId }"
      @contextmenu="openContextMenu($event, item.id)"
    >
      <div class="blueprint-library-main">
        <button
          class="text-button blueprint-open"
          :disabled="busy"
          @click="emit('open', item.id)"
        >
          <BlueprintThumbnail
            :id="item.id"
            :available="item.hasThumbnail"
          />
          <span class="blueprint-card-name">{{ item.name }}</span>
          <span class="blueprint-card-format">.{{ item.format ?? 'schem' }}</span>
        </button>
        <div class="blueprint-library-actions">
          <button
            class="text-button"
            :disabled="busy || saving"
            :aria-pressed="Boolean(item.favorite)"
            :aria-label="t(item.favorite ? 'blueprints.unfavorite' : 'blueprints.favorite')"
            :title="t(item.favorite ? 'blueprints.unfavorite' : 'blueprints.favorite')"
            @click="favorite(item)"
          >
            <Star
              :size="16"
              :stroke-width="1.75"
              :fill="item.favorite ? 'currentColor' : 'none'"
              aria-hidden="true"
            />
          </button>
          <button
            class="text-button"
            :disabled="busy"
            :aria-expanded="importId === item.id"
            @click="importId = importId === item.id ? null : item.id"
          >
            {{ t('blueprints.importToGame') }}
          </button>
          <DropdownMenuRoot
            :open="menuOpen === item.id"
            @update:open="menuOpen = $event ? item.id : null"
          >
            <DropdownMenuTrigger as-child>
              <AppIconButton
                class="blueprint-more"
                icon="more-horizontal"
                :disabled="busy || saving"
                :label="t('blueprints.moreLabel', { name: item.name })"
              />
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                class="profile-menu"
                align="end"
                :side-offset="6"
                :collision-padding="12"
              >
                <DropdownMenuItem
                  class="profile-menu-item"
                  @select="startRename(item)"
                >
                  <AppIcon name="pencil" />{{ t('blueprints.rename') }}
                </DropdownMenuItem>
                <ExtensionMenuItems
                  position="blueprint"
                  :context="{ blueprintId: item.id }"
                  :disabled="busy || saving"
                  @selected="menuOpen = null; emit('command', $event)"
                  @error="error = $event"
                />
                <DropdownMenuItem
                  class="profile-menu-item danger-text"
                  @select="removing = item.id; editing = null; error = ''"
                >
                  <AppIcon name="trash" />{{ t('blueprints.remove') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
      </div>
      <div
        v-if="item.source && !item.source.provider.startsWith('generated:')"
        class="blueprint-source"
      >
        <span>{{ item.source.author }} · {{ item.source.provider }}</span>
        <span>{{ t(item.source.attribution === 'original' ? 'blueprintOnline.original' : item.source.attribution === 'repost' ? 'blueprintOnline.repost' : 'blueprintOnline.unknownAttribution') }}</span>
        <span v-if="item.source.license">{{ item.source.license }}</span>
      </div>
      <BlueprintImportPanel
        v-if="importId === item.id"
        :key="item.id"
        :blueprint-id="item.id"
        :format="item.format"
        :initial-game-id="initialGameId"
        @market="emit('market', $event)"
      />
      <form
        v-if="editing === item.id"
        class="blueprint-library-edit"
        @submit.prevent="rename(item)"
      >
        <label
          class="sr-only"
          :for="`blueprint-name-${item.id}`"
        >{{ t('blueprints.name') }}</label>
        <input
          :id="`blueprint-name-${item.id}`"
          v-model="name"
          :disabled="saving"
          maxlength="120"
        >
        <button
          class="secondary-button"
          type="submit"
          :disabled="saving || !validName"
        >
          {{ t('blueprints.saveName') }}
        </button>
        <button
          class="text-button"
          type="button"
          :disabled="saving"
          @click="cancelRename"
        >
          {{ t('blueprints.cancel') }}
        </button>
      </form>
      <div
        v-if="removing === item.id"
        class="blueprint-library-confirm"
      >
        <p>{{ t('blueprints.removeConfirm', { name: item.name }) }}</p>
        <button
          class="secondary-button danger-text"
          :disabled="saving"
          @click="remove(item)"
        >
          {{ t('blueprints.confirmRemove') }}
        </button>
        <button
          class="text-button"
          :disabled="saving"
          @click="removing = null; error = ''"
        >
          {{ t('blueprints.cancel') }}
        </button>
      </div>
      <p
        v-if="error && (editing === item.id || removing === item.id || menuOpen === item.id)"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
    </li>
  </ul>
</template>
<style scoped>
.blueprint-library{list-style:none;padding:0;margin:var(--ui-space-4) 0 0;display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,260px),1fr));gap:var(--ui-space-4);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.blueprint-library-search{display:flex;align-items:center;gap:var(--ui-space-2);width:100%;margin-top:var(--ui-space-3)}.blueprint-search-field{flex:1;min-width:0}.blueprint-library-search>.text-button{min-height:var(--ui-control-height);padding:0 var(--ui-space-3)}.blueprint-library-search>.text-button[aria-pressed=true]{background:var(--ui-surface-2);color:var(--ui-text)}.blueprint-library-empty{margin:var(--ui-space-4) 0;color:var(--ui-text-muted);font-size:var(--ui-text-body)}.blueprint-library li{position:relative;padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);min-width:0;background:var(--ui-surface-1)}.blueprint-library li.blueprint-located{border-color:var(--ui-focus);box-shadow:0 0 0 1px var(--ui-focus)}.blueprint-open{display:flex;flex-wrap:wrap;gap:var(--ui-space-2);width:100%;padding:0;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);position:static}.blueprint-open:after{content:'';position:absolute;inset:0;border-radius:var(--ui-radius-lg)}.blueprint-open:focus-visible:after{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}.blueprint-open :deep(.blueprint-thumbnail){border-radius:var(--ui-radius-md);background:var(--ui-surface-2)}.blueprint-card-name{flex:1;overflow-wrap:anywhere;font-weight:600}.blueprint-card-format{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.blueprint-library-actions,.blueprint-library-edit,.blueprint-library-confirm,.blueprint-source,.blueprint-import{position:relative;z-index:1}.blueprint-source{display:flex;gap:var(--ui-space-1) var(--ui-space-2);flex-wrap:wrap;margin-top:var(--ui-space-2);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);overflow-wrap:anywhere}.blueprint-library-main,.blueprint-library-actions,.blueprint-library-edit,.blueprint-library-confirm{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap;min-width:0}.blueprint-library-main{justify-content:space-between}.blueprint-library-main>button{min-width:0;overflow-wrap:anywhere;text-align:left}.blueprint-library-actions{width:100%;justify-content:flex-end;margin-left:auto}.blueprint-library-actions>.text-button{min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}.blueprint-library-actions>.text-button:first-child{width:var(--ui-control-height);padding:0}.blueprint-library-edit,.blueprint-library-confirm{margin-top:var(--ui-space-3)}.blueprint-library-edit input{flex:1 1 180px;min-width:0;max-width:100%;min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit}.blueprint-library-confirm p{flex-basis:100%;margin:0 0 var(--ui-space-1);overflow-wrap:anywhere}.blueprint-library-edit button,.blueprint-library-confirm button{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}
</style>
