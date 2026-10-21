<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, ModSearchHit, ModSearchPage, ModInstallPreview } from './games';
import { loaderLabel } from './games';
const props = defineProps<{ game: Game; locked: boolean }>();
const emit = defineEmits<{ installing: [value: boolean]; installed: [] }>();
const query = ref('');
const submitted = ref('');
const hits = ref<ModSearchHit[]>([]);
const total = ref(0);
const searching = ref(false);
const searched = ref(false);
const searchError = ref('');
const selected = ref<ModSearchHit | null>(null);
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
  if (!offset) { hits.value = []; total.value = 0; }
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
async function select(hit: ModSearchHit) {
  if (installing.value) return;
  const current = ++previewRequest;
  selected.value = hit;
  preview.value = null;
  planError.value = '';
  resolving.value = true;
  try {
    const result = await invoke<ModInstallPreview>('preview_game_mod_install', { id: props.game.id, projectId: hit.project_id });
    if (current === previewRequest) preview.value = result;
  } catch (cause) { if (current === previewRequest) planError.value = "无法准备安装：" + String(cause); }
  finally { if (current === previewRequest) resolving.value = false; }
}
function back() { if (!installing.value) { ++previewRequest; selected.value = null; preview.value = null; resolving.value = false; } }
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
watch(() => props.game.id, () => {
  ++searchRequest; ++previewRequest; hits.value = []; total.value = 0; searched.value = false;
  query.value = ''; selected.value = null; preview.value = null; searching.value = false; resolving.value = false;
});
onUnmounted(() => { ++searchRequest; ++previewRequest; });
</script>
<template>
  <div class="discover-content">
    <p
      v-if="!game.loader_version"
      class="content-empty"
    >
      发现 Mod 适用于 Fabric、Forge 或 NeoForge 游戏。创建带模组加载器的游戏后即可浏览和安装。
    </p>
    <template v-else>
      <div class="discover-heading">
        <p class="mod-meta">
          来自 Modrinth
        </p>
        <div class="compatibility-tags">
          <span>Minecraft {{ game.version }}</span><span>{{ loaderLabel(game.loader_kind ?? 'fabric') }}</span>
        </div>
      </div>
      <template v-if="selected">
        <button
          class="text-button"
          :disabled="installing"
          @click="back"
        >
          ← 返回搜索结果
        </button>
        <div class="mod-preview">
          <span
            class="project-monogram"
            aria-hidden="true"
          >{{ selected.title.slice(0, 1) }}</span>
          <div class="mod-info">
            <h2>{{ selected.title }}</h2><p class="mod-meta">
              {{ selected.author }}
            </p>
          </div>
        </div>
        <p class="preview-description">
          {{ selected.description }}
        </p>
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
          <p>{{ planError }}</p><button
            v-if="!preview"
            class="text-button"
            :disabled="resolving"
            @click="select(selected)"
          >
            重试
          </button>
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
        <p
          v-if="!searched"
          class="content-empty"
        >
          为这个游戏寻找新玩法。输入名称，搜索适用的 Mod。
        </p>
        <ul
          v-if="hits.length"
          class="mod-list discover-list"
          aria-label="发现 Mod"
        >
          <li
            v-for="hit in hits"
            :key="hit.project_id"
            class="mod-row"
          >
            <span
              class="project-monogram"
              aria-hidden="true"
            >{{ hit.title.slice(0, 1) }}</span>
            <div class="mod-info">
              <h3>{{ hit.title }}</h3><p class="mod-description">
                {{ hit.description }}
              </p><p class="mod-meta">
                {{ hit.author }} · {{ new Intl.NumberFormat('zh-CN', { notation: 'compact' }).format(hit.downloads) }} 次下载
              </p>
            </div>
            <button
              class="secondary-button"
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
        <p
          v-else-if="searching"
          class="content-empty"
          role="status"
        >
          正在搜索…
        </p>
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
