<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Game } from './games';
import type { OpenedBlueprint, SavedBlueprint } from './blueprints';
import { canPreviewOnlineBlueprint, type OnlineBlueprint, type OnlineBlueprintOutput, type BlueprintProviderAvailability, type BlueprintProviderProgress, type ProviderCandidateSelection, type BlueprintViewerChoices } from './online-blueprints';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';

const props = withDefaults(defineProps<{ games?: Game[]; initialGameId?: string }>(), { games: () => [], initialGameId: '' });
const emit = defineEmits<{ opened: [value: OpenedBlueprint]; saved: [value?: SavedBlueprint]; library: []; extensions: [candidate?: ProviderCandidateSelection]; busy: [value: boolean] }>();
const availability = ref<BlueprintProviderAvailability | null>(null);
const providerId = ref('');
const selectedProvider = computed(() => availability.value?.providers.find(provider => provider.id === providerId.value));
const gameId = ref(props.initialGameId || props.games.find(game => game.status === 'installed')?.id || '');
const query = ref(''); const format = ref(''); const mod = ref('');
const items = ref<OnlineBlueprint[]>([]); const cursor = ref<string | null>(null); const searched = ref(false);
const selected = ref<OnlineBlueprint | null>(null);
const searchScope = ref<'all_versions' | null>(null);
const viewerChoices = ref<BlueprintViewerChoices | null>(null);
const viewerId = ref('');
const viewerLoading = ref(false);
const viewerError = ref('');
const selectedViewer = computed(() => viewerChoices.value?.viewers.find(viewer => viewer.id === viewerId.value));
const saved = ref(false);
const busy = ref(false); const error = ref(''); const progress = ref<BlueprintProviderProgress | null>(null);
const installedGames = computed(() => props.games.filter(game => game.status === 'installed'));
const game = computed(() => installedGames.value.find(game => game.id === gameId.value));
const detailHeading = ref<HTMLElement | null>(null);
let viewerRevision = 0;
let current = ''; let disposed = false; let unlisten: UnlistenFn | undefined; let filterRevision = 0;
async function availabilityCheck() {
  error.value = '';
  try {
    const value = await invoke<BlueprintProviderAvailability>('get_blueprint_provider_availability');
    if (!disposed) {
      availability.value = value;
      if (!value.providers.some(provider => provider.id === providerId.value)) providerId.value = value.provider_id ?? '';
    }
  }
  catch { if (!disposed) error.value = t('blueprintOnline.unavailable'); }
}
async function cancel() {
  const id = current; current = ''; busy.value = false; emit('busy', false); progress.value = null;
  if (id) await invoke('cancel_blueprint_provider', { requestId: id }).catch(() => {});
}
async function request<T>(command: string, args: Record<string, unknown>, accept: (value: T) => void) {
  const source = providerId.value; const revision = filterRevision;
  await cancel();
  if (disposed || revision !== filterRevision || source !== providerId.value) return;
  const id = crypto.randomUUID(); current = id; busy.value = true; emit('busy', true); error.value = '';
  try {
    const value = await invoke<T>(command, { ...args, providerId: source, requestId: id });
    if (disposed || current !== id) {
      if (command === 'open_online_blueprint' && value) await invoke('close_viewer', { resourceId: (value as unknown as OpenedBlueprint).resourceId }).catch(() => {});
      return;
    }
    accept(value);
  } catch (cause) { if (!disposed && current === id) error.value = String(cause); }
  finally { if (current === id) { current = ''; busy.value = false; emit('busy', false); progress.value = null; } }
}
async function search(more = false) {
  if (!game.value || !selectedProvider.value) return;
  selected.value = null;
  saved.value = false;
  await request<OnlineBlueprintOutput>('search_online_blueprints', { gameId: game.value.id, query: query.value, format: format.value || null, modNamespace: mod.value.trim() || null, cursor: more ? cursor.value : null }, value => {
    searchScope.value = value.search_scope ?? null;
    items.value = more ? [...items.value, ...value.items] : value.items; cursor.value = value.next_cursor; searched.value = true;
  });
}
async function details(item: OnlineBlueprint) {
  selected.value = item;
  await request<OnlineBlueprintOutput>('online_blueprint_details', { id: item.id, version: item.version }, value => {
    if (value.detail) selected.value = { ...value.detail, description: value.detail.description || item.description, java_release: value.detail.java_release || item.java_release, namespaces: value.detail.namespaces.length ? value.detail.namespaces : item.namespaces };

  });
  await nextTick(); detailHeading.value?.focus();
}
async function loadViewerChoices() {
  const revision = ++viewerRevision;
  viewerChoices.value = null; viewerId.value = ''; viewerError.value = ''; viewerLoading.value = false;
  if (!selected.value || !providerId.value || !canPreviewOnlineBlueprint(selected.value)) return;
  viewerLoading.value = true;
  try {
    const choices = await invoke<BlueprintViewerChoices>('get_blueprint_viewer_choices', { providerId: providerId.value, format: selected.value.format });
    if (disposed || revision !== viewerRevision) return;
    viewerChoices.value = choices;
    viewerId.value = choices.viewer_id ?? '';
  } catch (cause) { if (!disposed && revision === viewerRevision) viewerError.value = String(cause); }
  finally { if (!disposed && revision === viewerRevision) viewerLoading.value = false; }
}
async function preview() {
  if (!selected.value || !selectedViewer.value || !canPreviewOnlineBlueprint(selected.value)) return;
  await request<OpenedBlueprint>('open_online_blueprint', { id: selected.value.id, version: selected.value.version, viewerId: selectedViewer.value.id }, value => {
    saved.value = true;
    emit('saved');
    emit('opened', value);
  });
}
async function saveToLibrary() {
  if (!selected.value || !canPreviewOnlineBlueprint(selected.value)) return;
  await request<SavedBlueprint>('download_online_blueprint', { id: selected.value.id, version: selected.value.version }, value => {
    saved.value = true;
    emit('saved', value);
  });
}
watch([providerId, () => selected.value?.id, () => selected.value?.version, () => selected.value?.format], loadViewerChoices, { flush: 'sync' });
watch([providerId, gameId, query, format, mod], () => { filterRevision++; void cancel(); cursor.value = null; searched.value = false; searchScope.value = null; items.value = []; selected.value = null; }, { flush: 'sync' });
onMounted(async () => {
  await availabilityCheck();
  try {
    const stop = await listen<BlueprintProviderProgress>('blueprint-provider-progress', event => { if (!disposed && event.payload.request_id === current) progress.value = event.payload; });
    if (disposed) stop(); else unlisten = stop;
  } catch { /* The request still reports bounded progress through its busy state. */ }
});
onUnmounted(() => { disposed = true; ++viewerRevision; unlisten?.(); void cancel(); });
</script>
<template>
  <section class="online-blueprints">
    <p
      v-if="!availability && !error"
      role="status"
    >
      {{ t('blueprintOnline.checking') }}
    </p>
    <div
      v-if="availability && !availability.available"
      class="online-unavailable"
    >
      <p>{{ t('blueprintOnline.unavailable') }}</p>
      <ul
        v-if="availability.interface?.candidates.length"
        class="provider-candidates"
        :aria-label="t('blueprintOnline.subscribedProviders')"
      >
        <li
          v-for="candidate in availability.interface.candidates"
          :key="`${candidate.source_id}:${candidate.plugin_id}`"
        >
          <button
            class="secondary-button"
            @click="emit('extensions', { sourceId: candidate.source_id, pluginId: candidate.plugin_id })"
          >
            {{ candidate.name }} <span>{{ candidate.version }}</span>
          </button>
        </li>
      </ul>
      <button
        v-else
        class="secondary-button"
        @click="emit('extensions')"
      >
        {{ t('blueprintOnline.managePlugins') }}
      </button>
      <button
        class="text-button"
        @click="availabilityCheck"
      >
        {{ t('blueprintOnline.retry') }}
      </button>
    </div>
    <template v-if="availability?.available">
      <form
        class="online-search"
        @submit.prevent="search()"
      >
        <label v-if="availability.providers.length > 1">{{ t('blueprintOnline.provider') }}<select v-model="providerId"><option
          value=""
          disabled
        >{{ t('blueprintOnline.chooseProvider') }}</option><option
          v-for="provider in availability.providers"
          :key="provider.id"
          :value="provider.id"
        >{{ provider.name }}</option></select></label>
        <label>{{ t('blueprintOnline.game') }}<select v-model="gameId"><option
          disabled
          value=""
        >{{ t('blueprintOnline.chooseGame') }}</option><option
          v-for="item in installedGames"
          :key="item.id"
          :value="item.id"
        >{{ item.name }} · {{ item.version }}</option></select></label>
        <label>{{ t('blueprintOnline.format') }}<select v-model="format"><option value="">{{ t('blueprintOnline.allFormats') }}</option><option value="schem">.schem</option><option value="litematic">.litematic</option></select></label>
        <label>{{ t('blueprintOnline.query') }}<input
          v-model="query"
          type="search"
          maxlength="200"
        ></label>
        <label>{{ t('blueprintOnline.mod') }}<input
          v-model="mod"
          type="text"
          maxlength="64"
          placeholder="create"
        ></label>
        <button
          class="primary-button"
          type="submit"
          :disabled="busy || !game || !selectedProvider"
        >
          <AppIcon name="search" />{{ t('blueprintOnline.search') }}
        </button>
      </form>
      <p
        v-if="searchScope === 'all_versions'"
        class="setting-hint online-search-scope"
      >
        {{ t('blueprintOnline.allVersions') }}
      </p>
      <p
        v-if="!installedGames.length"
        class="setting-hint"
      >
        {{ t('blueprintOnline.installGame') }}
      </p>
      <div
        v-if="busy"
        class="online-progress"
        role="status"
      >
        <AppIcon
          name="loader-circle"
          class="ui-busy-icon"
        />{{ progress ? t(`blueprintOnline.${progress.phase}`) : t('blueprintOnline.working') }}
        <progress
          v-if="progress && progress.total"
          :value="progress.completed"
          :max="progress.total"
        />
        <button
          class="text-button"
          @click="cancel"
        >
          {{ t('blueprintOnline.cancel') }}
        </button>
      </div>
      <p
        v-if="searched && !items.length && !busy"
        class="setting-hint"
      >
        {{ t('blueprintOnline.empty') }}
      </p>
      <ul class="online-results">
        <li
          v-for="item in items"
          :key="`${item.id}:${item.version}`"
        >
          <button
            class="online-result-card"
            :aria-label="item.title"
            :disabled="busy"
            @click="details(item)"
          >
            <span
              class="blueprint-cover"
              aria-hidden="true"
            ><AppIcon name="puzzle" /></span>
            <span class="online-result-copy">
              <span class="online-result-title">{{ item.title }}</span>
              <span class="online-result-meta">{{ item.author }} · {{ item.format }}<template v-if="item.java_release"> · {{ item.java_release }}</template></span>
            </span>
          </button>
        </li>
      </ul>
      <button
        v-if="cursor"
        class="secondary-button"
        :disabled="busy"
        @click="search(true)"
      >
        {{ t('blueprintOnline.more') }}
      </button>
      <section
        v-if="selected"
        class="online-detail"
      >
        <h2
          ref="detailHeading"
          tabindex="-1"
        >
          {{ selected.title }}
        </h2>
        <p class="online-detail-meta">
          {{ selectedProvider?.name }} · {{ selected.author }} · {{ selected.format }} · v{{ selected.version }}
        </p>
        <p
          v-if="selected.description"
          class="online-description"
        >
          {{ selected.description }}
        </p>
        <p
          v-if="selected.namespaces.length"
          class="setting-hint"
        >
          {{ selected.namespaces.join(', ') }}
        </p>
        <p class="setting-hint">
          {{ t(selected.attribution === 'original' ? 'blueprintOnline.original' : selected.attribution === 'repost' ? 'blueprintOnline.repost' : 'blueprintOnline.unknownAttribution') }} · {{ selected.license ?? t('blueprintOnline.unknownLicense') }}
        </p>
        <p
          v-if="viewerLoading"
          role="status"
        >
          {{ t('blueprintOnline.checkingViewers') }}
        </p>
        <label
          v-else-if="viewerChoices && viewerChoices.viewers.length > 1"
          class="viewer-picker"
        >
          {{ t('blueprintOnline.viewer') }}
          <select
            v-model="viewerId"
            :disabled="busy"
          >
            <option
              disabled
              value=""
            >{{ t('blueprintOnline.chooseViewer') }}</option>
            <option
              v-for="viewer in viewerChoices.viewers"
              :key="viewer.id"
              :value="viewer.id"
            >{{ viewer.name }}</option>
          </select>
        </label>
        <template v-else-if="viewerChoices && !viewerChoices.viewers.length">
          <p>{{ viewerChoices.disabled_viewers?.length ? t('blueprintOnline.disabledViewers', { names: viewerChoices.disabled_viewers.map(viewer => viewer.name).join(', ') }) : t('blueprintOnline.viewerUnavailable') }}</p>
          <button
            class="secondary-button"
            @click="emit('extensions')"
          >
            {{ t('blueprintOnline.managePlugins') }}
          </button>
        </template>
        <p
          v-if="viewerError"
          role="alert"
        >
          {{ viewerError }} <button
            class="text-button"
            @click="loadViewerChoices"
          >
            {{ t('blueprintOnline.retry') }}
          </button>
        </p>
        <p
          v-if="!canPreviewOnlineBlueprint(selected)"
          class="setting-hint"
        >
          {{ t('blueprintOnline.unsupportedFormat') }}
        </p>
        <div class="online-actions">
          <button
            class="primary-button"
            :disabled="busy || saved || !canPreviewOnlineBlueprint(selected)"
            @click="saveToLibrary"
          >
            <AppIcon name="download" />{{ saved ? t('blueprintOnline.saved') : t('blueprintOnline.save') }}
          </button>
          <button
            v-if="saved"
            class="text-button"
            @click="emit('library')"
          >
            {{ t('blueprintOnline.viewLibrary') }}
          </button>
          <button
            class="secondary-button"
            :disabled="busy || viewerLoading || !selectedViewer || !canPreviewOnlineBlueprint(selected)"
            @click="preview"
          >
            {{ t('blueprintOnline.preview') }}
          </button>
        </div>
      </section>
    </template>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
  </section>
