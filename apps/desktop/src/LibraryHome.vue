<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenuRoot, ContextMenuTrigger, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui';
import GameCard from './GameCard.vue';
import AppIcon from './ui/AppIcon.vue';
import { effectiveProfile, type ProfileLibrary } from './games';
import { homeGames, searchHomeGames, type HomeGame } from './library-home';
import { filterOrganizedGames, organizationOptions, readLibraryOrganization, searchOrganization, updateGameOrganization, writeLibraryOrganization } from './library-organization';
import { defaultHomeLayout, moveHomeSection, readHomeLayout, writeHomeLayout, type HomeSection } from './home-layout';
import type { CommandAction } from './commands/registry';
import { ArrowUp, ArrowDown } from '@lucide/vue';
import SwitchField from './ui/SwitchField.vue';
import SelectField from './ui/SelectField.vue';
import SearchField from './ui/SearchField.vue';
import { savedMarketItems } from './market-saved';
import { t } from './i18n';

const props = defineProps<{ editHome?: boolean;  games: HomeGame[]; profileLibrary: ProfileLibrary; starting: string[]; retrying: string[]; quickActions?: CommandAction[]; quickActionsLoading?: boolean; loading?: boolean; error?: string }>();
defineEmits<{ play: [id: string]; manage: [id: string]; select: [id: string]; retry: [id: string]; retrySessionRecovery: [id: string]; create: []; importPack: []; importLauncher: []; receiveGame: []; reload: []; finishEditing: []; savedMarket: [collection: 'favorite' | 'later'] }>();
const all = ref(false);
const organization = ref(readLibraryOrganization());
const layout = ref(readHomeLayout());
function refreshLayout() { layout.value = readHomeLayout(); }
onMounted(() => window.addEventListener('cobble-home-layout-changed', refreshLayout));
onBeforeUnmount(() => window.removeEventListener('cobble-home-layout-changed', refreshLayout));
const editing = ref(props.editHome ?? false);
watch(() => props.editHome, value => { editing.value = !!value; });
const archived = ref(false);
const groupFilter = ref('');
const tagFilters = ref<string[]>([]);
const selected = ref<string[]>([]);
const groupDraft = ref('');
const tagsDraft = ref('');
const quickSearch = ref('');
const options = computed(() => organizationOptions(props.games, organization.value));
const groupOptions = computed(() => [{ value: '__all__', label: t('home.allGroups') }, ...options.value.groups.map(group => ({ value: group, label: group }))]);
const groupFilterValue = computed({ get: () => groupFilter.value || '__all__', set: (value: string) => { groupFilter.value = value === '__all__' ? '' : value; } });
const savedCounts = computed(() => ({ favorite: savedMarketItems.value.filter(item => item.collection === 'favorite').length, later: savedMarketItems.value.filter(item => item.collection === 'later').length }));
const quickActions = computed(() => (props.quickActions ?? []).filter(action => action.enabled !== false).map((action, index) => ({ action, index })).sort((a, b) => Number(!!b.action.quickEntry) - Number(!!a.action.quickEntry) || a.index - b.index).map(({ action }) => action));
const quickMatches = computed(() => {
  const query = quickSearch.value.trim().toLocaleLowerCase();
  return query ? quickActions.value.filter(action => action.label.toLocaleLowerCase().includes(query) || action.keywords?.some(keyword => keyword.toLocaleLowerCase().includes(query))) : quickActions.value;
});
const quickCandidates = computed(() => quickMatches.value.slice(0, 20));
const unavailableQuickIds = computed(() => layout.value.quickIds.filter(id => !quickActions.value.some(action => action.id === id)));
const quickEntries = computed(() => layout.value.quickIds.map(id => quickActions.value.find(action => action.id === id)).filter((action): action is CommandAction => !!action).slice(0, 6));
const runningQuick = ref<string | null>(null);
const quickError = ref(false);
async function runQuick(action: CommandAction) {
  if (runningQuick.value || action.enabled === false) return;
  runningQuick.value = action.id; quickError.value = false;
  try { await action.run(); } catch { quickError.value = true; }
  finally { runningQuick.value = null; }
}
function removeUnavailableQuick() { setLayout({ ...layout.value, quickIds: layout.value.quickIds.filter(id => !unavailableQuickIds.value.includes(id)) }); }
const otherGames = computed(() => selection.value.ordered.filter(game => game.id !== selection.value.featured?.id).slice(0, 4));
function saveOrganization(changes: { group?: string; tags?: string[]; archived?: boolean }) {
  for (const id of selected.value) organization.value = updateGameOrganization(organization.value, id, changes);
  writeLibraryOrganization(organization.value);
  selected.value = [];
}
function applyLabels() {
  const changes = { ...(groupDraft.value.trim() ? { group: groupDraft.value } : {}), ...(tagsDraft.value.trim() ? { tags: tagsDraft.value.split(',') } : {}) };
  if (!Object.keys(changes).length) return;
  saveOrganization(changes);
  groupDraft.value = ''; tagsDraft.value = '';
}
function toggleSelected(id: string) { selected.value = selected.value.includes(id) ? selected.value.filter(value => value !== id) : [...selected.value, id]; }
function setLayout(next: typeof layout.value) { layout.value = next; writeHomeLayout(next); }
function resetLayout() { setLayout({ order: [...defaultHomeLayout.order], hidden: [], quickIds: [], libraryView: 'list' }); }
function moveSection(section: HomeSection, direction: -1 | 1) { setLayout(moveHomeSection(layout.value, section, direction)); }
function toggleSection(section: HomeSection) { setLayout({ ...layout.value, hidden: layout.value.hidden.includes(section) ? layout.value.hidden.filter(value => value !== section) : [...layout.value.hidden, section] }); }
function moveQuick(id: string, direction: -1 | 1) {
  const ids = [...layout.value.quickIds], index = ids.indexOf(id), target = index + direction;
  if (target < 0 || target >= ids.length) return;
  [ids[index], ids[target]] = [ids[target]!, ids[index]!];
  setLayout({ ...layout.value, quickIds: ids });
}
function toggleQuick(id: string) {
  const ids = layout.value.quickIds.includes(id) ? layout.value.quickIds.filter(value => value !== id) : [...layout.value.quickIds, id].slice(0, 6);
  setLayout({ ...layout.value, quickIds: ids });
}
function dragSection(event: DragEvent, section: HomeSection) { event.dataTransfer?.setData('text/plain', section); }
function dropSection(event: DragEvent, target: HomeSection) {
  const source = event.dataTransfer?.getData('text/plain') as HomeSection | undefined;
  if (!source || !layout.value.order.includes(source) || source === target) return;
  const order = layout.value.order.filter(value => value !== source);
  order.splice(order.indexOf(target), 0, source);
  setLayout({ ...layout.value, order });
}
const search = ref('');
const page = ref(0);
const pageSize = 50;
const updates = ref(new Map<string, Pick<HomeGame, 'pinned' | 'cover_ref'>>());
watch(() => props.games.map(game => `${game.id}:${game.pinned}:${game.cover_ref}`).join('|'), () => updates.value.clear());
const selection = computed(() => homeGames(filterOrganizedGames(props.games.map(game => ({ ...game, ...updates.value.get(game.id) })), organization.value)));
function updateAppearance(game: HomeGame) { updates.value.set(game.id, { pinned: game.pinned, cover_ref: game.cover_ref }); }
const covers = ref(new Map<string, string>());
let coverRevision = 0;
watch(() => selection.value.featured?.cover_ref, async () => {
  const game = selection.value.featured;
  const revision = ++coverRevision;
  if (!game?.cover_ref || covers.value.has(game.cover_ref)) return;
  try {
    const results = await invoke<{ cover_ref: string; data_url: string }[]>('get_game_covers', { ids: [game.id] });
    if (revision !== coverRevision) return;
    for (const result of results) if (result.data_url.startsWith('data:image/png;base64,')) covers.value.set(result.cover_ref, result.data_url);
    while (covers.value.size > 8) covers.value.delete(covers.value.keys().next().value!);
  } catch { /* Keep the original artwork if a cover is unavailable. */ }
}, { immediate: true });
const organizedGames = computed(() => homeGames(filterOrganizedGames(props.games.map(game => ({ ...game, ...updates.value.get(game.id) })), organization.value, { archived: archived.value, group: groupFilter.value, tags: tagFilters.value })).ordered);
const filtered = computed(() => {
  const games = organizedGames.value;
  if (!search.value.trim()) return games;
  const regular = new Set(searchHomeGames(games, search.value).map(game => game.id));
  return games.filter(game => regular.has(game.id) || searchOrganization(game, organization.value, search.value));
});
const pageGames = computed(() => filtered.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
const sharedContextOpen = ref(false);
const contextGameId = ref('');
function prepareGameContext(event: MouseEvent) {
  const row = event.target instanceof Element ? event.target.closest<HTMLElement>('.library-game') : null;
  const game = pageGames.value.find(game => game.id === row?.dataset.gameId);
  if (!game || game.status === 'installing') { event.preventDefault(); event.stopPropagation(); return; }
  contextGameId.value = game.id;
}
function prepareTouchContext(event: PointerEvent) { if (event.pointerType === 'touch' || event.pointerType === 'pen') prepareGameContext(event); }
watch([search, page, all, archived, groupFilter, tagFilters], () => { sharedContextOpen.value = false; contextGameId.value = ''; });
const visible = computed(() => all.value ? pageGames.value : [...(selection.value.featured ? [selection.value.featured] : []), ...selection.value.recent]);
watch([search, archived, groupFilter, tagFilters], () => { page.value = 0; selected.value = []; });
watch(() => filtered.value.length, count => { page.value = Math.min(page.value, Math.max(0, Math.ceil(count / pageSize) - 1)); });
function toggleAll() { all.value = !all.value; page.value = 0; search.value = ''; }

const icons = ref(new Map<string, string>());
let iconRevision = 0;
watch(() => visible.value.filter(game => game.icon_ref).map(game => `${game.id}:${game.icon_ref}`).join('|'), async () => {
  const revision = ++iconRevision;
  const ids = visible.value.filter(game => game.icon_ref && !icons.value.has(game.icon_ref)).map(game => game.id).slice(0, 64);
  if (!ids.length) return;
  try {
    const results = await invoke<{ id: string; icon_ref: string; data_url: string }[]>('get_game_icons', { ids });
    if (revision !== iconRevision) return;
    for (const result of results) if (result.data_url.startsWith('data:image/png;base64,')) icons.value.set(result.icon_ref, result.data_url);
    while (icons.value.size > 128) {
      const oldest = icons.value.keys().next().value;
      if (!oldest) break;
      icons.value.delete(oldest);
    }
  } catch { /* An unavailable thumbnail keeps the selected preset visible. */ }
}, { immediate: true });
onBeforeUnmount(() => { iconRevision++; coverRevision++; });
function iconUrl(game: HomeGame) { return game.icon_ref ? icons.value.get(game.icon_ref) : undefined; }
</script>

<template>
  <div class="library-home">
    <header class="home-heading">
      <h1>{{ t('home.title') }}</h1>
      <div class="home-heading-actions">
        <button
          v-if="editing && !all"
          class="text-button home-organize"
          @click="toggleAll"
        >
          {{ t('home.all') }}
        </button>
        <button
          v-if="editing"
          class="secondary-button home-edit-done"
          @click="editing = false; $emit('finishEditing')"
        >
          {{ t('home.done') }}
        </button>
        <DropdownMenuRoot>
          <DropdownMenuTrigger class="secondary-button home-add">
            <AppIcon name="plus" />{{ t('home.add') }}<AppIcon
              name="chevron-down"
              :size="14"
            />
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent
              class="profile-menu home-add-menu"
              align="end"
              :side-offset="7"
              :collision-padding="12"
            >
              <DropdownMenuItem
                class="profile-menu-item"
                @select="$emit('create')"
              >
                <AppIcon name="plus" />{{ t('home.create') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                @select="$emit('importPack')"
              >
                <AppIcon name="puzzle" />{{ t('home.importPack') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                @select="$emit('importLauncher')"
              >
                <AppIcon name="folder" />{{ t('home.importLauncher') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                @select="$emit('receiveGame')"
              >
                <AppIcon name="download" />{{ t('home.receiveGame') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </div>
    </header>
    <div
      v-if="loading && !games.length"
      class="home-empty"
      role="status"
      :aria-label="t('navigation.loading')"
    >
      <AppIcon
        name="loader-circle"
        class="ui-busy-icon"
        :size="24"
      />
    </div>
    <section
      v-else-if="error && !games.length"
      class="home-empty"
    >
      <p role="alert">
        {{ error }}
      </p>
      <button
        class="secondary-button"
        @click="$emit('reload')"
      >
        <AppIcon name="refresh-cw" />{{ t('settingsLayout.retry') }}
      </button>
    </section>
    <section
      v-else-if="!games.length"
      class="home-empty"
    >
      <div
        class="home-empty-icon"
        aria-hidden="true"
      >
        <AppIcon
          name="puzzle"
          :size="34"
        />
      </div>
      <h2>{{ t('home.empty') }}</h2>
      <button
        class="primary-button"
        @click="$emit('create')"
      >
        <AppIcon name="plus" />{{ t('home.create') }}
      </button>
    </section>
    <template v-else-if="all">
      <button
        class="text-button home-view-back"
        @click="toggleAll"
      >
        <AppIcon name="arrow-left" />{{ t('home.recent') }}
      </button>
      <SearchField
        v-model="search"
        class="home-search"
        :placeholder="t('home.search')"
        :label="t('home.search')"
      >
        <span
          role="status"
          :aria-label="t('home.results', { count: filtered.length })"
        >{{ filtered.length }}</span>
      </SearchField>
      <div class="home-filters">
        <div class="home-view-choice">
          <button
            class="text-button"
            :aria-pressed="layout.libraryView === 'list'"
            @click="setLayout({ ...layout, libraryView: 'list' })"
          >
            {{ t('home.listView') }}
          </button><button
            class="text-button"
            :aria-pressed="layout.libraryView === 'cards'"
            @click="setLayout({ ...layout, libraryView: 'cards' })"
          >
            {{ t('home.cardView') }}
          </button>
        </div>
        <label><input
          v-model="archived"
          type="checkbox"
        >{{ t('home.archived') }}</label>
        <SelectField
          id="home-group-filter"
          v-model="groupFilterValue"
          :options="groupOptions"
          :label="t('home.group')"
        />
        <PopoverRoot v-if="options.tags.length">
          <PopoverTrigger class="text-button home-tag-trigger">
            {{ t('home.tags') }}<span v-if="tagFilters.length"> ({{ tagFilters.length }})</span>
          </PopoverTrigger>
          <PopoverPortal>
            <PopoverContent
              class="home-tag-popover"
              align="start"
              :side-offset="6"
              :collision-padding="8"
            >
              <label
                v-for="tag in options.tags"
                :key="tag"
              ><input
                v-model="tagFilters"
                type="checkbox"
                :value="tag"
              >{{ tag }}</label>
            </PopoverContent>
          </PopoverPortal>
        </PopoverRoot>
      </div>
      <div
        v-if="selected.length"
        class="home-batch"
        role="group"
        :aria-label="t('home.selected', { count: selected.length })"
      >
        <span>{{ t('home.selected', { count: selected.length }) }}</span>
        <input
          v-model="groupDraft"
          maxlength="40"
          :aria-label="t('home.group')"
          :placeholder="t('home.group')"
        >
        <input
          v-model="tagsDraft"
          :aria-label="t('home.tags')"
          :placeholder="t('home.tagsComma')"
        >
        <button
          class="secondary-button"
          @click="applyLabels"
        >
          {{ t('home.applyLabels') }}
        </button>
        <details class="home-batch-clear">
          <summary>{{ t('home.clearLabels') }}</summary>
          <button
            class="text-button"
            @click="saveOrganization({ group: '' })"
          >
            {{ t('home.clearGroup') }}
          </button>
          <button
            class="text-button"
            @click="saveOrganization({ tags: [] })"
          >
            {{ t('home.clearTags') }}
          </button>
        </details>
        <button
          class="secondary-button"
          @click="saveOrganization({ archived: !archived })"
        >
          {{ archived ? t('home.restore') : t('home.archive') }}
        </button>
        <button
          class="text-button"
          @click="selected = []"
        >
          {{ t('home.clearSelection') }}
        </button>
      </div>
      <ContextMenuRoot
        v-if="pageGames.length"
        v-model:open="sharedContextOpen"
      >
        <ContextMenuTrigger as-child>
          <div
            class="home-game-list"
            :class="{ 'home-game-list--grid': layout.libraryView === 'cards' }"
            @contextmenu.capture="prepareGameContext"
            @pointerdown.capture="prepareTouchContext"
          >
            <div
              v-for="game in pageGames"
              :key="game.id"
              class="home-select-row"
            >
              <label class="home-select"><input
                type="checkbox"
                :checked="selected.includes(game.id)"
                :aria-label="t('home.selectGame', { name: game.name })"
                @change="toggleSelected(game.id)"
              ></label>
              <GameCard
                :game="game"
                shared-context
                :shared-context-open="sharedContextOpen && contextGameId === game.id"
                variant="row"
                :profile="effectiveProfile(game, profileLibrary)"
                :starting="starting.includes(game.id)"
                :retrying="retrying.includes(game.id)"
                :icon-url="iconUrl(game)"
                @play="$emit('play', $event)"
                @manage="$emit('manage', $event)"
                @retry="$emit('retry', $event)"
                @retry-session-recovery="$emit('retrySessionRecovery', $event)"
                @appearance="updateAppearance"
                @select="$emit('select', $event)"
              />
              <div
                v-if="organization[game.id]?.group || organization[game.id]?.tags.length"
                class="home-item-labels"
              >
                <span v-if="organization[game.id]?.group">{{ organization[game.id]?.group }}</span><span
                  v-for="tag in organization[game.id]?.tags"
                  :key="tag"
                >{{ tag }}</span>
              </div>
            </div>
          </div>
        </ContextMenuTrigger>
      </ContextMenuRoot>
      <div
        v-else
        class="home-no-results"
      >
        <p role="status">
          {{ t('home.noMatches') }}
        </p><button
          class="text-button"
          @click="search = ''"
        >
          {{ t('home.clearSearch') }}
        </button>
      </div>
      <div
        v-if="filtered.length > pageSize"
        class="home-pagination"
      >
        <span>{{ t('home.range', { first: page * pageSize + 1, last: Math.min((page + 1) * pageSize, filtered.length), total: filtered.length }) }}</span><button
          class="text-button"
          :disabled="page === 0"
          @click="page--"
        >
          {{ t('home.previous') }}
        </button><button
          class="text-button"
          :disabled="(page + 1) * pageSize >= filtered.length"
          @click="page++"
        >
          {{ t('home.next') }}
        </button>
      </div>
    </template>
    <template v-else>
      <GameCard
        v-if="selection.featured"
        :game="selection.featured"
        :profile="effectiveProfile(selection.featured, profileLibrary)"
        :starting="starting.includes(selection.featured.id)"
        :retrying="retrying.includes(selection.featured.id)"
        :icon-url="iconUrl(selection.featured)"
        :cover-url="selection.featured.cover_ref ? covers.get(selection.featured.cover_ref) : undefined"
        @play="$emit('play', $event)"
        @manage="$emit('manage', $event)"
        @retry="$emit('retry', $event)"
        @retry-session-recovery="$emit('retrySessionRecovery', $event)"
        @appearance="updateAppearance"
        @select="$emit('select', $event)"
      />
      <div
        v-else-if="!selection.ordered.length"
        class="home-no-results"
      >
        <p>{{ t('home.noActiveGames') }}</p><button
          class="text-button"
          @click="toggleAll(); archived = true"
        >
          {{ t('home.viewArchived') }}
        </button>
      </div>
      <div
        v-if="editing"
        class="home-layout-edit"
        role="group"
        :aria-label="t('home.editHome')"
      >
        <div
          v-for="(section, index) in layout.order"
          :key="section"
          draggable="true"
          @dragstart="dragSection($event, section)"
          @dragover.prevent
          @drop.prevent="dropSection($event, section)"
        >
          <span>{{ t(`home.sections.${section}`) }}</span><SwitchField
            :model-value="!layout.hidden.includes(section)"
            :label="t(`home.sections.${section}`)"
            @update:model-value="toggleSection(section)"
          />
          <button
            class="text-button"
            :disabled="index === 0"
            :aria-label="t('home.moveUp', { name: t(`home.sections.${section}`) })"
            @click="moveSection(section, -1)"
          >
            <ArrowUp :size="16" />
          </button>
          <button
            class="text-button"
            :disabled="index === layout.order.length - 1"
            :aria-label="t('home.moveDown', { name: t(`home.sections.${section}`) })"
            @click="moveSection(section, 1)"
          >
            <ArrowDown :size="16" />
          </button>
        </div>
        <div
          v-if="quickActions.length"
          class="home-quick-picker"
        >
          <div class="home-quick-picker-head">
            <input
              v-model="quickSearch"
              type="search"
              :aria-label="t('home.quickSearch')"
              :placeholder="t('home.quickSearch')"
            ><span>{{ t('home.quickSelected', { count: layout.quickIds.length }) }}</span>
          </div><div
            v-for="action in quickCandidates"
            :key="action.id"
            class="home-quick-candidate"
          >
            <span>{{ action.label }}</span><SwitchField
              :model-value="layout.quickIds.includes(action.id)"
              :label="action.label"
              :disabled="!layout.quickIds.includes(action.id) && layout.quickIds.length >= 6"
              @update:model-value="toggleQuick(action.id)"
            />
          </div><span
            v-if="quickMatches.length > 20"
            class="home-quick-picker-count"
          >{{ t('home.quickSearchMore') }}</span>
        </div>
        <button
          v-if="!quickActionsLoading && unavailableQuickIds.length"
          class="text-button home-quick-remove"
          @click="removeUnavailableQuick"
        >
          {{ t('home.removeUnavailableQuick', { count: unavailableQuickIds.length }) }}
        </button>
        <button
          class="text-button"
          @click="resetLayout"
        >
          {{ t('home.resetHome') }}
        </button>
      </div>
      <template
        v-for="section in layout.order"
        :key="section"
      >
        <section
          v-if="section === 'other' && !layout.hidden.includes(section) && otherGames.length"
          class="home-recent"
        >
          <h2>{{ t('home.other') }}</h2><div class="home-game-list">
            <GameCard
              v-for="game in otherGames"
              :key="game.id"
              :game="game"
              variant="row"
              :profile="effectiveProfile(game, profileLibrary)"
              :starting="starting.includes(game.id)"
              :retrying="retrying.includes(game.id)"
              :icon-url="iconUrl(game)"
              @play="$emit('play', $event)"
              @manage="$emit('manage', $event)"
              @retry="$emit('retry', $event)"
              @retry-session-recovery="$emit('retrySessionRecovery', $event)"
              @appearance="updateAppearance"
              @select="$emit('select', $event)"
            />
          </div>
          <button
            v-if="selection.ordered.length > otherGames.length + (selection.featured ? 1 : 0)"
            class="text-button home-more-games"
            @click="toggleAll"
          >
            {{ t('home.moreGames') }}<AppIcon
              name="play"
              :size="16"
            />
          </button>
        </section>
        <section
          v-if="section === 'favorites' && !layout.hidden.includes(section) && (savedCounts.favorite || savedCounts.later)"
          class="home-recent home-saved-market"
        >
          <h2>{{ t('home.savedMarket') }}</h2>
          <div class="home-saved-market-actions">
            <button
              v-if="savedCounts.favorite"
              class="secondary-button"
              @click="$emit('savedMarket', 'favorite')"
            >
              {{ t('home.marketFavorites', { count: savedCounts.favorite }) }}
            </button>
            <button
              v-if="savedCounts.later"
              class="secondary-button"
              @click="$emit('savedMarket', 'later')"
            >
              {{ t('home.marketLater', { count: savedCounts.later }) }}
            </button>
          </div>
        </section>
        <section
          v-if="section === 'quick' && !layout.hidden.includes(section) && quickEntries.length"
          class="home-recent home-quick"
        >
          <h2>{{ t('home.sections.quick') }}</h2><div class="home-quick-actions">
            <div
              v-for="(action, index) in quickEntries"
              :key="action.id"
            >
              <button
                class="secondary-button"
                :disabled="!!runningQuick"
                @click="runQuick(action)"
              >
                {{ action.label }}
              </button><template v-if="editing">
                <button
                  class="text-button"
                  :disabled="index === 0"
                  :aria-label="t('home.moveUp', { name: action.label })"
                  @click="moveQuick(action.id, -1)"
                >
                  <ArrowUp :size="16" />
                </button><button
                  class="text-button"
                  :disabled="index === quickEntries.length - 1"
                  :aria-label="t('home.moveDown', { name: action.label })"
                  @click="moveQuick(action.id, 1)"
                >
                  <ArrowDown :size="16" />
                </button>
              </template>
            </div>
          </div>
          <p
            v-if="quickError"
            role="alert"
            class="home-quick-error"
          >
            {{ t('home.quickActionFailed') }}
          </p>
        </section>
      </template>
    </template>
  </div>
</template>

<style scoped>
.library-home{min-width:0}.home-heading{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-4);margin-bottom:var(--ui-space-5)}.home-heading h1{font-size:var(--ui-text-page);line-height:var(--ui-leading-page);font-weight:600;letter-spacing:normal}.home-heading-actions{display:flex;align-items:center;gap:var(--ui-space-3)}.home-add{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label);border-radius:var(--ui-radius-md)}
.home-recent{margin-top:var(--ui-space-6)}.home-recent>h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600;letter-spacing:normal;margin:0 0 var(--ui-space-4)}.home-game-list{display:flex;flex-direction:column;gap:var(--ui-space-2)}.home-more-games{display:inline-flex;align-items:center;gap:var(--ui-space-2);margin-top:var(--ui-space-3)}.home-view-back{display:inline-flex;align-items:center;gap:var(--ui-space-2);margin-bottom:var(--ui-space-4)}.home-search{margin:0 0 var(--ui-space-4)}.home-search>span{padding-right:var(--ui-space-3);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums;color:var(--ui-text-muted)}
.home-filters{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-3);margin:0 0 var(--ui-space-4);font-size:var(--ui-text-label);line-height:var(--ui-leading-label);color:var(--ui-text-muted)}.home-filters label{display:flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height)}.home-filters :deep(.select-field-trigger){width:auto}.home-view-choice{display:flex;align-items:center;gap:var(--ui-space-1);padding:var(--ui-space-1);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1)}.home-view-choice .text-button{min-height:var(--ui-control-compact);padding:0 var(--ui-space-3);border-radius:var(--ui-radius-sm)}.home-view-choice .text-button[aria-pressed=true]{background:var(--ui-surface-2);color:var(--ui-text)}.home-tag-trigger{min-height:var(--ui-control-height);padding:0 var(--ui-space-3);border-radius:var(--ui-radius-md)}.home-tag-trigger:hover{background:var(--ui-surface-hover)}
.home-select-row{display:grid;grid-template-columns:var(--ui-control-compact) minmax(0,1fr);gap:var(--ui-space-2);align-items:center}.home-select-row .game-card{min-width:0}.home-select{display:grid;place-items:center;min-height:var(--ui-control-height)}.home-select input,.home-filters input,.home-layout-edit input{accent-color:var(--ui-focus)}.home-item-labels{grid-column:2;display:flex;flex-wrap:wrap;gap:var(--ui-space-1);margin-top:0}.home-item-labels span{padding:var(--ui-space-1) var(--ui-space-2);background:var(--ui-surface-2);border-radius:var(--ui-radius-sm);color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.home-game-list--grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--ui-space-3)}.home-game-list--grid .home-select-row{align-items:start}.home-game-list--grid :deep(.library-game--row){grid-template-columns:var(--ui-control-height) minmax(0,1fr);min-height:calc(3 * var(--ui-control-height) + var(--ui-space-5));align-items:start}.home-game-list--grid :deep(.library-game--row .library-game-actions){grid-column:2;justify-content:flex-start}.home-game-list--grid :deep(.library-game--row .library-game-name){font-size:var(--ui-text-body)}
.home-batch{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);margin-bottom:var(--ui-space-4);padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);font-size:var(--ui-text-label)}.home-batch input{max-width:160px;min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit}.home-batch button{min-height:var(--ui-control-height)}.home-batch-clear summary{cursor:pointer;color:var(--ui-text-muted)}.home-batch-clear[open]{display:flex;align-items:center;gap:var(--ui-space-2)}.home-batch-clear .text-button{font-size:var(--ui-text-label)}
.home-layout-edit{display:flex;flex-wrap:wrap;align-items:center;gap:var(--ui-space-3);margin-top:var(--ui-space-4);padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);font-size:var(--ui-text-label)}.home-layout-edit>div:not(.home-quick-picker){display:flex;align-items:center;gap:var(--ui-space-1)}.home-layout-edit>div:not(.home-quick-picker)>span{min-width:105px}.home-layout-edit label{display:flex;align-items:center;gap:var(--ui-space-2)}.home-layout-edit .text-button{padding:var(--ui-space-1)}.home-layout-edit>div:not(.home-quick-picker) .text-button,.home-quick-actions .text-button{display:grid;place-items:center;width:var(--ui-control-compact);height:var(--ui-control-compact)}.home-layout-edit>button:last-child{flex:0 0 100%;width:auto;text-align:left}
.home-quick-picker{display:flex;flex-direction:column;flex-wrap:nowrap;flex:0 0 100%;gap:var(--ui-space-2);max-height:230px;overflow:auto}.home-quick-picker>div{display:flex;align-items:center;gap:var(--ui-space-2)}.home-quick-picker-head{width:100%;flex:none;justify-content:space-between}.home-quick-picker-head input{min-width:180px;flex:1;min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-body)}.home-quick-picker-head>span,.home-quick-picker-count{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.home-quick-candidate{width:100%;min-height:var(--ui-control-height);flex:none;justify-content:space-between;font-size:var(--ui-text-body)}.home-quick-candidate>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.home-quick-actions,.home-saved-market-actions{display:flex;flex-wrap:wrap;gap:var(--ui-space-2)}.home-quick-actions>div{display:flex;align-items:center}.home-quick-actions .secondary-button,.home-saved-market-actions .secondary-button{min-height:var(--ui-control-height)}
.home-pagination{display:flex;align-items:center;justify-content:flex-end;gap:var(--ui-space-4);margin-top:var(--ui-space-4);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.home-pagination>span{margin-right:auto}.home-pagination button:disabled{opacity:.45}.home-no-results{text-align:center;padding:var(--ui-space-6) var(--ui-space-3);color:var(--ui-text-muted);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.home-empty{display:flex;flex-direction:column;align-items:center;text-align:center;padding:calc(2 * var(--ui-space-6)) var(--ui-space-4);gap:var(--ui-space-5)}.home-empty-icon{display:grid;place-items:center;width:76px;height:76px;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-2);color:var(--ui-text-muted)}.home-empty h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600;letter-spacing:normal}.home-empty .primary-button{min-height:var(--ui-launch-height,48px)}
@media(max-width:780px){.home-game-list--grid{grid-template-columns:1fr}}@media(max-width:560px){.home-heading{gap:var(--ui-space-3)}.home-heading-actions{gap:var(--ui-space-2)}.home-add{padding:0 var(--ui-space-2)}.home-empty{padding-top:var(--ui-space-6)}}
</style>
<style>
.home-add-menu{min-width:185px}.home-add-menu .profile-menu-item{gap:var(--ui-space-3)}.home-tag-popover{display:grid;gap:var(--ui-space-1);min-width:150px;max-height:240px;overflow:auto;padding:var(--ui-space-2);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);box-shadow:var(--ui-shadow-menu);z-index:140}.home-tag-popover label{display:flex;align-items:center;gap:var(--ui-space-2);padding:var(--ui-space-2);font-size:var(--ui-text-label);color:var(--ui-text)}.home-tag-popover input{accent-color:var(--ui-focus)}
</style>
