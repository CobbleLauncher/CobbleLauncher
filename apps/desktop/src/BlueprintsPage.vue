<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import SelectField from './ui/SelectField.vue';
import SwitchField from './ui/SwitchField.vue';
import SearchField from './ui/SearchField.vue';
import { t } from './i18n';
import type { ToolLocation } from './navigation';
import type { BlueprintViewerChoices, ProviderCandidateSelection } from './online-blueprints';
import BlueprintLibrary from './BlueprintLibrary.vue';
import MaterialExportButton from './MaterialExportButton.vue';
import MaterialPlannerButton from './MaterialPlannerButton.vue';
import OnlineBlueprints from './OnlineBlueprints.vue';
import type { Game } from './games';
import type { ResolvedCommand } from './extensions/contributions';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { incomingBlueprint, pendingBlueprint, type PendingBlueprintRequest, type OpenedBlueprint as Opened, type SavedBlueprint, type SceneBlock } from './blueprints';
import { invoke } from '@tauri-apps/api/core';
import { createBlueprintRenderer } from './blueprint-viewer/renderer';
import type {ViewerTextures} from './blueprint-viewer/textures';
import { usePageWorkspace } from './use-page-workspace';
const props = withDefaults(defineProps<{ view?: ToolLocation['blueprintView']; games?: Game[]; initialGameId?: string; initialBlueprintId?: string; selectedCommand?: ResolvedCommand }>(), { view: undefined, games: () => [], initialGameId: '', initialBlueprintId: '', selectedCommand: undefined });
const workspace = usePageWorkspace('blueprints', props.initialGameId, () => ({ locatedBlueprintId: null as string | null }));
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; extensions: [candidate?: ProviderCandidateSelection]; command: [value: ResolvedCommand]; navigate: [value: { view: NonNullable<ToolLocation['blueprintView']>; title?: string }] }>();
const online = ref(false);
const localChoices = ref<BlueprintViewerChoices | null>(null);
const localViewerId = ref('');
const localViewersLoading = ref(false);
const pendingLocalOpen = ref<{ id?: string; format?: string; queueId?: string; name?: string } | null>(null);
const localViewers = computed(() => (localChoices.value?.viewers ?? []).filter(viewer => !pendingLocalOpen.value?.format || viewer.file_extensions.includes(pendingLocalOpen.value.format)));
const disabledLocalViewers = computed(() => (localChoices.value?.disabled_viewers ?? []).filter(viewer => !pendingLocalOpen.value?.format || viewer.file_extensions.includes(pendingLocalOpen.value.format)));
const library = ref<SavedBlueprint[]>([]); const opened = ref<Opened | null>(null); const busy = ref(false); const error = ref('');
const saved = ref(false); const canvas = ref<HTMLCanvasElement | null>(null); const loaded = ref(0); const materialSearch = ref('');
const savedBlueprintId = ref<string | null>(null);
const locatedBlueprintId = ref<string | null>(props.initialBlueprintId || workspace.locatedBlueprintId);
watch(() => props.initialBlueprintId, id => { if (id) { locatedBlueprintId.value = id; void loadLibrary(); } });
watch(locatedBlueprintId, value => { workspace.locatedBlueprintId = value; });
async function saveThumbnail(id: string) {
  try {
    const png = activeRenderer?.snapshot?.();
    if (!png) return;
    await invoke('save_blueprint_thumbnail', { id, png });
    await loadLibrary();
  } catch { /* Optional thumbnail failure must not prevent opening a blueprint. */ }
}
async function downloaded(item?: SavedBlueprint) {
  if (item) locatedBlueprintId.value = null;
  await loadLibrary();
  if (!item) return;
  locatedBlueprintId.value = item.id;
  selectTab('library');
}
const textureGames=computed(()=>props.games.filter(game=>game.status==='installed'));
const textureGameId=ref(props.initialGameId||textureGames.value[0]?.id||'');
const texturesEnabled=ref(true);const textureStatus=ref('');let textureRequest=0;
let activeRenderer:Awaited<ReturnType<typeof createBlueprintRenderer>>;
watch(textureGames,games=>{if(!games.some(game=>game.id===textureGameId.value))textureGameId.value=games[0]?.id??'';},{immediate:true});
async function refreshTextures(){
  const request=++textureRequest,viewer=activeRenderer,value=opened.value;
  textureStatus.value='';if(!viewer||!value)return;
  if(!texturesEnabled.value||!textureGameId.value){await viewer.setTextures(null);textureStatus.value=t('blueprints.colorsFallback');return;}
  await viewer.setTextures(null);
  if(request!==textureRequest)return;
  textureStatus.value=t('blueprints.texturesLoading');
  try{const textures=await invoke<ViewerTextures>('load_viewer_textures',{resourceId:value.resourceId,gameId:textureGameId.value});
    if(request!==textureRequest||graphics.signal.aborted||viewer!==activeRenderer)return;
    const applied=await viewer.setTextures(textures);
    if(request!==textureRequest||viewer!==activeRenderer)return;
    textureStatus.value=applied?t('blueprints.texturesResolved',{count:textures.resolvedBlocks,total:value.scene.palette.length}):t('blueprints.texturesNone');
    if(applied && savedBlueprintId.value && loaded.value===value.scene.chunk_count)void saveThumbnail(savedBlueprintId.value);
  }catch{if(request===textureRequest)textureStatus.value=t('blueprints.texturesUnavailable');}
}
watch([texturesEnabled,textureGameId],()=>void refreshTextures());
const materialSort = ref<'count' | 'name'>('count');
const copiedMaterials = ref(false);
const materials = computed(() => (opened.value?.scene.materials ?? []).filter(item => item.state.toLowerCase().includes(materialSearch.value.trim().toLowerCase())).slice().sort((a, b) => materialSort.value === 'count' ? b.count - a.count || a.state.localeCompare(b.state) : a.state.localeCompare(b.state)));
watch([materialSearch, materialSort, opened], () => { copiedMaterials.value = false; });
watch(() => props.initialGameId, id => { if (textureGames.value.some(game => game.id === id)) textureGameId.value = id; });
async function copyMaterials() {
  try { await writeText(materials.value.map(item => `${item.count}\t${item.state}`).join('\n')); copiedMaterials.value = true; }
  catch (cause) { error.value = String(cause); }
}
let generation = 0; let disposed = false; let graphics = new AbortController(); let opening = 0; let latestOpen = 0;
async function close() {
  textureRequest++;activeRenderer=undefined;textureStatus.value='';
  pendingLocalOpen.value = null;
  ++opening;
  ++generation; graphics.abort(); graphics = new AbortController(); const active=opened.value; opened.value=null; loaded.value=0;
  if (active) await invoke('close_viewer', { resourceId: active.resourceId }).catch(() => {});
}
function selectTab(view: 'library' | 'online') {
  online.value = view === 'online';
  if (view === 'library') void loadLibrary();
  void close();
  emit('navigate', { view });
}
function focusTab(view: 'library' | 'online', event: KeyboardEvent) {
  selectTab(view);
  const button = event.currentTarget as HTMLElement;
  button.parentElement?.querySelector<HTMLButtonElement>(`#blueprint-${view}-tab`)?.focus();
}
function closePreview() {
  void close();
  emit('navigate', { view: online.value ? 'online' : 'library' });
}
watch(() => props.view, view => {
  if (view === 'library' || view === 'online') {
    online.value = view === 'online';
    void close();
  }
}, { immediate: true });
async function loadLibrary() { try { library.value=await invoke<SavedBlueprint[]>('list_blueprints'); } catch { error.value=t('blueprints.loadError'); } }
function libraryChanged(items: SavedBlueprint[]) {
  library.value = items;
  if (!savedBlueprintId.value) return;
  const active = items.find(item => item.id === savedBlueprintId.value);
  if (active && opened.value) opened.value.name = active.name;
  if (!active) { savedBlueprintId.value = null; saved.value = false; }
}
async function display(value: Opened) {
  const closing = close(); const current = generation; const signal = graphics.signal;
  await closing;
  if (disposed || current !== generation) { await invoke('close_viewer', { resourceId: value.resourceId }); return; }
  opened.value=value; emit('navigate', { view: 'viewer', title: value.name }); saved.value=false; savedBlueprintId.value=null; materialSearch.value='';
  await nextTick();
  if (signal.aborted || !canvas.value) return;
  const viewer = await createBlueprintRenderer(canvas.value, value, signal);
  if (!viewer || signal.aborted) return;
  activeRenderer=viewer;
  for (let index = 0; index < value.scene.chunk_count; index++) {
    if (signal.aborted) return;
    let chunk: { blocks: SceneBlock[] };
    try { chunk = await invoke('read_viewer_chunk', { resourceId: value.resourceId, index }); }
    catch (cause) { if (signal.aborted) return; throw cause; }
    if (signal.aborted || !await viewer.addChunk(chunk.blocks)) return;
    loaded.value = index + 1;
    if(index===0)void refreshTextures();
  }
  return true;
}
async function open(id?: string, pluginId?: string) {
  const current = ++opening; latestOpen = current;
  let owner = current;
  busy.value=true; error.value='';
  try {
    const value=await invoke<Opened | null>(id?'open_blueprint':'open_schematic', id?{id, ...(pluginId ? { pluginId } : {})}:(pluginId ? { pluginId } : {}));
    if (disposed || current !== opening) { if (value) await invoke('close_viewer', { resourceId: value.resourceId }).catch(() => {}); return; }
    if(value) {
      const displayed = display(value); owner = opening;
      if(await displayed) { savedBlueprintId.value=id ?? null; saved.value=Boolean(id); if (id) void saveThumbnail(id); }
    }
  }
  catch(cause){if (!disposed && owner === opening && current === latestOpen) error.value=String(cause);}
  finally{if (current === latestOpen) busy.value=false;}
}
async function loadLocalViewers() {
  localViewersLoading.value = true;
  try {
    const choices = await invoke<BlueprintViewerChoices>('get_blueprint_viewer_choices', {});
    if (disposed) return;
    localChoices.value = choices;
    localViewerId.value = choices.viewer_id ?? '';
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { if (!disposed) localViewersLoading.value = false; }
}
async function requestLocalOpen(id?: string) {
  if (busy.value || localViewersLoading.value) return;
  pendingLocalOpen.value = { id, format: id ? library.value.find(item => item.id === id)?.format : undefined };
  const viewer = localViewers.value.find(viewer => viewer.id === localViewerId.value) ?? (localViewers.value.length === 1 ? localViewers.value[0] : undefined);
  if (viewer) {
    pendingLocalOpen.value = null;
    await open(id, viewer.id);
  } else {
    localViewerId.value = '';
    await nextTick(); document.getElementById('local-blueprint-viewer')?.focus();
  }
}
function chooseLocalViewer() {
  const pending = pendingLocalOpen.value;
  if (!pending || !localViewers.value.some(viewer => viewer.id === localViewerId.value)) return;
  if (pending.queueId) void openPendingBlueprint(pending, localViewerId.value);
  else { pendingLocalOpen.value = null; void open(pending.id, localViewerId.value); }
}
function selectLocalViewer(value: string) { localViewerId.value = value; chooseLocalViewer(); }
async function openPendingBlueprint(pending: { queueId?: string }, pluginId: string) {
  if (!pending.queueId || busy.value) return;
  const queueId = pending.queueId;
  const current = ++opening; latestOpen = current;
  let owner = current;
  busy.value = true; error.value = '';
  try {
    const value = await invoke<Opened | null>('take_pending_schematic', { pluginId, expectedQueueId: queueId });
    if (disposed || current !== opening) { if (value) await invoke('close_viewer', { resourceId: value.resourceId }).catch(() => {}); return; }
    if (value) {
      if (pendingBlueprint.value?.queueId === queueId) pendingBlueprint.value = null;
      pendingLocalOpen.value = null;
      const displayed = display(value); owner = opening;
      await displayed;
    }
  } catch (cause) { if (!disposed && current === latestOpen && owner === opening) error.value = String(cause); }
  finally { if (current === latestOpen) busy.value = false; }
}
async function resumePendingBlueprint() {
  const pending = pendingLocalOpen.value;
  if (!pending?.queueId || !localChoices.value || localViewersLoading.value || busy.value) return;
  if (localViewers.value.length === 1) await openPendingBlueprint(pending, localViewers.value[0]!.id);
  else { await nextTick(); document.getElementById('local-blueprint-viewer')?.focus(); }
}
function receivePendingBlueprint(request: PendingBlueprintRequest) {
  latestOpen = ++opening;
  busy.value = false;
  pendingLocalOpen.value = { queueId: request.queueId, name: request.name, format: request.format };
  localViewerId.value = '';
  void resumePendingBlueprint();
}
watch(pendingBlueprint, request => { if (request) receivePendingBlueprint(request); }, { immediate: true, flush: 'post' });
watch(localViewersLoading, loading => { if (!loading) void resumePendingBlueprint(); });
async function save() {
  if(!opened.value)return;
  try {
    const item=await invoke<SavedBlueprint>('save_blueprint',{resourceId:opened.value.resourceId});
    savedBlueprintId.value=item?.id ?? null; saved.value=true; await loadLibrary();
    if (item?.id) void saveThumbnail(item.id);
  } catch(cause){error.value=String(cause);}
}
watch(incomingBlueprint, value => { if(value){incomingBlueprint.value=null; void display(value).catch(cause=>{error.value=String(cause);});} }, { immediate:true, flush:'post' });
watch(() => props.selectedCommand, command => { if (command?.workflow === 'viewer' && command.blueprintId) void open(command.blueprintId, command.pluginId); else ++opening; }, { immediate: true });
onMounted(() => { void loadLibrary(); void loadLocalViewers(); });
onUnmounted(() => { disposed=true; void close(); });
</script>
<template>
  <div class="blueprints-page">
    <div class="blueprint-toolbar">
      <div
        class="blueprint-tabs"
        role="tablist"
        :aria-label="t('tools.blueprintsName')"
      >
        <button
          id="blueprint-library-tab"
          role="tab"
          :aria-selected="!online"
          aria-controls="blueprint-library-panel"
          :tabindex="online ? -1 : 0"
          @click="selectTab('library')"
          @keydown.right.prevent="focusTab('online', $event)"
          @keydown.left.prevent="focusTab('online', $event)"
        >
          {{ t('blueprints.libraryTab') }}
        </button>
        <button
          id="blueprint-online-tab"
          role="tab"
          :aria-selected="online"
          aria-controls="blueprint-online-panel"
          :tabindex="online ? 0 : -1"
          @click="selectTab('online')"
          @keydown.left.prevent="focusTab('library', $event)"
          @keydown.right.prevent="focusTab('library', $event)"
        >
          {{ t('blueprintOnline.online') }}
        </button>
      </div>
      <button
        v-if="library.length || opened"
        class="secondary-button"
        :disabled="busy || localViewersLoading || !localChoices?.viewers?.length"
        @click="requestLocalOpen()"
      >
        {{ t('blueprints.open') }}
      </button>
      <button
        v-if="opened && !view"
        class="text-button"
        @click="closePreview"
      >
        {{ t('blueprints.close') }}
      </button>
    </div>
    <div
      v-if="!online && !opened"
      class="local-viewer-choice"
    >
      <p
        v-if="localViewersLoading"
        role="status"
      >
        {{ t('blueprintOnline.checkingViewers') }}
      </p>
      <div
        v-else-if="localViewers.length > 1"
        class="local-viewer-picker"
      >
        {{ t('blueprintOnline.viewer') }}<span v-if="pendingLocalOpen?.queueId"> · {{ pendingLocalOpen.name }}</span>
        <SelectField
          id="local-blueprint-viewer"
          :model-value="localViewerId"
          :label="t('blueprintOnline.viewer')"
          :placeholder="t('blueprintOnline.chooseViewer')"
          :options="localViewers.map(viewer => ({value: viewer.id, label: `${viewer.name} · ${viewer.file_extensions.map(extension => `.${extension}`).join(', ')}`}))"
          :disabled="busy"
          @update:model-value="selectLocalViewer"
        />
      </div>
      <template v-else-if="localChoices && !localViewers.length">
        <p>{{ disabledLocalViewers.length ? t('blueprintOnline.disabledViewers', { names: disabledLocalViewers.map(viewer => viewer.name).join(', ') }) : t('blueprintOnline.viewerUnavailable') }}</p>
        <button
          class="secondary-button"
          @click="emit('extensions')"
        >
          {{ t('blueprintOnline.managePlugins') }}
        </button>
      </template>
    </div>
    <OnlineBlueprints
      v-if="online"
      id="blueprint-online-panel"
      role="tabpanel"
      aria-labelledby="blueprint-online-tab"
      :games="games"
      :initial-game-id="initialGameId"
      @opened="value => { online = false; void loadLibrary(); void display(value).catch(cause => { error = String(cause); }); }"
      @saved="downloaded"
      @library="selectTab('library')"
      @busy="busy = $event"
      @extensions="emit('extensions', $event)"
    />
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
      <button
        v-if="pendingLocalOpen?.queueId"
        class="text-button"
        :disabled="busy"
        @click="resumePendingBlueprint"
      >
        {{ t('blueprintOnline.retry') }}
      </button>
    </p>
    <section
      v-if="opened"
      class="blueprint-viewer"
    >
      <div class="blueprint-title">
        <h2>{{ opened.name }}</h2><span class="setting-hint">{{ opened.scene.width }} × {{ opened.scene.height }} × {{ opened.scene.length }}</span><button
          class="text-button"
          :disabled="saved || busy"
          @click="save"
        >
          {{ saved ? t('blueprints.saved') : t('blueprints.save') }}
        </button>
      </div>
      <div
        class="texture-controls"
      >
        <label
          v-if="textureGames.length"
          for="blueprint-textures"
        >{{ t('blueprints.localTextures') }}</label>
        <SwitchField
          v-if="textureGames.length"
          id="blueprint-textures"
          v-model="texturesEnabled"
          :label="t('blueprints.localTextures')"
        />
        <SelectField
          v-if="textureGames.length && (textureGames.length > 1 || !initialGameId)"
          id="blueprint-texture-game"
          v-model="textureGameId"
          :options="textureGames.map(game => ({ value: game.id, label: game.name }))"
          :label="t('blueprints.textureGame')"
          :disabled="!texturesEnabled"
        />
        <span
          v-if="textureStatus"
          class="setting-hint"
          role="status"
        >{{ textureStatus }}</span>
      </div>
      <canvas
        ref="canvas"
        :aria-label="t('blueprints.preview')"
      />
      <p
        v-if="loaded < opened.scene.chunk_count"
        class="setting-hint"
        role="status"
      >
        {{ t('blueprints.loading', { loaded, total: opened.scene.chunk_count }) }}
      </p>
      <div class="material-actions">
        <SearchField
          id="material-search"
          v-model="materialSearch"
          class="material-search-field"
          :label="t('blueprints.searchMaterials')"
          :placeholder="t('blueprints.searchMaterials')"
        />
        <SelectField
          id="material-sort"
          v-model="materialSort"
          :label="t('blueprints.sortMaterials')"
          :options="[{ value: 'count', label: t('blueprints.countDescending') }, { value: 'name', label: t('blueprints.nameAscending') }]"
        />
        <button
          class="text-button"
          :disabled="!materials.length"
          @click="copyMaterials"
        >
          {{ t(copiedMaterials ? 'blueprints.materialsCopied' : 'blueprints.copyMaterials') }}
        </button>
        <MaterialPlannerButton
          :key="`planner-${opened.resourceId}`"
          :resource-id="opened.resourceId"
          :games="games"
          :initial-game-id="initialGameId"
          @error="error = $event"
        />
        <MaterialExportButton
          :key="opened.resourceId"
          :resource-id="opened.resourceId"
          @error="error = $event"
        />
      </div>
      <p
        v-if="!materials.length"
        class="setting-hint"
      >
        {{ t('blueprints.noMaterials') }}
      </p>
      <div class="materials">
        <table>
          <thead><tr><th>{{ t('blueprints.material') }}</th><th>{{ t('blueprints.count') }}</th></tr></thead><tbody>
            <tr
              v-for="item in materials"
              :key="item.state"
            >
              <td>{{ item.state }}</td><td>{{ item.count }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
    <section
      v-show="!online && !opened"
      id="blueprint-library-panel"
      role="tabpanel"
      aria-labelledby="blueprint-library-tab"
    >
      <div
        v-if="!library.length && !opened"
        class="blueprint-empty"
      >
        <p>{{ t('blueprints.empty') }}</p>
        <button
          class="primary-button"
          :disabled="busy || localViewersLoading || !localChoices?.viewers?.length"
          @click="requestLocalOpen()"
        >
          {{ t('blueprints.open') }}
        </button>
      </div>
      <BlueprintLibrary
        :items="library"
        :selected-id="locatedBlueprintId"
        :initial-game-id="initialGameId"
        :busy="busy || localViewersLoading"
        @open="requestLocalOpen"
        @changed="libraryChanged"
        @command="emit('command', $event)"
        @market="emit('market', $event)"
      />
    </section>
  </div>
</template>
<style scoped>
.blueprints-page{width:100%;min-width:0}.local-viewer-choice:has(.local-viewer-picker),.local-viewer-choice:has(p){margin-bottom:var(--ui-space-4)}.local-viewer-picker{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);font-size:var(--ui-text-label);line-height:var(--ui-leading-label)}.local-viewer-picker :deep(.select-field-trigger){width:min(360px,100%)}.blueprint-toolbar{display:flex;gap:var(--ui-space-3);align-items:center;flex-wrap:wrap;margin-bottom:var(--ui-space-4)}.blueprint-toolbar>button{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}.blueprint-tabs{display:flex;gap:var(--ui-space-5);margin-right:auto;border-bottom:1px solid var(--ui-border)}.blueprint-tabs button{min-height:var(--ui-control-height);padding:0 0 var(--ui-space-2);font:inherit;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);border:0;background:transparent;color:var(--ui-text-muted);border-bottom:2px solid transparent}.blueprint-tabs button[aria-selected=true]{color:var(--ui-text);border-bottom-color:var(--ui-focus);font-weight:600}.blueprint-empty{padding:var(--ui-space-6) 0;display:flex;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap}.blueprint-empty p{flex-basis:100%;margin:0 0 var(--ui-space-1);color:var(--ui-text-muted);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.blueprint-title{display:flex;gap:var(--ui-space-3);align-items:center;flex-wrap:wrap;margin-bottom:var(--ui-space-3)}.blueprint-title h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);margin:0}.blueprint-title>.setting-hint{margin:0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);font-variant-numeric:tabular-nums}.blueprint-title>button{margin-left:auto;min-height:var(--ui-control-height)}.blueprint-viewer canvas{width:100%;height:clamp(260px,44dvh,480px);display:block;touch-action:none;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-2)}.texture-controls{display:flex;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap;margin:var(--ui-space-3) 0;font-size:var(--ui-text-label)}.texture-controls label{display:flex;align-items:center;gap:var(--ui-space-2)}.texture-controls :deep(.select-field-trigger){width:auto;max-width:280px}.texture-controls>.setting-hint{margin:0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.material-actions{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-2);margin:var(--ui-space-4) 0}.material-actions :deep(.select-field-trigger){width:auto;min-width:110px}.material-search-field{flex:1 1 200px;min-width:160px}.material-actions>button{min-height:var(--ui-control-height);font-size:var(--ui-text-label)}.materials{max-height:320px;overflow:auto;margin-top:var(--ui-space-3)}table{width:100%;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);border-collapse:collapse}th,td{padding:var(--ui-space-2) var(--ui-space-3);text-align:left;border-bottom:1px solid var(--ui-border);overflow-wrap:anywhere}td:last-child,th:last-child{text-align:right;font-variant-numeric:tabular-nums}th{font-size:var(--ui-text-caption);font-weight:500;color:var(--ui-text-muted)}
</style>