</template>
<style scoped>
.viewer-picker{display:flex;align-items:center;gap:8px;font-size:12px}.viewer-picker select{padding:8px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit}.provider-candidates{display:flex;flex-wrap:wrap;gap:8px;list-style:none;padding:0;margin:0}.provider-candidates span{color:var(--muted);font-size:12px;margin-left:8px}.online-blueprints{margin:18px 0 26px}.online-search{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));align-items:end;gap:10px;padding:12px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}.online-search label{display:flex;flex-direction:column;gap:6px;font-size:12px;color:var(--muted)}.online-search input,.online-search select{width:100%;min-width:0;background:var(--surface,var(--panel));color:var(--ink);border:1px solid var(--line);border-radius:8px;padding:9px;font:inherit}.online-search>.primary-button{justify-self:start;min-height:36px;gap:7px}.online-search-scope{margin-top:12px}.online-progress{display:flex;align-items:center;gap:10px;margin-top:16px;font-size:12px}.online-progress progress{max-width:160px;min-width:0}.online-results{list-style:none;padding:0;margin:14px 0;display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,260px),1fr));gap:12px}.online-results li{min-width:0;display:grid;grid-template-columns:54px minmax(0,1fr);align-items:center;gap:10px;padding:10px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}.blueprint-cover{width:54px;height:54px;display:grid;place-items:center;border-radius:9px;background:linear-gradient(135deg,color-mix(in srgb,var(--accent) 16%,var(--panel)),var(--panel));color:var(--accent)}.blueprint-cover :deep(svg){width:23px;height:23px}.online-result-copy{min-width:0;display:flex;flex-direction:column;gap:5px}.online-result-copy>.text-button{padding:0;text-align:left;overflow-wrap:anywhere;font-weight:600}.online-result-copy>span{font-size:12px;color:var(--muted);overflow-wrap:anywhere}.result-action{grid-column:2;justify-self:start;padding:5px 9px;font-size:12px}.online-detail{display:flex;flex-direction:column;align-items:flex-start;gap:12px;margin-top:22px;padding-top:20px;border-top:1px solid var(--line);min-width:0}.online-detail h2{font-size:17px}.online-detail p{font-size:12px;overflow-wrap:anywhere}.online-description{white-space:pre-wrap;line-height:1.8}.online-detail>.primary-button,.online-detail>.secondary-button{gap:8px;min-height:36px}.online-unavailable{display:flex;gap:12px;flex-wrap:wrap;align-items:center;font-size:12px}.online-unavailable p{flex-basis:100%}@media(max-width:480px){.online-progress{flex-wrap:wrap}}
</style>
<style scoped>
.online-result-card{appearance:none;grid-column:1/-1;display:grid;grid-template-columns:54px minmax(0,1fr);align-items:center;gap:10px;width:100%;min-width:0;padding:4px;border:0;border-radius:9px;background:transparent;color:var(--ink);font:inherit;text-align:left;cursor:pointer}
.online-result-card:hover{background:color-mix(in srgb,var(--accent) 7%,var(--panel))}
.online-result-card:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.online-result-card:disabled{cursor:default;opacity:.65}
.online-result-title{font-size:14px;font-weight:600;color:var(--ink);overflow-wrap:anywhere}
.online-result-meta,.online-detail-meta{font-size:12px;color:var(--muted);overflow-wrap:anywhere}
.online-actions{display:flex;flex-wrap:wrap;align-items:center;gap:8px;width:100%}
.online-actions button{min-height:36px}
.online-search input,.online-search select{font-size:14px}
.online-detail .online-description{font-size:14px;line-height:1.65}
</style>

