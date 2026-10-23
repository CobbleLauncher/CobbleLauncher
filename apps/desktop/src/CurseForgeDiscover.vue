<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import MarketImage from "./MarketImage.vue";
import { marketReturn } from './market-return';
import MarketGallery from "./MarketGallery.vue";
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import CurseForgePackPreparation from './CurseForgePackPreparation.vue';
import CurseForgeInstallPlan from './CurseForgeInstallPlan.vue';
import VersionSelector from './VersionSelector.vue';
import type { Game } from './games';
import { mergeFiles, mergeProjects, type CurseForgePackPreview, type CurseForgeHit, type CurseForgeFile, type CurseForgeSearchPage, type CurseForgeFilePage } from './curseforge';
import LoadingIndicator from './ui/LoadingIndicator.vue';
const props = defineProps<{kind: 'mod' | 'modpack'; game?: Game; locked?: boolean; initialQuery?: string}>();
const emit = defineEmits<{busy: [value: boolean]; selectedCfPack: [preview: CurseForgePackPreview]; installed: []}>();
const root = ref<HTMLElement>();
const detailReturn = marketReturn(root);
const query = ref('');
const minecraftVersion = ref('');
const loader = ref('');
const available = ref<boolean | null>(null);
const busy = ref(false);
const error = ref('');
const searched = ref(false);
const hits = ref<CurseForgeHit[]>([]);
const total = ref(0);
const next = ref(0);
const selected = ref<CurseForgeHit | null>(null);
const files = ref<CurseForgeFile[]>([]);
const fileTotal = ref(0);
const fileNext = ref(0);
const fileId = ref('');
const installing = ref(false);
const packPreview = ref<CurseForgePackPreview | null>(null);
function installBusy(value: boolean) { installing.value = value; emit('busy', value); }
const chosen = computed(() => files.value.find(file => file.file_id === fileId.value));
const filterSummary = computed(() => [minecraftVersion.value.trim(), loader.value].filter(Boolean).join(' · ') || t('market.filters'));
let revision = 0;
let failedOffset = 0;
let submitted = {query: '', minecraftVersion: null as string | null, loader: null as string | null};
async function availability() {
  const current = ++revision;
  busy.value = true; error.value = '';
  try { const value = await invoke<{available: boolean}>('get_curseforge_availability'); if (current === revision) available.value = value.available; }
  catch { if (current === revision) error.value = t('curseforge.serviceFailed'); }
  finally {
    if (current === revision) {
      busy.value = false;
      if (available.value) void search();
    }
  }
}
async function search(offset = 0) {
  failedOffset = offset;
  if (busy.value || !available.value) return;
  if (!offset) {
    const version = props.game?.version ?? minecraftVersion.value.trim();
    const selectedLoader = props.game?.loader_kind ?? loader.value;
    if (selectedLoader && !version) { error.value = t('curseforge.versionRequired'); return; }
    submitted = {query: query.value.trim(), minecraftVersion: version || null, loader: selectedLoader || null};
  }
  const current = ++revision;
  busy.value = true; error.value = ''; searched.value = true;
  try {
    const page = await invoke<CurseForgeSearchPage>('search_curseforge', {...submitted, kind: props.kind, offset});
    if (current !== revision) return;
    hits.value = offset ? mergeProjects(hits.value, page.hits) : page.hits;
    total.value = page.total_hits; next.value = page.offset + page.hits.length;
  } catch { if (current === revision) error.value = t('curseforge.searchFailed'); }
  finally { if (current === revision) busy.value = false; }
}
async function loadFiles(offset = 0) {
  failedOffset = offset;
  if (!selected.value || busy.value) return;
  const current = ++revision;
  busy.value = true; error.value = '';
  try {
    const page = await invoke<CurseForgeFilePage>('list_curseforge_files', {projectId: selected.value.project_id, minecraftVersion: submitted.minecraftVersion, loader: submitted.loader, offset});
    if (current !== revision) return;
    files.value = offset ? mergeFiles(files.value, page.files) : page.files;
    fileTotal.value = page.total_hits; fileNext.value = page.offset + page.files.length;
    if (!offset) fileId.value = files.value[0]?.file_id ?? '';
  } catch { if (current === revision) error.value = t('curseforge.filesFailed'); }
  finally { if (current === revision) busy.value = false; }
}
function select(hit: CurseForgeHit) { if (busy.value || installing.value) return; detailReturn.enter(hit.project_id); selected.value = hit; files.value = []; fileId.value = ''; void loadFiles(); }

