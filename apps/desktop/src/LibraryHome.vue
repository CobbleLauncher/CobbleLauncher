<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import GameCard from './GameCard.vue';
import AppIcon from './ui/AppIcon.vue';
import { effectiveProfile, type ProfileLibrary } from './games';
import { homeGames, searchHomeGames, type HomeGame } from './library-home';
import { t } from './i18n';

const props = defineProps<{ games: HomeGame[]; profileLibrary: ProfileLibrary; starting: string[]; retrying: string[]; loading?: boolean; error?: string }>();
defineEmits<{ play: [id: string]; manage: [id: string]; retry: [id: string]; create: []; importPack: []; importLauncher: []; reload: [] }>();
const all = ref(false);
const search = ref('');
const page = ref(0);
const pageSize = 50;
const updates = ref(new Map<string, Pick<HomeGame, 'pinned' | 'cover_ref'>>());
watch(() => props.games.map(game => `${game.id}:${game.pinned}:${game.cover_ref}`).join('|'), () => updates.value.clear());
const selection = computed(() => homeGames(props.games.map(game => ({ ...game, ...updates.value.get(game.id) }))));
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
const filtered = computed(() => searchHomeGames(selection.value.ordered, search.value));
const pageGames = computed(() => filtered.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
const visible = computed(() => all.value ? pageGames.value : [...(selection.value.featured ? [selection.value.featured] : []), ...selection.value.recent]);
watch(search, () => { page.value = 0; });
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
          v-if="games.length"
          class="text-button home-view-toggle"
          :aria-pressed="all"
          @click="toggleAll"
        >
          {{ all ? t('home.recent') : t('home.all') }}
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
      <div class="home-search">
        <AppIcon name="search" /><input
          v-model="search"
          type="search"
          :placeholder="t('home.search')"
          :aria-label="t('home.search')"
        ><span
          role="status"
          :aria-label="t('home.results', { count: filtered.length })"
        >{{ filtered.length }}</span>
      </div>
      <div
        v-if="pageGames.length"
        class="home-game-list"
      >
        <GameCard
          v-for="game in pageGames"
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
          @appearance="updateAppearance"
        />
      </div>
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
        @appearance="updateAppearance"
      />
      <section
        v-if="selection.recent.length"
        class="home-recent"
      >
        <h2>{{ t('home.other') }}</h2>
        <div class="home-game-list">
          <GameCard
            v-for="game in selection.recent"
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
            @appearance="updateAppearance"
          />
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.library-home{min-width:0}.home-heading{display:flex;align-items:center;justify-content:space-between;gap:16px;margin-bottom:28px}.home-heading h1{font-size:25px;font-weight:650;letter-spacing:-.7px}.home-heading-actions{display:flex;align-items:center;gap:20px}.home-add{min-height:36px;padding:0 12px;font-size:12px;gap:7px;border-radius:9px}.home-view-toggle{font-size:12px}.home-recent{margin-top:29px}.home-recent>h2{font-size:14px;letter-spacing:0;font-weight:600;margin:0 0 13px}.home-game-list{display:flex;flex-direction:column;gap:9px}.home-search{display:flex;align-items:center;gap:10px;margin:0 0 18px;padding:0 12px;min-height:42px;border:1px solid var(--line);border-radius:10px;background:var(--panel);color:var(--muted)}.home-search input{flex:1;min-width:0;border:0;background:transparent;font:inherit;font-size:12px;color:var(--ink);padding:11px 0;outline-offset:4px}.home-search>span{font-size:11px;font-variant-numeric:tabular-nums}.home-pagination{display:flex;align-items:center;justify-content:flex-end;gap:18px;margin-top:15px;font-size:11px;color:var(--muted)}.home-pagination>span{margin-right:auto}.home-pagination button:disabled{opacity:.4}.home-no-results{text-align:center;padding:34px 10px;color:var(--muted);font-size:12px}.home-empty{display:flex;flex-direction:column;align-items:center;text-align:center;padding:60px 16px 70px;gap:25px}.home-empty-icon{display:grid;place-items:center;width:78px;height:78px;border:1px solid var(--line);border-radius:22px;background:var(--hover);color:var(--muted)}.home-empty h2{font-size:20px;font-weight:600;letter-spacing:-.3px}.home-empty .primary-button{min-height:42px}.home-add-menu{min-width:185px}.home-add-menu .profile-menu-item{gap:11px}@media(max-width:560px){.home-heading{margin-bottom:23px;gap:12px}.home-heading h1{font-size:23px}.home-heading-actions{gap:13px}.home-view-toggle{font-size:11px}.home-add{padding:0 10px;gap:5px}.home-recent{margin-top:25px}.home-empty{padding-top:45px}.home-empty h2{font-size:18px}}

.home-heading{margin-bottom:24px}.home-heading h1{font-size:24px;font-weight:600;letter-spacing:normal}.home-heading-actions{gap:16px}.home-add{min-height:40px;font-size:13px;border-radius:8px}.home-view-toggle{font-size:13px}.home-recent{margin-top:32px}.home-recent>h2{font-size:18px;font-weight:600;margin-bottom:16px}.home-game-list{gap:8px}.home-search{border-color:var(--ui-border-strong);border-radius:8px}.home-search input{font-size:14px}.home-empty h2{font-weight:600;letter-spacing:normal}
</style>
