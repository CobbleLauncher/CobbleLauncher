<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { loaderLabel, type ModrinthPackVersion, type ModSearchHit, type ModSearchPage, type PackPreview } from './games';
const emit = defineEmits<{ selectedPack: [preview: PackPreview]; busy: [value: boolean] }>();
const query = ref('');
const minecraftVersion = ref('');
const loader = ref('');
const filterError = ref('');
const submitted = ref<{ query: string; minecraftVersion: string | null; loader: string | null }>({ query: '', minecraftVersion: null, loader: null });
const hits = ref<ModSearchHit[]>([]);
const total = ref(0);
const nextOffset = ref(0);
const hasMore = ref(false);
const searching = ref(false);
const searched = ref(false);
const searchError = ref('');
const selected = ref<ModSearchHit | null>(null);
const versions = ref<ModrinthPackVersion[]>([]);
const versionId = ref('');
const loadingVersions = ref(false);
const versionError = ref('');
const preparing = ref(false);
const previewError = ref('');
const chosenVersion = computed(() => versions.value.find((version) => version.version_id === versionId.value));
let searchRequest = 0;
let versionRequest = 0;
let failedOffset = 0;
function packLoader(value: string) { return value === 'minecraft' || value === 'vanilla' ? '原版' : ['fabric', 'forge', 'neoforge'].includes(value) ? loaderLabel(value) : value; }
async function search(offset = 0, retry = false) {
  if (preparing.value) return;
  filterError.value = '';
  if (!offset && !retry) {
    const version = minecraftVersion.value.trim();
    if (version && !/^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$/.test(version)) { filterError.value = 'Minecraft 版本格式无效，例如 1.21.1。'; return; }
    submitted.value = { query: query.value.trim(), minecraftVersion: version || null, loader: loader.value || null };
    selected.value = null; versions.value = []; ++versionRequest;
  }
  if (!isTauri()) { searchError.value = '请在桌面应用中浏览整合包'; return; }
  const current = ++searchRequest;
  failedOffset = offset; searching.value = true; searched.value = true; searchError.value = '';
  if (!offset) { hits.value = []; total.value = 0; hasMore.value = false; }
  try {
    const result = await invoke<ModSearchPage>('search_modrinth_packs', { ...submitted.value, offset });
    if (current !== searchRequest) return;
    hits.value = offset ? [...new Map([...hits.value, ...result.hits].map((hit) => [hit.project_id, hit])).values()] : result.hits;
    total.value = result.total_hits; nextOffset.value = result.offset + result.hits.length; hasMore.value = result.hits.length > 0 && nextOffset.value < result.total_hits;
  } catch { if (current === searchRequest) searchError.value = '暂时无法获取整合包，请重试。'; }
  finally { if (current === searchRequest) searching.value = false; }
}
async function loadVersions() {
  if (!selected.value || preparing.value) return;
  const current = ++versionRequest;
  loadingVersions.value = true; versionError.value = ''; previewError.value = ''; versions.value = []; versionId.value = '';
  try {
    const result = await invoke<ModrinthPackVersion[]>('list_modrinth_pack_versions', { projectId: selected.value.project_id, minecraftVersion: submitted.value.minecraftVersion, loader: submitted.value.loader });
    if (current !== versionRequest) return;
    versions.value = result; versionId.value = result[0]?.version_id ?? '';
  } catch (cause) { if (current === versionRequest) versionError.value = `无法读取整合包版本：${String(cause)}`; }
  finally { if (current === versionRequest) loadingVersions.value = false; }
}
function select(hit: ModSearchHit) { if (!preparing.value) { selected.value = hit; void loadVersions(); } }
function back() { if (!preparing.value) { ++versionRequest; selected.value = null; loadingVersions.value = false; } }
async function prepare() {
  if (!selected.value || !chosenVersion.value || preparing.value) return;
  preparing.value = true; previewError.value = ''; emit('busy', true);
  try { emit('selectedPack', await invoke<PackPreview>('preview_modrinth_pack', { projectId: selected.value.project_id, versionId: versionId.value })); }
  catch (cause) { previewError.value = `无法准备安装预览：${String(cause)}`; }
  finally { preparing.value = false; emit('busy', false); }
}
onUnmounted(() => { ++searchRequest; ++versionRequest; });
</script>
<template>
  <div class="page-content discover-packs-page">
    <template v-if="selected">
      <button
        class="text-button"
        :disabled="preparing"
        @click="back"
      >
        ← 返回搜索结果
      </button>
      <div class="mod-preview pack-project-preview">
        <span
          class="project-monogram"
          aria-hidden="true"
        >{{ selected.title.slice(0, 1) }}</span><div class="mod-info">
          <h2>{{ selected.title }}</h2><p class="mod-meta">
            {{ selected.author }} · {{ new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(selected.downloads) }} 次下载
          </p>
        </div>
      </div>
      <p class="preview-description">
        {{ selected.description }}
      </p>
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
          <label for="online-pack-version">整合包版本</label><select
            id="online-pack-version"
            v-model="versionId"
            :disabled="preparing"
            @change="previewError = ''"
          >
            <option
              v-for="version in versions"
              :key="version.version_id"
              :value="version.version_id"
            >
              {{ version.name }}{{ version.name !== version.version_number ? ` · ${version.version_number}` : '' }}
            </option>
          </select>
        </div>
        <div
          v-if="chosenVersion"
          class="pack-version-info"
        >
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
      <p class="pack-discovery-source">
        来自 Modrinth
      </p>
      <form
        class="pack-discovery-form"
        novalidate
        @submit.prevent="search()"
      >
        <div class="content-toolbar discover-search">
          <input
            v-model="query"
            type="search"
            maxlength="120"
            aria-label="搜索整合包"
            placeholder="搜索整合包名称或玩法"
          ><button
            class="secondary-button"
            type="submit"
          >
            {{ searching ? '搜索中…' : '搜索' }}
          </button>
        </div>
        <div class="pack-discovery-filters">
          <div class="form-field">
            <label for="pack-minecraft-filter">Minecraft 版本 <span
              class="field-note"
              aria-hidden="true"
            >可选</span></label><input
              id="pack-minecraft-filter"
              v-model="minecraftVersion"
              maxlength="64"
              autocomplete="off"
              spellcheck="false"
              placeholder="全部版本，例如 1.21.1"
              :aria-invalid="!!filterError"
            >
          </div><div class="form-field">
            <label for="pack-loader-filter">加载器</label><select
              id="pack-loader-filter"
              v-model="loader"
            >
              <option value="">
                全部
              </option><option value="fabric">
                Fabric
              </option><option value="forge">
                Forge
              </option><option value="neoforge">
                NeoForge
              </option><option value="minecraft">
                原版
              </option>
            </select>
          </div>
        </div>
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
        aria-label="发现整合包"
      >
        <li
          v-for="hit in hits"
          :key="hit.project_id"
          class="mod-row"
        >
          <span
            class="project-monogram"
            aria-hidden="true"
          >{{ hit.title.slice(0, 1) }}</span><div class="mod-info">
            <h2>{{ hit.title }}</h2><p class="mod-description">
              {{ hit.description }}
            </p><p class="mod-meta">
              {{ hit.author }} · {{ new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(hit.downloads) }} 次下载
            </p>
          </div><button
            class="secondary-button"
            :aria-label="`查看 ${hit.title}`"
            @click="select(hit)"
          >
            查看版本
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
          重试搜索
        </button>
      </div>
      <p
        v-else-if="searching"
        class="content-empty"
        role="status"
      >
        正在搜索整合包…
      </p>
      <p
        v-else-if="!searched"
        class="content-empty"
      >
        输入名称搜索，或直接搜索浏览整合包。
      </p>
      <p
        v-else-if="!hits.length"
        class="content-empty"
      >
        没有找到符合条件的整合包，试试其他关键词或筛选。
      </p>
      <div
        v-else
        class="pack-results-footer"
      >
        <p class="mod-meta">
          已显示 {{ hits.length }} / {{ total }} 个整合包
        </p><button
          v-if="hasMore"
          class="secondary-button"
          :disabled="searching"
          @click="search(nextOffset)"
        >
          加载更多
        </button>
      </div>
    </template>
  </div>
</template>
