<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import { computed, onUnmounted, ref, watch } from 'vue';
import type { CurseForgePackPreview } from './curseforge';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import DiscoverPacksPage from './DiscoverPacksPage.vue';
import DiscoverMods from './DiscoverMods.vue';
import DiscoverPacks from './DiscoverPacks.vue';
import { type Game, type PackPreview } from './games';
import { marketGames, marketGameLocked, type MarketKind } from './market';
import MarketToolbar from './market/MarketToolbar.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { marketTarget, rememberMarketTarget } from './market-preferences';
import { savedMarketItems, type SavedMarketItem } from './market-saved';
import { marketWorkstate, rememberMarketWorkstate } from './market-workstate';
import type { FileInboxMarketLink } from './file-inbox-market';

const props = defineProps<{ games: Game[]; initialKind: MarketKind; initialGameId?: string; initialQuery?: string; initialLink?: FileInboxMarketLink | null }>();
const activeLink = ref(props.initialLink);
const emit = defineEmits<{ busy: [value: boolean]; selectedPack: [preview: PackPreview]; selectedCfPack: [preview: CurseForgePackPreview]; installed: [gameId: string]; locate: [gameId: string, kind: MarketKind] }>();
const categories: Exclude<MarketKind, 'blueprint'>[] = ['modpack', 'mod', 'resource_pack', 'shader_pack'];
const previousSelection = marketWorkstate<{ kind: MarketKind; provider: string; gameId: string; minecraftVersion?: string; loader?: string }>('market:selection');
const kind = ref<MarketKind>(props.initialLink?.kind ?? (props.initialQuery || props.initialGameId ? props.initialKind : previousSelection?.kind ?? props.initialKind));
const gameId = ref(marketTarget(props.games, kind.value, props.initialLink?.gameId ?? props.initialGameId ?? previousSelection?.gameId));
const busy = ref(false);
const cfAvailable = ref<boolean | null>(null);
function sharedQueryKey(category: MarketKind, target: string) { return `market:shared-query:${category}:${target || 'global'}`; }
const sharedQuery = ref(props.initialLink?.project ?? props.initialQuery ?? marketWorkstate<string>(sharedQueryKey(kind.value, gameId.value)) ?? '');
const sharedInitialQuery = ref(sharedQuery.value);
let sharedSerial = 0;
const minecraftVersion = ref(props.initialQuery || props.initialLink ? '' : previousSelection?.minecraftVersion ?? '');
const filterLoader = ref(props.initialQuery || props.initialLink ? '' : previousSelection?.loader ?? '');
const sharedRequest = ref({ serial: 0, query: sharedQuery.value, minecraftVersion: minecraftVersion.value, loader: filterLoader.value });
const queryError = ref('');
function submitSharedSearch() {
  const query = sharedQuery.value.trim();
  if (activeLink.value && query !== activeLink.value.project) activeLink.value = null;
  queryError.value = '';
  if (kind.value === 'modpack') {
    const version = minecraftVersion.value.trim();
    if (filterLoader.value && !version) { queryError.value = t('curseforge.versionRequired'); return; }
    if (version && !/^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$/.test(version)) { queryError.value = t('market.invalidVersion'); return; }
  }
  rememberMarketWorkstate(sharedQueryKey(kind.value, gameId.value), query);
  sharedRequest.value = { serial: ++sharedSerial, query, minecraftVersion: minecraftVersion.value.trim(), loader: filterLoader.value };
}
type MarketProvider = 'all' | 'modrinth' | 'curseforge';
const provider = ref<MarketProvider>(props.initialLink?.provider ?? 'all');
const savedRequest = ref<{ serial: number; item: SavedMarketItem }>();
const savedCollections = computed(() => ([
  { id: 'favorite', label: t('market.favorites'), items: savedMarketItems.value.filter(item => item.collection === 'favorite') },
  { id: 'later', label: t('market.saved'), items: savedMarketItems.value.filter(item => item.collection === 'later') },
]).filter(collection => collection.items.length));
let savedSerial = 0;
function openSaved(item: SavedMarketItem) {
  if (busy.value) return;
  kind.value = item.kind;
  provider.value = item.provider;
  if (item.kind !== 'modpack') gameId.value = marketTarget(props.games, item.kind, gameId.value);
  if (item.kind === 'modpack' || gameId.value) savedRequest.value = { serial: ++savedSerial, item };
}
const installedName = ref('');
const status = computed(() => installedName.value ? t('market.installed', { name: installedName.value }) : '');
function categoryLabel(category: MarketKind) {
  const key = category === 'resource_pack' ? 'resourcePack' : category === 'shader_pack' ? 'shaderPack' : category;
  return t(`market.${key}`);
}
const targets = computed(() => marketGames(props.games, kind.value));
const game = computed(() => targets.value.find(game => game.id === gameId.value));
const activeSources = computed<Exclude<MarketProvider, 'all'>[]>(() => kind.value === 'mod' || kind.value === 'modpack'
  ? provider.value === 'all' ? ['modrinth', 'curseforge'] : [provider.value]
  : ['modrinth']);
