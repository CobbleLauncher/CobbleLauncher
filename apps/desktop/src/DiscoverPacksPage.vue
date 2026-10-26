<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SearchField from './ui/SearchField.vue';
import CompatibilityNotice from './CompatibilityNotice.vue';
import MarketImage from "./MarketImage.vue";
import { marketReturn } from './market-return';
import MarketGallery from "./MarketGallery.vue";
import VersionSelector from './VersionSelector.vue';
import { preferredRelease } from './version-selector';
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { loaderLabel, type ModrinthPackVersion, type ModSearchHit, type ModSearchPage, type PackPreview } from './games';
import { locale, t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { isMarketSaved, toggleMarketSaved, type SavedMarketItem } from './market-saved';
import { marketWorkstate, rememberMarketWorkstate, restoreMarketScroll } from './market-workstate';
import { findLinkedProject, linkedVersionId, type FileInboxMarketLink } from './file-inbox-market';
const props = defineProps<{ initialQuery?: string; sharedMode?: boolean; sharedRequest?: { serial: number; query: string; minecraftVersion?: string; loader?: string }; savedRequest?: { serial: number; item: SavedMarketItem }; linkRequest?: FileInboxMarketLink }>();
const emit = defineEmits<{ selectedPack: [preview: PackPreview]; busy: [value: boolean] }>();
const root = ref<HTMLElement>();
const detailReturn = marketReturn(root);
const query = ref('');
const minecraftVersion = ref(props.sharedMode ? props.sharedRequest?.minecraftVersion ?? '' : '');
const loader = ref(props.sharedMode ? props.sharedRequest?.loader ?? '' : '');
const filterError = ref('');
const submitted = ref<{ query: string; minecraftVersion: string | null; loader: string | null }>({ query: '', minecraftVersion: null, loader: null });
const hits = ref<ModSearchHit[]>([]);
const total = ref(0);
const nextOffset = ref(0);
const hasMore = ref(false);
const searching = ref(false);
const searched = ref(false);
const searchError = ref('');
const linkProjectMissing = ref(false);
const selected = ref<ModSearchHit | null>(null);
const versions = ref<ModrinthPackVersion[]>([]);
const versionId = ref('');
const loadingVersions = ref(false);
const versionError = ref('');
const preparing = ref(false);
const previewError = ref('');
const chosenVersion = computed(() => versions.value.find((version) => version.version_id === versionId.value));
const packFilterSummary = computed(() => [minecraftVersion.value.trim(), loader.value].filter(Boolean).join(' · ') || t('market.filters'));
let searchRequest = 0;
let versionRequest = 0;
let failedOffset = 0;
function packLoader(value: string) { return value === 'minecraft' || value === 'vanilla' ? '原版' : ['fabric', 'forge', 'neoforge'].includes(value) ? loaderLabel(value) : value; }
async function search(offset = 0, retry = false) {
  if (preparing.value) return;
  filterError.value = '';
  if (!offset && !retry) {
    const version = minecraftVersion.value.trim();
    if (version && !/^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$/.test(version)) { filterError.value = t('market.invalidVersion'); return; }
    submitted.value = { query: query.value.trim(), minecraftVersion: version || null, loader: loader.value || null };
    selected.value = null; versions.value = []; ++versionRequest;
  }
  if (!isTauri()) { searchError.value = t('market.desktopRequired'); return; }
  const current = ++searchRequest;
  let linked: ModSearchHit | undefined;
  if (!offset) linkProjectMissing.value = false;
  failedOffset = offset; searching.value = true; searched.value = true; searchError.value = '';
  try {
    const result = await invoke<ModSearchPage>('search_modrinth_packs', { ...submitted.value, offset });
    if (current !== searchRequest) return;
    hits.value = offset ? [...new Map([...hits.value, ...result.hits].map((hit) => [hit.project_id, hit])).values()] : result.hits;
    if (!offset && props.linkRequest) { linked = findLinkedProject(hits.value, props.linkRequest); linkProjectMissing.value = !linked; }
    total.value = result.total_hits; nextOffset.value = result.offset + result.hits.length; hasMore.value = result.hits.length > 0 && nextOffset.value < result.total_hits;
  } catch { if (current === searchRequest) searchError.value = t('market.packSearchFailed'); }
  finally { if (current === searchRequest) { searching.value = false; if (linked) select(linked); } }
}
async function loadVersions() {
  if (!selected.value || preparing.value) return;
  const current = ++versionRequest;
  loadingVersions.value = true; versionError.value = ''; previewError.value = ''; versions.value = []; versionId.value = '';
  try {
    const result = await invoke<ModrinthPackVersion[]>('list_modrinth_pack_versions', { projectId: selected.value.project_id, minecraftVersion: submitted.value.minecraftVersion, loader: submitted.value.loader });
    if (current !== versionRequest) return;
    versions.value = result; versionId.value = props.linkRequest?.version ? linkedVersionId(result, props.linkRequest.version) : preferredRelease(result)?.version_id ?? '';
  } catch (cause) { if (current === versionRequest) versionError.value = `无法读取整合包版本：${String(cause)}`; }
  finally { if (current === versionRequest) loadingVersions.value = false; }
}
function select(hit: ModSearchHit) { if (!preparing.value && !searching.value) { detailReturn.enter(hit.project_id); selected.value = hit; void loadVersions(); } }

function back() { if (!preparing.value) { ++versionRequest; selected.value = null; loadingVersions.value = false; detailReturn.restore(); } }
async function prepare() {
  if (!selected.value || !chosenVersion.value || preparing.value) return;
  preparing.value = true; previewError.value = ''; emit('busy', true);
  try { emit('selectedPack', await invoke<PackPreview>('preview_modrinth_pack', { projectId: selected.value.project_id, versionId: versionId.value })); }
  catch (cause) { previewError.value = `无法准备安装预览：${String(cause)}`; }
  finally { preparing.value = false; emit('busy', false); }
}
const workKey = 'modrinth:modpack:global';
function remember() {
  rememberMarketWorkstate(workKey, { query: query.value, minecraftVersion: minecraftVersion.value, loader: loader.value, submitted: submitted.value, hits: hits.value, total: total.value, nextOffset: nextOffset.value, hasMore: hasMore.value }, root.value);
}
onActivated(() => restoreMarketScroll(workKey, root.value));
onDeactivated(remember);
onUnmounted(() => { remember(); ++searchRequest; ++versionRequest; });
watch(() => props.savedRequest?.serial, () => {
  const item = props.savedRequest?.item;
  if (item?.provider === 'modrinth' && item.kind === 'modpack' && item.hit.project_id === item.project_id) {
    ++searchRequest; searching.value = false; select(item.hit as ModSearchHit);
  }
});
watch(() => props.sharedRequest?.serial, () => {
  if (!props.sharedMode || !props.sharedRequest) return;
  query.value = props.sharedRequest.query;
  minecraftVersion.value = props.sharedRequest.minecraftVersion ?? ''; loader.value = props.sharedRequest.loader ?? '';
  void search();
});
onMounted(() => {
  const item = props.savedRequest?.item;
  if (item?.provider === 'modrinth' && item.kind === 'modpack') select(item.hit as ModSearchHit);
  else if (props.linkRequest) { query.value = props.linkRequest.project; void search(); }
  else {
    const state = marketWorkstate<{ query: string; minecraftVersion: string; loader: string; submitted: typeof submitted.value; hits: ModSearchHit[]; total: number; nextOffset: number; hasMore: boolean }>(workKey);
    if (state && (!props.sharedMode || (state.query === (props.sharedRequest?.query ?? props.initialQuery ?? '') && (!props.sharedRequest || (state.minecraftVersion === (props.sharedRequest.minecraftVersion ?? '') && state.loader === (props.sharedRequest.loader ?? '')))))) { query.value = state.query; minecraftVersion.value = state.minecraftVersion; loader.value = state.loader; submitted.value = state.submitted; hits.value = state.hits; total.value = state.total; nextOffset.value = state.nextOffset; hasMore.value = state.hasMore; restoreMarketScroll(workKey, root.value); }
    else { query.value = props.sharedMode ? props.sharedRequest?.query ?? props.initialQuery ?? '' : props.initialQuery ?? ''; void search(); }
  }
});
</script>
<template>
  <div
    ref="root"
    class="page-content discover-packs-page"
  >
    <p v-if="linkProjectMissing && !selected" role="status" class="setting-hint">{{ locale === 'zh-CN' ? '未在当前结果中找到该项目。可调整搜索或筛选。' : 'Project not found in these results. Try another search or filter.' }}</p>
    <p v-if="selected && linkRequest?.version && !loadingVersions && !versionId && !versionError" role="status" class="setting-hint">{{ locale === 'zh-CN' ? '链接中的版本未在当前列表中找到，请手动选择版本。' : 'Linked version was not found here. Choose a version manually.' }}</p>
    <template v-if="selected">
      <button
        class="text-button"
        :disabled="preparing"
        @click="back"
      >
        ← 返回搜索结果
      </button>
      <div class="mod-preview pack-project-preview">
        <MarketImage
          provider="modrinth"
          :url="selected.icon_url"
          :title="selected.title"
        /><div class="mod-info">
          <h2>{{ selected.title }}</h2><p class="mod-meta">
            {{ selected.author }} · {{ new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(selected.downloads) }} 次下载
          </p>
        </div>
      </div>
      <p class="preview-description">
        {{ selected.description }}
      </p>
      <button
        class="text-button market-save-button"
        @click="toggleMarketSaved('modrinth', 'modpack', selected)"
      >
        {{ t(isMarketSaved('modrinth', 'modpack', selected.project_id) ? 'market.removeSaved' : 'market.saveLater') }}
      </button>
      <button
        class="text-button"
        @click="toggleMarketSaved('modrinth', 'modpack', selected, 'favorite')"
      >
        {{ t(isMarketSaved('modrinth', 'modpack', selected.project_id, 'favorite') ? 'market.removeFavorite' : 'market.favorite') }}
      </button>
      <MarketGallery
        v-if="selected.gallery?.length"
        provider="modrinth"
        :urls="selected.gallery"
        :title="selected.title"
      />
      <p
        v-if="loadingVersions"
        class="content-empty"
        role="status"
      >
        正在读取整合包版本…
      </p>
      <div
        v-else-if="versionError"
        class="inline-error"
        role="alert"
      >
        <p>{{ versionError }}</p><button
          class="text-button"
          @click="loadVersions"
        >
          重试读取版本
        </button>
      </div>
      <template v-else-if="versions.length">
        <div class="form-field pack-version-field">
          <label for="online-pack-version">整合包版本</label><VersionSelector
            id="online-pack-version"
            v-model="versionId"
            :disabled="preparing"
            :options="versions.map(version => ({ value: version.version_id, label: `${version.name}${version.name !== version.version_number ? ` · ${version.version_number}` : ''}` }))"
            @update:model-value="previewError = ''"
          />
        </div>
        <div
          v-if="chosenVersion"
          class="pack-version-info"
        >
          <CompatibilityNotice
            v-for="mc in chosenVersion.minecraft_versions"
            :key="mc"
            :minecraft-version="mc"
            :loader-kind="chosenVersion.loaders.find(kind => kind === 'forge' || kind === 'neoforge')"
          />
          <div class="compatibility-tags">
            <span
              v-for="version in chosenVersion.minecraft_versions"
              :key="version"
            >Minecraft {{ version }}</span><span
              v-for="kind in chosenVersion.loaders"
              :key="kind"
            >{{ packLoader(kind) }}</span>
          </div><p class="mod-meta">
            {{ ({ release: '正式版', beta: '测试版', alpha: '预览版' })[chosenVersion.version_type as 'release' | 'beta' | 'alpha'] || chosenVersion.version_type }} · {{ chosenVersion.date_published.slice(0, 10) }}
          </p>
        </div>
        <button
          class="primary-button pack-preview-button"
          :disabled="preparing || !chosenVersion"
          @click="prepare"
        >
          <span
            v-if="preparing"
            class="busy-spinner"
            aria-hidden="true"
          />{{ preparing ? '准备预览中…' : previewError ? '重试安装预览' : '查看安装预览' }}
        </button>
        <p
          v-if="previewError"
          class="settings-error"
          role="alert"
        >
          {{ previewError }}
        </p>
      </template>
      <p
        v-else
        class="content-empty"
      >
        没有符合筛选条件的可安装版本。可以返回搜索结果调整筛选。
      </p>
    </template>
    <template v-else>
      <form
        class="pack-discovery-form"
        novalidate
        @submit.prevent="search()"
      >
        <div
          v-if="!sharedMode"
          class="content-toolbar discover-search"
        >
          <SearchField
            v-model="query"
            maxlength="120"
            :label="t('market.searchPacks')"
            :placeholder="t('market.packSearchPlaceholder')"
            @submit="search()"
          /><button
            class="secondary-button"
            type="submit"
          >
            {{ searching ? t('market.searching') : t('curseforge.search') }}
          </button>
        </div>
        <details
          v-if="!sharedMode"
          class="pack-discovery-filter-details"
        >
          <summary>{{ packFilterSummary }}</summary>
          <div class="pack-discovery-filters">
            <div class="form-field">
              <label for="pack-minecraft-filter">{{ t('curseforge.version') }}</label><input
                id="pack-minecraft-filter"
                v-model="minecraftVersion"
                maxlength="64"
                autocomplete="off"
                spellcheck="false"
                :placeholder="t('curseforge.allVersions')"
                :aria-invalid="!!filterError"
              >
            </div><div class="form-field">
              <label for="pack-loader-filter">{{ t('curseforge.loader') }}</label><select
                id="pack-loader-filter"
                v-model="loader"
              >
                <option value="">
                  {{ t('curseforge.allLoaders') }}
                </option><option value="fabric">
                  Fabric
                </option><option value="forge">
                  Forge
                </option><option value="neoforge">
                  NeoForge
                </option><option value="minecraft">
                  Minecraft
                </option>
              </select>
            </div>
          </div>
        </details>
      </form>
      <p
        v-if="filterError"
        class="settings-error"
        role="alert"
      >
        {{ filterError }}
      </p>
      <ul
        v-if="hits.length"
        class="mod-list discover-list pack-search-results"
        :aria-label="t('market.modpack')"
      >
        <li
          v-for="hit in hits"
          :key="hit.project_id"
          class="mod-row pack-result-card ui-card-action-surface"
          :aria-disabled="searching || preparing"
        >
          <MarketImage
            provider="modrinth"
            :url="hit.preview_url || hit.gallery?.[0] || hit.icon_url"
            :title="hit.title"
            :preview="!!(hit.preview_url || hit.gallery?.[0])"
            artwork
          /><div class="mod-info">
            <h2>{{ hit.title }}</h2><p class="mod-description">
              {{ hit.description }}
            </p><p class="mod-meta">
              {{ hit.author }} · {{ t('market.downloads', { count: new Intl.NumberFormat(locale, { notation: 'compact' }).format(hit.downloads) }) }}
            </p>
          </div>
          <button
            type="button"
            class="text-button ui-stretched-action"
            :disabled="searching || preparing"
            :data-market-project="hit.project_id"
            :aria-label="t('market.viewProject', { name: hit.title })"
            @click="select(hit)"
          >
            {{ t('market.view') }}
          </button>
        </li>
      </ul>
      <div
        v-if="searchError"
        class="inline-error"
        role="alert"
      >
        <p>{{ searchError }}</p><button
          class="text-button"
          :disabled="searching"
          @click="search(failedOffset, true)"
        >
          {{ t('curseforge.retry') }}
        </button>
      </div>
      <LoadingIndicator
        v-else-if="searching"
        :label="t('market.searching')"
        :skeleton="!hits.length"
        :compact="!!hits.length"
      />
      <p
        v-else-if="!searched"
        class="content-empty"
      >
        {{ t('market.browsePacks') }}
      </p>
      <p
        v-else-if="!hits.length"
        class="content-empty"
      >
        {{ t('market.emptyPacks') }}
      </p>
      <div
        v-else
        class="pack-results-footer"
      >
        <p class="mod-meta">
          {{ t('market.shownPacks', { shown: hits.length, total }) }}
        </p><button
          v-if="hasMore"
          class="secondary-button"
          :disabled="searching"
          @click="search(nextOffset)"
        >
          {{ t('curseforge.loadMore') }}
        </button>
      </div>
    </template>
  </div>
</template>
<style scoped>
.pack-discovery-filter-details{margin-top:-7px}
.pack-discovery-filter-details>summary{width:fit-content;color:var(--muted);font-size:var(--ui-text-caption);cursor:pointer}
.pack-discovery-filters{margin-top:11px}
.pack-search-results .pack-result-card{cursor:pointer;border:1px solid var(--line);border-radius:12px;padding:12px;transition:background-color .14s ease,border-color .14s ease}
.pack-search-results .pack-result-card:not([aria-disabled="true"]):hover{background:color-mix(in srgb,var(--panel) 88%,#000 12%);border-color:var(--muted)}
.pack-search-results .pack-result-card:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.pack-search-results .pack-result-card[aria-disabled="true"]{cursor:default;opacity:.72}
@media(prefers-reduced-motion:reduce){.pack-search-results .pack-result-card{transition:none}}
</style>

<style scoped>.discover-search .search-field{flex:1;min-width:0}.discover-search{gap:var(--ui-space-3)}</style>
