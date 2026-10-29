<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import SearchField from './ui/SearchField.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import MarketImage from "./MarketImage.vue";
import { marketReturn } from './market-return';
import MarketGallery from "./MarketGallery.vue";
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import type { Game, ModSearchHit, ModSearchPage, ModInstallPreview, ModVersion } from './games';
import { beginMarketInstall, endMarketInstall, marketInstallPending, modInstallResult, type MarketInstallResult } from './market-install-result';
import { useDelayedBusy } from './market/use-delayed-busy';
import type { ManagedGameMod } from './games';
import ModVersionPicker from './ModVersionPicker.vue';
import AppIcon from './ui/AppIcon.vue';
import { isMarketSaved, toggleMarketSaved, type SavedMarketItem } from './market-saved';
import { marketWorkstate, rememberMarketWorkstate, restoreMarketScroll } from './market-workstate';
import { findLinkedProject, type FileInboxMarketLink } from './file-inbox-market';
const props = defineProps<{ game: Game; locked: boolean; installedRecords?: ManagedGameMod[]; initialQuery?: string; sharedMode?: boolean; sharedRequest?: { serial: number; query: string }; savedRequest?: { serial: number; item: SavedMarketItem }; linkRequest?: FileInboxMarketLink }>();
const emit = defineEmits<{ installing: [value: boolean]; installed: [result: MarketInstallResult] }>();
const root = ref<HTMLElement>();
const detailReturn = marketReturn(root);
const query = ref('');
const submitted = ref('');
const hits = ref<ModSearchHit[]>([]);
const total = ref(0);
const searching = ref(false);
const showSearching = useDelayedBusy(searching);
const searched = ref(false);
const searchError = ref('');
const linkProjectMissing = ref(false);
const selected = ref<ModSearchHit | null>(null);
const version = ref<ModVersion | null>(null);
const preview = ref<ModInstallPreview | null>(null);
const resolving = ref(false);
const installing = ref(false);
const planError = ref('');
const completedVersions = ref<Record<string, string>>({});
const selectedRecord = computed(() => props.installedRecords?.find(record => record.project_id === selected.value?.project_id));
const selectedInstalled = computed(() => !!selected.value && !!version.value && (selectedRecord.value?.version_id === version.value.version_id || completedVersions.value[selected.value.project_id] === version.value.version_id));
const installLocked = computed(() => props.locked || marketInstallPending(props.game.id));
function projectInstalled(projectId: string) { return !!completedVersions.value[projectId] || props.installedRecords?.some(record => record.project_id === projectId); }
let searchRequest = 0;
let previewRequest = 0;
let failedOffset = 0;
async function search(offset = 0, retry = false) {
  if (!props.game.loader_version || installing.value) return;
  const current = ++searchRequest;
  let linked: ModSearchHit | undefined;
  if (!offset) linkProjectMissing.value = false;
  const id = props.game.id;
  if (!offset && !retry) submitted.value = query.value.trim();
  failedOffset = offset;
  searching.value = true;
  searched.value = true;
  searchError.value = '';
  try {
    const result = await invoke<ModSearchPage>('search_game_mods', { id, query: submitted.value, offset });
    if (current !== searchRequest) return;
    hits.value = offset ? [...hits.value, ...result.hits] : result.hits;
    if (!offset && props.linkRequest) { linked = findLinkedProject(hits.value, props.linkRequest); linkProjectMissing.value = !linked; }
    total.value = result.total_hits;
  } catch { if (current === searchRequest) searchError.value = t('market.modSearchFailed'); }
  finally { if (current === searchRequest) { searching.value = false; if (linked) select(linked); } }
}
function select(hit: ModSearchHit) {
  if (installing.value || searching.value) return;
  detailReturn.enter(hit.project_id);
  ++previewRequest; selected.value = hit; version.value = null; preview.value = null; planError.value = ''; resolving.value = false;
}
function chooseVersion(value: ModVersion | null) {
  ++previewRequest; version.value = value; preview.value = null; planError.value = ''; resolving.value = false;
  if (value && !selectedInstalled.value) void prepare();
}
async function prepare() {
  if (!selected.value || !version.value || resolving.value || installing.value) return;
  const current = ++previewRequest;
  preview.value = null;
  planError.value = '';
  resolving.value = true;
  try {
    const result = await invoke<ModInstallPreview>('preview_game_mod_install', { id: props.game.id, projectId: selected.value.project_id, versionId: version.value.version_id });
    if (current === previewRequest) preview.value = result;
  } catch (cause) { if (current === previewRequest) planError.value = "无法准备安装：" + String(cause); }
  finally { if (current === previewRequest) resolving.value = false; }
}
function back() { if (!installing.value) { ++previewRequest; selected.value = null; preview.value = null; resolving.value = false; detailReturn.restore(); } }
async function install() {
  if (!preview.value || installLocked.value || installing.value || selectedInstalled.value) return;
  const id = props.game.id;
  const plan = preview.value;
  if (!beginMarketInstall(id)) return;
  installing.value = true;
  emit('installing', true);
  planError.value = '';
  try {
    await invoke('install_game_mod_plan', { id, planId: plan.plan_id });
    if (id === props.game.id) for (const mod of plan.mods) completedVersions.value[mod.project_id] = mod.version_id;
    emit('installed', await modInstallResult(id, 'modrinth', plan.mods));
  } catch (cause) { planError.value = "安装未完成：" + String(cause); }
  finally { endMarketInstall(id); installing.value = false; emit('installing', false); }
}
watch(() => props.game.id, () => { completedVersions.value = {}; });
watch([() => props.game.id, () => props.initialQuery], () => {
  ++searchRequest; ++previewRequest; hits.value = []; total.value = 0; searched.value = false;
  query.value = props.sharedMode ? props.sharedRequest?.query ?? props.initialQuery ?? '' : props.initialQuery ?? ''; selected.value = null; preview.value = null; searching.value = false; resolving.value = false;
  void search();
}, { immediate: true });
watch(() => props.savedRequest?.serial, () => {
  const item = props.savedRequest?.item;
  if (item?.provider === 'modrinth' && item.kind === 'mod' && item.hit.project_id === item.project_id) {
    ++searchRequest; searching.value = false; select(item.hit as ModSearchHit);
  }
}, { immediate: true });
watch(() => props.sharedRequest?.serial, () => {
  if (!props.sharedMode || !props.sharedRequest || installing.value) return;
  query.value = props.sharedRequest.query;
  selected.value = null; preview.value = null; ++previewRequest;
  void search();
});
function workKey() { return `modrinth:mod:${props.game.id}`; }
function remember() {
  rememberMarketWorkstate(workKey(), { query: query.value, submitted: submitted.value, hits: hits.value, total: total.value, searched: searched.value }, root.value);
}
onMounted(() => {
  if (props.savedRequest || props.linkRequest) return;
  const state = marketWorkstate<{ query: string; submitted: string; hits: ModSearchHit[]; total: number; searched: boolean }>(workKey());
  if (state && (!props.sharedMode || state.query === (props.sharedRequest?.query ?? props.initialQuery ?? ''))) { ++searchRequest; query.value = state.query; submitted.value = state.submitted; hits.value = state.hits; total.value = state.total; searched.value = state.searched; searching.value = false; restoreMarketScroll(workKey(), root.value); }
});
onActivated(() => restoreMarketScroll(workKey(), root.value));
onDeactivated(remember);
onUnmounted(() => { remember(); ++searchRequest; ++previewRequest; });
</script>
<template>
  <div
    ref="root"
    class="discover-content"
    :aria-busy="searching || installing"
  >
    <p
      v-if="linkProjectMissing && !selected"
      role="status"
      class="setting-hint"
    >
      {{ locale === 'zh-CN' ? '未在当前结果中找到该项目。可调整搜索或筛选。' : 'Project not found in these results. Try another search or filter.' }}
    </p>
    <p
      v-if="!game.loader_version"
      class="content-empty"
    >
      发现 Mod 适用于 Fabric、Forge 或 NeoForge 游戏。创建带模组加载器的游戏后即可浏览和安装。
    </p>
    <template v-else>
      <template v-if="selected">
        <button
          class="text-button market-back-button"
          :disabled="installing"
          @click="back"
        >
          <AppIcon name="arrow-left" />{{ t('market.backResults') }}
        </button>
        <div class="mod-preview">
          <MarketImage
            provider="modrinth"
            :url="selected.icon_url"
            :title="selected.title"
          />
          <div class="mod-info">
            <h2>{{ selected.title }}</h2><p class="mod-meta">
              {{ selected.author }}
            </p>
          </div>
        </div>
        <p class="preview-description">
          {{ selected.description }}
        </p>
        <button
          class="text-button market-save-button"
          @click="toggleMarketSaved('modrinth', 'mod', selected)"
        >
          {{ t(isMarketSaved('modrinth', 'mod', selected.project_id) ? 'market.removeSaved' : 'market.saveLater') }}
        </button>
        <button
          class="text-button"
          @click="toggleMarketSaved('modrinth', 'mod', selected, 'favorite')"
        >
          {{ t(isMarketSaved('modrinth', 'mod', selected.project_id, 'favorite') ? 'market.removeFavorite' : 'market.favorite') }}
        </button>
        <MarketGallery
          v-if="selected.gallery?.length"
          provider="modrinth"
          :urls="selected.gallery"
          :title="selected.title"
        />
        <ModVersionPicker
          :key="selected.project_id"
          :game-id="game.id"
          :project-id="selected.project_id"
          :disabled="installing"
          :linked-version="linkRequest?.version"
          :current-version-id="selectedRecord?.version_id"
          @selected="chooseVersion"
        />
        <button
          v-if="version && !preview && planError"
          class="secondary-button mod-version-preview-button"
          :disabled="resolving || installing"
          @click="prepare"
        >
          {{ resolving ? '准备中…' : '重试预览安装' }}
        </button>
        <p
          v-if="resolving"
          class="content-empty"
          role="status"
        >
          正在查找版本和必需依赖…
        </p>
        <p
          v-if="selectedInstalled"
          class="setting-hint"
          role="status"
        >
          {{ t('market.selectedInstalled') }}
        </p>
        <template v-if="preview && !selectedInstalled">
          <h3 class="plan-heading">
            本次安装 <span class="count">{{ preview.mods.length }}</span>
          </h3>
          <ul
            class="mod-list"
            aria-label="安装内容"
          >
            <li
              v-for="mod in preview.mods.filter(item => !item.required_by)"
              :key="mod.project_id"
              class="mod-row"
            >
              <div class="mod-info">
                <h3>{{ mod.name }}</h3><p class="mod-meta">
                  {{ mod.version_number }}
                </p><p
                  v-if="mod.required_by"
                  class="mod-meta"
                >
                  {{ mod.required_by }} 的必需依赖
                </p>
              </div>
              <span class="mod-state">{{ mod.required_by ? '必需依赖' : '所选 Mod' }}</span>
            </li>
          </ul>
          <SettingsDisclosure
            v-if="preview.mods.some(item => item.required_by)"
            :key="preview.plan_id"
            :title="t('market.dependencies', { count: preview.mods.filter(item => item.required_by).length })"
          >
            <ul class="mod-list">
              <li
                v-for="mod in preview.mods.filter(item => item.required_by)"
                :key="mod.project_id"
                class="mod-row"
              >
                <div class="mod-info">
                  <h3>{{ mod.name }}</h3><p class="mod-meta">
                    {{ mod.version_number }}
                  </p><p class="mod-meta">
                    {{ mod.required_by }}
                  </p>
                </div>
              </li>
            </ul>
          </SettingsDisclosure>
          <div class="plan-actions">
            <button
              class="primary-button"
              :disabled="installLocked || installing || !preview.mods.length"
              @click="install"
            >
              <span
                v-if="installing"
                class="busy-spinner"
                aria-hidden="true"
              />{{ installing ? '正在安装…' : planError ? '重试安装' : selectedRecord ? t('modUpdates.changeVersion') : `安装 ${preview.mods.length} 个 Mod` }}
            </button>
            <p
              v-if="installing"
              class="mod-meta"
              role="status"
            >
              正在下载并安装，请稍候。
            </p>
            <p
              v-else-if="locked"
              class="mod-meta"
            >
              关闭游戏后即可安装。
            </p>
          </div>
        </template>
        <div
          v-if="planError"
          class="inline-error"
          role="alert"
        >
          <p>{{ planError }}</p>
        </div>
      </template>
      <template v-else>
        <form
          v-if="!sharedMode"
          class="content-toolbar discover-search"
          @submit.prevent="search()"
        >
          <SearchField
            v-model="query"
            :label="t('market.searchMods')"
            :placeholder="t('market.modSearchPlaceholder')"
            @submit="search()"
          />
          <button
            class="secondary-button"
            type="submit"
          >
            {{ t('curseforge.search') }}
          </button>
        </form>
        <ul
          v-if="hits.length"
          class="mod-list discover-list"
          :aria-label="t('market.mod')"
        >
          <li
            v-for="hit in hits"
            :key="hit.project_id"
            class="mod-row ui-card-action-surface"
          >
            <MarketImage
              provider="modrinth"
              :url="hit.icon_url"
              :title="hit.title"
            />
            <div class="mod-info">
              <h3>{{ hit.title }}</h3><span
                v-if="projectInstalled(hit.project_id)"
                class="mod-state"
              >{{ t('market.alreadyInstalled') }}</span><p class="mod-description">
                {{ hit.description }}
              </p><p class="mod-meta">
                {{ hit.author }} · {{ t('market.downloads', { count: new Intl.NumberFormat(locale, { notation: 'compact' }).format(hit.downloads) }) }}
              </p>
            </div>
            <button
              class="text-button ui-stretched-action"
              type="button"
              :disabled="searching || installing"
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
            @click="search(failedOffset, true)"
          >
            {{ t('market.retry') }}
          </button>
        </div>
        <LoadingIndicator
          v-else-if="showSearching"
          :label="t('market.searching')"
          :skeleton="!hits.length"
          :compact="!!hits.length"
        />
        <p
          v-else-if="searched && !searching && !hits.length"
          class="content-empty"
        >
          {{ t('market.emptyMods') }}
        </p>
        <button
          v-else-if="!searching && hits.length < total"
          class="load-more secondary-button"
          @click="search(hits.length)"
        >
          {{ t('market.loadMore') }}
        </button>
      </template>
    </template>
  </div>
</template>

<style scoped>.discover-search .search-field{flex:1;min-width:0}.discover-search{gap:var(--ui-space-3)}</style>