function browseComponent(source: Exclude<MarketProvider, 'all'>) {
  return kind.value === 'modpack'
    ? source === 'curseforge' ? CurseForgeDiscover : DiscoverPacksPage
    : kind.value === 'mod' ? source === 'curseforge' ? CurseForgeDiscover : DiscoverMods : DiscoverPacks;
}
function browseProps(source: Exclude<MarketProvider, 'all'>) {
  const sharedMode = true;
  const initialQuery = sharedMode ? sharedInitialQuery.value : props.initialQuery;
  const linkRequest = activeLink.value?.provider === source && activeLink.value.kind === kind.value ? activeLink.value : undefined;
  if (kind.value === 'modpack') return source === 'curseforge'
    ? { kind: 'modpack', initialQuery, sharedMode, sharedRequest: sharedRequest.value, linkRequest }
    : { initialQuery, sharedMode, sharedRequest: sharedRequest.value, linkRequest };
  const locked = game.value ? marketGameLocked(game.value) : false;
  if (kind.value === 'mod') return source === 'curseforge'
    ? { kind: 'mod', game: game.value, locked, initialQuery, sharedMode, sharedRequest: sharedRequest.value, linkRequest }
    : { game: game.value, locked, initialQuery, sharedMode, sharedRequest: sharedRequest.value, linkRequest };
  return { gameId: gameId.value, kind: kind.value, locked, browseOnMount: true, initialQuery, sharedMode, sharedRequest: sharedRequest.value, linkRequest };
}
function setBusy(value: boolean) { busy.value = value; emit('busy', value); }
function installed() {
  if (!game.value) return;
  installedName.value = game.value.name;
  emit('installed', game.value.id);
}
function selectKind(value: MarketKind) {
  if (!busy.value && kind.value !== value) {
    activeLink.value = null;
    savedRequest.value = undefined;
    kind.value = value;
    gameId.value = marketTarget(props.games, value);
  }
}
function selectProvider(value: MarketProvider) {
  if (value !== provider.value) {
    provider.value = value;
    if (activeLink.value && value !== activeLink.value.provider) activeLink.value = null;
  }
}
watch([() => props.initialKind, () => props.initialGameId], () => {
  savedRequest.value = undefined;
  kind.value = props.initialKind; gameId.value = marketTarget(props.games, props.initialKind, props.initialGameId); installedName.value = '';
});
watch([kind, gameId], ([newKind, newGame], [oldKind, oldGame]) => {
  rememberMarketWorkstate(sharedQueryKey(oldKind, oldGame), sharedQuery.value);
  sharedQuery.value = marketWorkstate<string>(sharedQueryKey(newKind, newGame)) ?? '';
  sharedInitialQuery.value = sharedQuery.value;
  sharedRequest.value = { serial: ++sharedSerial, query: sharedQuery.value, minecraftVersion: minecraftVersion.value, loader: filterLoader.value };
  installedName.value = '';
    queryError.value = '';
});
watch(targets, () => { if (gameId.value && !game.value) gameId.value = ''; });
function rememberTarget() { savedRequest.value = undefined; rememberMarketTarget(kind.value, gameId.value); }
function selectTarget(value: string) { gameId.value = value; rememberTarget(); }
onUnmounted(() => {
  rememberMarketWorkstate('market:selection', { kind: kind.value, provider: provider.value, gameId: gameId.value, minecraftVersion: minecraftVersion.value, loader: filterLoader.value });
  rememberMarketWorkstate(sharedQueryKey(kind.value, gameId.value), sharedQuery.value);
});
</script>

