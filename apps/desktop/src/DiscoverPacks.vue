<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SearchField from './ui/SearchField.vue';
import MarketImage from "./MarketImage.vue";
import { marketReturn } from './market-return';
import MarketGallery from "./MarketGallery.vue";
import VersionSelector from './VersionSelector.vue';
import { preferredRelease } from './version-selector';
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import type { ContentPackVersion, LocalPack, LocalPackKind, ModSearchHit, ModSearchPage } from './games';
import { isMarketSaved, toggleMarketSaved, type SavedMarketItem } from './market-saved';
import { marketWorkstate, rememberMarketWorkstate, restoreMarketScroll } from './market-workstate';
import { findLinkedProject, linkedVersionId, type FileInboxMarketLink } from './file-inbox-market';

const props = defineProps<{ gameId: string; kind: LocalPackKind; locked: boolean; revision?: number; browseOnMount?: boolean; initialQuery?: string; sharedMode?: boolean; sharedRequest?: {serial:number;query:string}; savedRequest?: { serial: number; item: SavedMarketItem }; linkRequest?: FileInboxMarketLink }>();
const emit = defineEmits<{ busy: [value: boolean]; installed: [packs: LocalPack[]] }>();
const label = computed(() => props.kind === 'resource_pack' ? '资源包' : '光影包');
const root = ref<HTMLElement>();
const detailReturn = marketReturn(root);
const query = ref(props.sharedMode ? props.sharedRequest?.query ?? props.initialQuery ?? '' : props.initialQuery ?? '');
const submitted = ref('');
const hits = ref<ModSearchHit[]>([]);
const searched = ref(false);
const searching = ref(false);
const searchError = ref('');
const linkProjectMissing = ref(false);
const nextOffset = ref(0);
const hasMore = ref(false);
const selected = ref<ModSearchHit | null>(null);
const versions = ref<ContentPackVersion[]>([]);
const versionId = ref('');
const chosenVersion = computed(() => versions.value.find((version) => version.version_id === versionId.value));
const loadingVersions = ref(false);
const versionError = ref('');
const installing = ref(false);
const installError = ref('');
let searchRequest = 0;
let versionRequest = 0;
let failedOffset = 0;
let disposed = false;

