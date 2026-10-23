<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import MarketImage from "./MarketImage.vue";
import { marketReturn } from './market-return';
import MarketGallery from "./MarketGallery.vue";
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import type { Game, ModSearchHit, ModSearchPage, ModInstallPreview, ModVersion } from './games';
import ModVersionPicker from './ModVersionPicker.vue';
const props = defineProps<{ game: Game; locked: boolean; initialQuery?: string }>();
const emit = defineEmits<{ installing: [value: boolean]; installed: [] }>();
const root = ref<HTMLElement>();
const detailReturn = marketReturn(root);
const query = ref('');
const submitted = ref('');
const hits = ref<ModSearchHit[]>([]);
const total = ref(0);
const searching = ref(false);
const searched = ref(false);
const searchError = ref('');
const selected = ref<ModSearchHit | null>(null);
const version = ref<ModVersion | null>(null);
const preview = ref<ModInstallPreview | null>(null);
const resolving = ref(false);
const installing = ref(false);
const planError = ref('');
let searchRequest = 0;
let previewRequest = 0;
let failedOffset = 0;
async function search(offset = 0, retry = false) {
  if (!props.game.loader_version || installing.value) return;
  const current = ++searchRequest;
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
    total.value = result.total_hits;
  } catch { if (current === searchRequest) searchError.value = '暂时无法获取 Mod，请稍后重试。'; }
  finally { if (current === searchRequest) searching.value = false; }
}
function select(hit: ModSearchHit) {
  if (installing.value || searching.value) return;
  detailReturn.enter(hit.project_id);
  ++previewRequest; selected.value = hit; version.value = null; preview.value = null; planError.value = ''; resolving.value = false;
}
function chooseVersion(value: ModVersion | null) {
  ++previewRequest; version.value = value; preview.value = null; planError.value = ''; resolving.value = false;
  if (value) void prepare();
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
  if (!preview.value || props.locked || installing.value) return;
  installing.value = true;
  emit('installing', true);
  planError.value = '';
  try {
    await invoke('install_game_mod_plan', { id: props.game.id, planId: preview.value.plan_id });
    emit('installed');
  } catch (cause) { planError.value = "安装未完成：" + String(cause); }
  finally { installing.value = false; emit('installing', false); }
}
watch([() => props.game.id, () => props.initialQuery], () => {
  ++searchRequest; ++previewRequest; hits.value = []; total.value = 0; searched.value = false;
  query.value = props.initialQuery ?? ''; selected.value = null; preview.value = null; searching.value = false; resolving.value = false;
  void search();
}, { immediate: true });
onUnmounted(() => { ++searchRequest; ++previewRequest; });
</script>
<template>
  <div
    ref="root"
    class="discover-content"
  >
    <p
      v-if="!game.loader_version"
      class="content-empty"
    >
      发现 Mod 适用于 Fabric、Forge 或 NeoForge 游戏。创建带模组加载器的游戏后即可浏览和安装。
    </p>
    <template v-else>
      <template v-if="selected">
        <button
          class="text-button"
          :disabled="installing"
          @click="back"
        >
          ← 返回搜索结果
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
        <template v-if="preview">
          <h3 class="plan-heading">
            本次安装 <span class="count">{{ preview.mods.length }}</span>
          </h3>
          <ul
            class="mod-list"
            aria-label="安装内容"
          >
            <li
              v-for="mod in preview.mods"
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
          <div class="plan-actions">
            <button
              class="primary-button"
              :disabled="locked || installing || !preview.mods.length"
              @click="install"
            >
              <span
                v-if="installing"
                class="busy-spinner"
                aria-hidden="true"
              />{{ installing ? '正在安装…' : planError ? '重试安装' : `安装 ${preview.mods.length} 个 Mod` }}
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
          class="content-toolbar discover-search"
          @submit.prevent="search()"
        >
          <input
            v-model="query"
            type="search"
            aria-label="搜索 Modrinth"
            placeholder="搜索想要的 Mod"
          >
          <button
            class="secondary-button"
            type="submit"
          >
            搜索
          </button>
        </form>
        <ul
          v-if="hits.length"
          class="mod-list discover-list"
          aria-label="发现 Mod"
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
              <h3>{{ hit.title }}</h3><p class="mod-description">
                {{ hit.description }}
              </p><p class="mod-meta">
                {{ hit.author }} · {{ new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(hit.downloads) }} 次下载
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
            重试
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
          没有找到适用的 Mod，换个关键词试试。
        </p>
        <button
          v-else-if="hits.length < total"
          class="load-more secondary-button"
          @click="search(hits.length)"
        >
          加载更多
        </button>
      </template>
    </template>
  </div>
</template>