<template>
  <div class="page-content market-page">
    <MarketToolbar
      :kind="kind"
      :categories="categories.map(value => ({value,label:categoryLabel(value)}))"
      :targets="targets"
      :game-id="gameId"
      :provider="provider"
      :query="sharedQuery"
      :minecraft-version="minecraftVersion"
      :loader="filterLoader"
      :busy="busy"
      :cf-available="cfAvailable"
      @kind="selectKind"
      @target="selectTarget"
      @provider="selectProvider($event as MarketProvider)"
      @query="sharedQuery = $event"
      @version="minecraftVersion = $event"
      @loader="filterLoader = $event"
      @search="submitSharedSearch"
    >
      <p
        v-if="queryError"
        class="settings-error"
        role="alert"
      >
        {{ queryError }}
      </p>
      <p
        v-if="provider === 'curseforge' && cfAvailable === false"
        class="setting-hint"
        role="status"
      >
        {{ t('curseforge.unavailable') }}
      </p>
      <SettingsDisclosure
        v-for="collection in savedCollections"
        :key="collection.id"
        class="market-saved"
        :title="`${collection.label} (${collection.items.length})`"
      >
        <div class="market-saved-items">
          <button
            v-for="item in collection.items"
            :key="`${item.collection}-${item.provider}-${item.kind}-${item.project_id}`"
            class="text-button"
            :disabled="busy"
            @click="openSaved(item)"
          >
            {{ item.title }}
          </button>
        </div>
      </SettingsDisclosure>
      <div class="market-workflow">
        <p v-if="activeLink" class="setting-hint" role="status">
          {{ activeLink.provider === 'modrinth' ? 'Modrinth' : 'CurseForge' }} · {{ activeLink.project }}<template v-if="activeLink.version"> · {{ activeLink.version }}</template>
        </p>
        <p
          v-if="kind !== 'modpack' && !targets.length"
          class="content-empty"
        >
          {{ kind === 'mod' ? t('market.noModGame') : t('market.noGame') }}
        </p>
        <section
          v-for="source in activeSources"
          v-show="source !== 'curseforge' || cfAvailable !== false"
          :key="source"
          class="market-source-results"
          :class="{ 'market-unified-results': provider === 'all' }"
          :aria-label="source === 'modrinth' ? 'Modrinth' : 'CurseForge'"
        >
          <div
            v-if="activeSources.length > 1 && cfAvailable !== false"
            class="market-source-title"
          >
            {{ source === 'modrinth' ? 'Modrinth' : 'CurseForge' }}
          </div>
          <KeepAlive :max="64">
            <component
              :is="browseComponent(source)"
              v-if="kind === 'modpack' || game"
              :key="`${kind}-${source}-${gameId}`"
              v-bind="browseProps(source)"
              :saved-request="savedRequest"
              @busy="setBusy"
              @availability="cfAvailable = $event"
              @installing="setBusy"
              @installed="installed"
              @selected-pack="emit('selectedPack', $event)"
              @selected-cf-pack="emit('selectedCfPack', $event)"
            />
          </KeepAlive>
        </section>
        <p
          v-if="status"
          role="status"
          class="setting-hint"
        >
          {{ status }} <button
            class="text-button"
            @click="emit('locate', gameId, kind)"
          >
            {{ t('market.viewInstalled') }}
          </button>
        </p>
      </div>
    </MarketToolbar>
  </div>
</template>
<style scoped>
.market-page,.market-workflow{min-width:0}.market-page{container-type:inline-size}.market-source-results+.market-source-results{margin-top:var(--ui-space-6)}.market-source-title{margin:0 0 var(--ui-space-2);font-size:var(--ui-text-caption);font-weight:var(--ui-weight-medium);color:var(--ui-text-muted)}
.market-page :deep(.discover-list){display:grid;gap:var(--ui-space-2)}.market-page :deep(.discover-packs-page){padding:0}.market-page :deep(.curseforge-discover){padding-top:0}.market-unified-results :deep(.discover-list){gap:0}.market-unified-results :deep(.pack-result-card){border:0;border-bottom:1px solid var(--ui-border);border-radius:0;padding:var(--ui-space-4) 0;background:transparent}.market-unified-results :deep(.pack-result-card:not([aria-disabled=true]):hover){background:var(--ui-surface-hover)}
.market-unified-results :deep(.pack-result-card h2),.market-unified-results :deep(.pack-result-card h3){font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:var(--ui-weight-semibold)}.market-saved{margin-bottom:var(--ui-space-3)}.market-saved-items{display:flex;gap:var(--ui-space-2);flex-wrap:wrap;padding-top:var(--ui-space-3)}
.market-unified-results :deep(.content-empty){text-align:left;padding:var(--ui-space-4) 0;font-size:var(--ui-text-label)}
</style>