function versionType(value: string): string { return ({ release: '正式版', beta: '测试版', alpha: '预览版' })[value] ?? value; }
function downloadCount(value: number): string { return new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(value); }
async function search(offset = 0, retry = false) {
  if (installing.value) return;
  const current = ++searchRequest;
  let linked: ModSearchHit | undefined;
  if (!offset) linkProjectMissing.value = false;
  if (!offset && !retry) submitted.value = query.value.trim();
  failedOffset = offset;
  searched.value = true; searching.value = true; searchError.value = '';
  try {
    const result = await invoke<ModSearchPage>('search_game_content_packs', { id: props.gameId, kind: props.kind, query: submitted.value, offset });
    if (current !== searchRequest) return;
    hits.value = offset ? [...new Map([...hits.value, ...result.hits].map((hit) => [hit.project_id, hit])).values()] : result.hits;
    if (!offset && props.linkRequest) { linked = findLinkedProject(hits.value, props.linkRequest); linkProjectMissing.value = !linked; }
    nextOffset.value = result.offset + result.hits.length;
    hasMore.value = result.hits.length > 0 && nextOffset.value < result.total_hits;
  } catch { if (current === searchRequest) searchError.value = `暂时无法获取${label.value}，请重试。`; }
  finally { if (current === searchRequest) { searching.value = false; if (linked) select(linked); } }
}
async function loadVersions() {
  if (!selected.value || installing.value) return;
  const current = ++versionRequest;
  versions.value = []; versionId.value = ''; versionError.value = ''; installError.value = ''; loadingVersions.value = true;
  try {
    const result = await invoke<ContentPackVersion[]>('list_game_content_pack_versions', { id: props.gameId, kind: props.kind, projectId: selected.value.project_id });
    if (current !== versionRequest) return;
    versions.value = result; versionId.value = props.linkRequest?.version ? linkedVersionId(result, props.linkRequest.version) : preferredRelease(result)?.version_id ?? '';
  } catch (cause) { if (current === versionRequest) versionError.value = `无法读取${label.value}版本：${String(cause)}`; }
  finally { if (current === versionRequest) loadingVersions.value = false; }
}
function select(hit: ModSearchHit) {
  if (installing.value || searching.value) return;
  detailReturn.enter(hit.project_id);
  selected.value = hit; void loadVersions();
}
function back() {
  if (installing.value) return;
  ++versionRequest; selected.value = null; loadingVersions.value = false; versionError.value = ''; installError.value = ''; detailReturn.restore();
}
async function install() {
  if (!selected.value || !chosenVersion.value || props.locked || installing.value) return;
  installing.value = true; installError.value = ''; emit('busy', true);
  try {
    const result = await invoke<LocalPack[]>('install_game_content_pack', { id: props.gameId, kind: props.kind, projectId: selected.value.project_id, versionId: chosenVersion.value.version_id });
    if (!disposed) emit('installed', result);
  } catch (cause) { if (!disposed) installError.value = `安装${label.value}未完成：${String(cause)}`; }
  finally { installing.value = false; emit('busy', false); }
}
watch(() => [props.gameId, props.kind], () => {
  ++searchRequest; ++versionRequest; query.value = ''; submitted.value = ''; hits.value = [];
  selected.value = null; versions.value = []; versionId.value = ''; searched.value = false;
  searching.value = false; loadingVersions.value = false; hasMore.value = false;
  searchError.value = ''; versionError.value = ''; installError.value = ''; detailReturn.restore();
});
watch(() => props.sharedRequest?.serial, () => { if (!props.sharedMode || !props.sharedRequest || installing.value) return; query.value = props.sharedRequest.query; selected.value = null; ++versionRequest; void search(); });
watch(() => props.revision, () => { installError.value = ''; });
function workKey() { return `modrinth:${props.kind}:${props.gameId}`; }
function remember() {
  rememberMarketWorkstate(workKey(), { query: query.value, submitted: submitted.value, hits: hits.value, searched: searched.value, nextOffset: nextOffset.value, hasMore: hasMore.value }, root.value);
}
onActivated(() => restoreMarketScroll(workKey(), root.value));
onDeactivated(remember);
onUnmounted(() => { remember(); disposed = true; ++searchRequest; ++versionRequest; });
watch(() => props.savedRequest?.serial, () => {
  const item = props.savedRequest?.item;
  if (item?.provider === 'modrinth' && item.kind === props.kind && item.hit.project_id === item.project_id) {
    ++searchRequest; searching.value = false; select(item.hit as ModSearchHit);
  }
});
onMounted(() => {
  const item = props.savedRequest?.item;
  if (item?.provider === 'modrinth' && item.kind === props.kind) select(item.hit as ModSearchHit);
  else if (props.linkRequest) { query.value = props.linkRequest.project; void search(); }
  else {
    const state = marketWorkstate<{ query: string; submitted: string; hits: ModSearchHit[]; searched: boolean; nextOffset: number; hasMore: boolean }>(workKey());
    if (state && (!props.sharedMode || state.query === (props.sharedRequest?.query ?? props.initialQuery ?? ''))) { query.value = state.query; submitted.value = state.submitted; hits.value = state.hits; searched.value = state.searched; nextOffset.value = state.nextOffset; hasMore.value = state.hasMore; restoreMarketScroll(workKey(), root.value); }
    else if (props.browseOnMount) void search();
  }
});
</script>
<template>
  <div
    ref="root"
    class="discover-content discover-local-packs"
  >
    <p v-if="linkProjectMissing && !selected" role="status" class="setting-hint">{{ locale === 'zh-CN' ? '未在当前结果中找到该项目。可调整搜索或筛选。' : 'Project not found in these results. Try another search or filter.' }}</p>
    <p v-if="selected && linkRequest?.version && !loadingVersions && !versionId && !versionError" role="status" class="setting-hint">{{ locale === 'zh-CN' ? '链接中的版本未在当前列表中找到，请手动选择版本。' : 'Linked version was not found here. Choose a version manually.' }}</p>
    <template v-if="selected">
      <button
        class="text-button"
        :disabled="installing"
        @click="back"
      >
        ← 返回搜索结果
      </button>
      <div class="mod-preview pack-project-preview">
        <MarketImage
          provider="modrinth"
          :url="selected.icon_url"
          :title="selected.title"
        />
        <div class="mod-info">
          <h2>{{ selected.title }}</h2><p class="mod-meta">
            {{ selected.author }} · {{ downloadCount(selected.downloads) }} 次下载
          </p>
        </div>
      </div>
      <p class="preview-description">
        {{ selected.description }}
      </p>
      <button
        class="text-button market-save-button"
        @click="toggleMarketSaved('modrinth', kind, selected)"
      >
        {{ t(isMarketSaved('modrinth', kind, selected.project_id) ? 'market.removeSaved' : 'market.saveLater') }}
      </button>
      <button
        class="text-button"
        @click="toggleMarketSaved('modrinth', kind, selected, 'favorite')"
      >
        {{ t(isMarketSaved('modrinth', kind, selected.project_id, 'favorite') ? 'market.removeFavorite' : 'market.favorite') }}
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
        正在读取{{ label }}版本…
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
          <label :for="`content-pack-version-${kind}`">{{ label }}版本</label>
          <VersionSelector
            :id="`content-pack-version-${kind}`"
            v-model="versionId"
            :disabled="installing"
            :options="versions.map(version => ({ value: version.version_id, label: `${version.name}${version.name !== version.version_number ? ` · ${version.version_number}` : ''} · ${versionType(version.version_type)} · ${version.date_published.slice(0, 10)}` }))"
            @update:model-value="installError = ''"
          />
        </div>
        <p class="setting-hint content-pack-use-hint">
          {{ kind === 'resource_pack' ? '安装后在游戏内选择资源包。' : '需要支持光影的游戏环境，安装后在游戏内选择。' }}
        </p>
        <div class="plan-actions">
          <button
            class="primary-button"
            :disabled="locked || installing || !chosenVersion"
            @click="install"
          >
            <span
              v-if="installing"
              class="busy-spinner"
              aria-hidden="true"
            />{{ installing ? '正在安装…' : installError ? '重试安装' : `安装${label}` }}
          </button>
          <p
            v-if="locked && !installing"
            class="mod-meta"
          >
            关闭游戏后即可安装。
          </p>
        </div>
        <p
          v-if="installError"
          class="settings-error"
          role="alert"
        >
          {{ installError }}
        </p>
      </template>
      <p
        v-else
        class="content-empty"
      >
        没有适用于当前游戏的{{ label }}版本。
      </p>
    </template>
    <template v-else>
      <form
        v-if="!sharedMode"
        class="content-toolbar discover-search"
        @submit.prevent="search()"
      >
        <SearchField
          v-model="query"
          :label="`搜索 Modrinth ${label}`"
          :placeholder="`搜索想要的${label}`"
          @submit="search()"
        />
        <button
          class="secondary-button"
          type="submit"
        >
          搜索
        </button>
      </form>
      <p
        v-if="!searched"
        class="content-empty"
      >
        输入名称，查找适用于当前游戏的{{ label }}。
      </p>
      <ul
        v-if="hits.length"
        class="mod-list discover-list"
        :aria-label="`发现${label}`"
      >
        <li
          v-for="hit in hits"
          :key="hit.project_id"
          class="mod-row ui-card-action-surface"
        >
          <MarketImage
            provider="modrinth"
            :url="hit.preview_url || hit.gallery?.[0] || hit.icon_url"
            :title="hit.title"
            :preview="!!(hit.preview_url || hit.gallery?.[0])"
            artwork
          />
          <div class="mod-info">
            <h3>{{ hit.title }}</h3><p class="mod-description">
              {{ hit.description }}
            </p><p class="mod-meta">
              {{ hit.author }} · {{ downloadCount(hit.downloads) }} 次下载
            </p>
          </div>
          <button
            class="text-button ui-stretched-action"
            type="button"
            :disabled="searching || installing"
            :data-market-project="hit.project_id"
            :aria-label="`查看 ${hit.title}`"
            @click="select(hit)"
          >
            查看
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
          @click="search(failedOffset, true)"
        >
          重试搜索
        </button>
      </div>
      <LoadingIndicator
        v-else-if="searching"
        :label="t('market.searching')"
        :skeleton="!hits.length"
        :compact="!!hits.length"
      />
      <p
        v-else-if="searched && !hits.length"
        class="content-empty"
      >
        没有找到适用的{{ label }}，换个关键词试试。
      </p>
      <button
        v-else-if="hasMore"
        class="load-more secondary-button"
        @click="search(nextOffset)"
      >
        加载更多
      </button>
    </template>
  </div>
</template>

<style scoped>.discover-search .search-field{flex:1;min-width:0}.discover-search{gap:var(--ui-space-3)}</style>
