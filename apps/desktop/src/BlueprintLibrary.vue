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
import type { ResolvedCommand } from './extensions/contributions';

const props = defineProps<{ items: SavedBlueprint[]; busy: boolean; initialGameId?: string; selectedId?: string | null }>();
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; open: [id: string]; changed: [items: SavedBlueprint[]]; command: [value: ResolvedCommand] }>();
const menuOpen = ref<string | null>(null);
const editing = ref<string | null>(null);
const name = ref('');
const removing = ref<string | null>(null);
const saving = ref(false);
const error = ref('');
const importId = ref<string | null>(null);
const search = ref('');
const favoritesOnly = ref(false);
const list = ref<HTMLElement | null>(null);
watch(() => props.selectedId, async id => {
  if (!id) return;
  search.value = ''; favoritesOnly.value = false;
  await nextTick();
  const card = Array.from(list.value?.querySelectorAll<HTMLElement>('[data-blueprint-id]') ?? []).find(card => card.dataset.blueprintId === id);
  card?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' });
  card?.querySelector<HTMLButtonElement>('.blueprint-open')?.focus();
}, { flush: 'post' });
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
    <label
      class="sr-only"
      for="blueprint-library-search"
    >{{ t('blueprints.searchSaved') }}</label>
    <input
      id="blueprint-library-search"
      v-model="search"
      type="search"
      :aria-label="t('blueprints.searchSaved')"
      :placeholder="t('blueprints.searchSaved')"
    >
    <button
      v-if="search"
      class="text-button"
      type="button"
      @click="search = ''"
    >
      {{ t('blueprints.clearSearch') }}
    </button>
    <button
      class="text-button"
      type="button"
      :aria-pressed="favoritesOnly"
      @click="favoritesOnly = !favoritesOnly"
    >
      {{ t('blueprints.favorites') }}
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
  >
    <li
      v-for="item in filteredItems"
      :key="item.id"
      :data-blueprint-id="item.id"
      :class="{ 'blueprint-located': item.id === selectedId }"
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
            <DropdownMenuTrigger
              class="text-button"
              :disabled="busy || saving"
              :aria-label="t('blueprints.moreLabel', { name: item.name })"
            >
              <AppIcon
                name="more-horizontal"
                :size="18"
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
        v-if="item.source"
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
.blueprint-library{list-style:none;padding:0;margin:16px 0 0;display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,260px),1fr));gap:12px;font-size:14px}
.blueprint-library-search{display:flex;align-items:center;gap:8px;width:100%;margin-top:12px}
.blueprint-library-search input{flex:1;min-width:0;min-height:38px;padding:8px 11px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:13px}
.blueprint-library-empty{margin:16px 0 0;color:var(--muted)}
.blueprint-library li{position:relative;padding:12px;border:1px solid var(--line);border-radius:12px;min-width:0;background:var(--panel)}
.blueprint-library li.blueprint-located{border-color:var(--accent);box-shadow:0 0 0 1px var(--accent)}
.blueprint-open{display:flex;flex-wrap:wrap;gap:8px;width:100%;font-size:14px;position:static}
.blueprint-open:after{content:'';position:absolute;inset:0;border-radius:12px}
.blueprint-open:focus-visible:after{outline:2px solid var(--accent);outline-offset:2px}
.blueprint-file-art{height:112px;width:100%;display:grid;place-items:center;color:var(--muted);background:var(--bg);border-radius:8px}
.blueprint-card-name{flex:1;overflow-wrap:anywhere}.blueprint-card-format{font-size:12px;color:var(--muted)}
.blueprint-library-actions,.blueprint-library-edit,.blueprint-library-confirm,.blueprint-source,.blueprint-import{position:relative;z-index:1}
.blueprint-source{display:flex;gap:4px 8px;flex-wrap:wrap;margin-top:8px;font-size:12px;color:var(--muted);overflow-wrap:anywhere}
.blueprint-library-actions{width:100%;justify-content:flex-end}
.blueprint-library-main,.blueprint-library-actions,.blueprint-library-edit,.blueprint-library-confirm{display:flex;align-items:center;gap:8px;flex-wrap:wrap;min-width:0}
.blueprint-library-main{justify-content:space-between}
.blueprint-library-main>button{min-width:0;overflow-wrap:anywhere;text-align:left}
.blueprint-library-actions{margin-left:auto}
.blueprint-library-edit,.blueprint-library-confirm{margin-top:10px}
.blueprint-library-edit input{flex:1 1 180px;min-width:0;max-width:100%}
.blueprint-library-confirm p{flex-basis:100%;margin:0 0 4px;overflow-wrap:anywhere}
@media(max-width:560px){.blueprint-library-actions{margin-left:0}}
</style>