async function preparePack() {
  if (!selected.value || !chosen.value || installing.value) return;
  const current = ++revision; installBusy(true); error.value = '';
  try { const result = await invoke<CurseForgePackPreview | null>('preview_curseforge_online_pack', {projectId: selected.value.project_id, fileId: chosen.value.file_id}); if (current === revision && result) packPreview.value = result; }
  catch (cause) { if (current === revision) error.value = t(String(cause) === 'manual_file_mismatch' ? 'curseforge.fileMismatch' : 'curseforge.packPrepareFailed'); }
  finally { installBusy(false); }
}
function reset() { packPreview.value = null; ++revision; busy.value = false; selected.value = null; hits.value = []; searched.value = false; error.value = ''; void availability(); }
watch([() => props.kind, () => props.game?.id, () => props.initialQuery], () => { query.value = props.initialQuery ?? ''; reset(); }, {immediate: true});
onUnmounted(() => { ++revision; });
</script>
<template>
  <div
    ref="root"
    class="curseforge-discover"
  >
    <div v-if="packPreview">
      <button
        class="text-button"
        :disabled="installing"
        @click="packPreview = null"
      >
        {{ t('curseforge.back') }}
      </button><CurseForgePackPreparation
        :preview="packPreview"
        @busy="installBusy"
        @selected="emit('selectedCfPack', $event)"
      />
    </div>
    <p
      v-else-if="available === false"
      class="content-empty"
      role="status"
    >
      {{ t('curseforge.unavailable') }}
    </p>
    <template v-else-if="available">
      <template v-if="selected">
        <button
          class="text-button"
          :disabled="busy || installing"
          @click="selected = null; error = ''; detailReturn.restore()"
        >
          {{ t('curseforge.back') }}
        </button>
        <div class="mod-preview">
          <MarketImage
            provider="curseforge"
            :url="selected.icon_url"
            :title="selected.title"
          /><h2>{{ selected.title }}</h2>
        </div>
        <p class="preview-description">
          {{ selected.description }}
        </p>
        <MarketGallery
          v-if="selected.gallery?.length"
          provider="curseforge"
          :urls="selected.gallery"
          :title="selected.title"
        />
        <div
          v-if="files.length"
          class="form-field"
        >
          <label for="cf-file">{{ t('curseforge.file') }}</label>
          <VersionSelector
            id="cf-file"
            v-model="fileId"
            :disabled="busy || installing"
            :options="files.map(file => ({value: file.file_id, label: file.name}))"
          />
        </div>
        <template v-if="chosen">
          <div class="compatibility-tags">
            <span
              v-for="version in chosen.game_versions"
              :key="version"
            >{{ version }}</span>
          </div>
          <p class="mod-meta">
            {{ chosen.file_name }} · {{ chosen.date_published.slice(0, 10) }}
          </p>
          <p
            v-if="chosen.download_availability !== 'available'"
            role="status"
          >
            {{ t(chosen.download_availability === 'manual' ? 'curseforge.manual' : 'curseforge.blocked') }}
          </p>
          <button
            v-if="kind === 'modpack'"
            class="primary-button"
            :disabled="busy || installing"
            @click="preparePack"
          >
            {{ t(installing ? 'curseforge.preparing' : chosen.download_availability === 'unavailable' ? 'curseforge.chooseLocalArchive' : chosen.download_availability === 'manual' ? 'curseforge.chooseArchive' : 'curseforge.downloadPack') }}
          </button>
          <CurseForgeInstallPlan
            v-if="kind === 'mod' && game"
            :key="chosen.file_id"
            :game-id="game.id"
            :project-id="selected.project_id"
            :file-id="chosen.file_id"
            :locked="!!locked"
            @busy="installBusy"
            @installed="emit('installed')"
          />
        </template>
        <p
          v-if="!busy && !files.length && !error"
          class="content-empty"
        >
          {{ t('curseforge.noFiles') }}
        </p>
        <button
          v-if="fileNext > 0 && fileNext < fileTotal"
          class="secondary-button"
          :disabled="busy"
          @click="loadFiles(fileNext)"
        >
          {{ t('curseforge.loadMore') }}
        </button>
      </template>
      <template v-else>
        <form @submit.prevent="search()">
          <div class="content-toolbar discover-search">
            <input
              v-model="query"
              type="search"
              maxlength="120"
              :aria-label="t('curseforge.searchLabel')"
              :placeholder="t('curseforge.searchLabel')"
            ><button
              type="submit"
              class="secondary-button"
              :disabled="busy"
            >
              {{ t('curseforge.search') }}
            </button>
          </div>
          <details
            v-if="kind === 'modpack'"
            class="cf-filter-details"
          >
            <summary>{{ filterSummary }}</summary>
            <div class="cf-filters">
              <div class="form-field">
                <label for="cf-minecraft">{{ t('curseforge.version') }}</label><input
                  id="cf-minecraft"
                  v-model="minecraftVersion"
                  maxlength="64"
                  :placeholder="t('curseforge.allVersions')"
                >
              </div>
              <div class="form-field">
                <label for="cf-loader">{{ t('curseforge.loader') }}</label><select
                  id="cf-loader"
                  v-model="loader"
                >
                  <option value="">
                    {{ t('curseforge.allLoaders') }}
                  </option><option
                    v-for="value in ['fabric', 'forge', 'neoforge', 'quilt']"
                    :key="value"
                    :value="value"
                  >
                    {{ value }}
                  </option>
                </select>
              </div>
            </div>
          </details>
        </form>
        <ul
          v-if="hits.length"
          class="mod-list discover-list"
          :aria-label="t('curseforge.searchLabel')"
        >
          <li
            v-for="hit in hits"
            :key="hit.project_id"
            class="mod-row pack-result-card ui-card-action-surface"
            :aria-disabled="busy || installing"
          >
            <MarketImage
              provider="curseforge"
              :url="kind === 'modpack' ? (hit.preview_url || hit.gallery?.[0] || hit.icon_url) : hit.icon_url"
              :title="hit.title"
              :preview="kind === 'modpack' && !!(hit.preview_url || hit.gallery?.[0])"
              :artwork="kind === 'modpack'"
            /><div class="mod-info">
              <h2>{{ hit.title }}</h2><p class="mod-description">
                {{ hit.description }}
              </p><p class="mod-meta">
                {{ hit.author }}
              </p>
            </div>
            <button
              type="button"
              class="text-button ui-stretched-action"
              :disabled="busy || installing"
              :data-market-project="hit.project_id"
              :aria-label="`查看 ${hit.title}`"
              @click="select(hit)"
            >
              {{ t('curseforge.versions') }}
            </button>
          </li>
        </ul>
        <p
          v-if="searched && !busy && !hits.length && !error"
          class="content-empty"
        >
          {{ t('curseforge.empty') }}
        </p>
        <div
          v-if="hits.length"
          class="cf-results"
        >
          <span class="mod-meta">{{ t('curseforge.shown', {shown: hits.length, total}) }}</span><button
            v-if="next > 0 && next < total"
            class="secondary-button"
            :disabled="busy"
            @click="search(next)"
          >
            {{ t('curseforge.loadMore') }}
          </button>
        </div>
      </template>
    </template>
    <LoadingIndicator
      v-if="busy"
      :label="t('curseforge.loading')"
      :skeleton="searched && !hits.length && available === true"
      :compact="!!hits.length || !!selected"
    />
    <div
      v-if="error"
      role="alert"
      class="inline-error"
    >
      <p>{{ error }}</p><button
        class="text-button"
        :disabled="busy || installing"
        @click="available === null ? availability() : selected ? loadFiles(failedOffset) : search(failedOffset)"
      >
        {{ t('curseforge.retry') }}
      </button>
    </div>
  </div>
</template>
<style scoped>
.curseforge-discover { padding-top: 20px; overflow-wrap: anywhere; }
.cf-filter-details { margin-top: 10px; }
.cf-filter-details summary { width: fit-content; color: var(--muted); font-size: 11px; cursor: pointer; }
.cf-filters { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 12px; }
.cf-results { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-top: 16px; }
.curseforge-discover .pack-result-card{cursor:pointer;border:1px solid var(--line);border-radius:12px;padding:12px;transition:background-color .14s ease,border-color .14s ease}
.curseforge-discover .pack-result-card:not([aria-disabled="true"]):hover{background:color-mix(in srgb,var(--panel) 88%,#000 12%);border-color:var(--muted)}
.curseforge-discover .pack-result-card:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.curseforge-discover .pack-result-card[aria-disabled="true"]{cursor:default;opacity:.72}
@media(prefers-reduced-motion:reduce){.curseforge-discover .pack-result-card{transition:none}}
@media(max-width: 560px) { .cf-filters { grid-template-columns: 1fr; } .mod-row { flex-wrap: wrap; } }
</style>





