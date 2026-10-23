<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import SelectField from './ui/SelectField.vue';
import SwitchField from './ui/SwitchField.vue';
import { t } from './i18n';
import type { ToolLocation } from './navigation';
import type { BlueprintViewerChoices, ProviderCandidateSelection } from './online-blueprints';
import BlueprintLibrary from './BlueprintLibrary.vue';
import MaterialExportButton from './MaterialExportButton.vue';
import OnlineBlueprints from './OnlineBlueprints.vue';
import type { Game } from './games';
import type { ResolvedCommand } from './extensions/contributions';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { incomingBlueprint, pendingBlueprint, type PendingBlueprintRequest, type OpenedBlueprint as Opened, type SavedBlueprint, type SceneBlock } from './blueprints';
import { invoke } from '@tauri-apps/api/core';
import { createBlueprintRenderer } from './blueprint-viewer/renderer';
import type {ViewerTextures} from './blueprint-viewer/textures';
const props = withDefaults(defineProps<{ view?: ToolLocation['blueprintView']; games?: Game[]; initialGameId?: string; selectedCommand?: ResolvedCommand }>(), { view: undefined, games: () => [], initialGameId: '', selectedCommand: undefined });
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; extensions: [candidate?: ProviderCandidateSelection]; command: [value: ResolvedCommand]; navigate: [value: { view: NonNullable<ToolLocation['blueprintView']>; title?: string }] }>();
const online = ref(false);
const localChoices = ref<BlueprintViewerChoices | null>(null);
const localViewerId = ref('');
const localViewersLoading = ref(false);
const localViewerSelect = ref<HTMLSelectElement | null>(null);
const pendingLocalOpen = ref<{ id?: string; format?: string; queueId?: string; name?: string } | null>(null);
const localViewers = computed(() => (localChoices.value?.viewers ?? []).filter(viewer => !pendingLocalOpen.value?.format || viewer.file_extensions.includes(pendingLocalOpen.value.format)));
const disabledLocalViewers = computed(() => (localChoices.value?.disabled_viewers ?? []).filter(viewer => !pendingLocalOpen.value?.format || viewer.file_extensions.includes(pendingLocalOpen.value.format)));
const library = ref<SavedBlueprint[]>([]); const opened = ref<Opened | null>(null); const busy = ref(false); const error = ref('');
const saved = ref(false); const canvas = ref<HTMLCanvasElement | null>(null); const loaded = ref(0); const materialSearch = ref('');
const savedBlueprintId = ref<string | null>(null);
const locatedBlueprintId = ref<string | null>(null);
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
    await nextTick(); localViewerSelect.value?.focus();
  }
}
function chooseLocalViewer() {
  const pending = pendingLocalOpen.value;
  if (!pending || !localViewers.value.some(viewer => viewer.id === localViewerId.value)) return;
  if (pending.queueId) void openPendingBlueprint(pending, localViewerId.value);
  else { pendingLocalOpen.value = null; void open(pending.id, localViewerId.value); }
}
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
  else { await nextTick(); localViewerSelect.value?.focus(); }
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
        v-if="opened"
        class="text-button"
        @click="closePreview"
      >
        {{ t('blueprints.close') }}
      </button>
    </div>
    <div
      v-if="!online"
      class="local-viewer-choice"
    >
      <p
        v-if="localViewersLoading"
        role="status"
      >
        {{ t('blueprintOnline.checkingViewers') }}
      </p>
      <label
        v-else-if="localViewers.length > 1"
        class="local-viewer-picker"
      >
        {{ t('blueprintOnline.viewer') }}<span v-if="pendingLocalOpen?.queueId"> · {{ pendingLocalOpen.name }}</span>
        <select
          ref="localViewerSelect"
          v-model="localViewerId"
          :disabled="busy"
          @change="chooseLocalViewer"
        >
          <option
            disabled
            value=""
          >{{ t('blueprintOnline.chooseViewer') }}</option>
          <option
            v-for="viewer in localViewers"
            :key="viewer.id"
            :value="viewer.id"
          >{{ viewer.name }} · {{ viewer.file_extensions.map(extension => `.${extension}`).join(', ') }}</option>
        </select>
      </label>
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
        <input
          id="material-search"
          v-model="materialSearch"
          class="search-input"
          type="search"
          :aria-label="t('blueprints.searchMaterials')"
          :placeholder="t('blueprints.searchMaterials')"
        >
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
      v-show="!online"
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
.local-viewer-choice:has(label),.local-viewer-choice:has(p){margin-bottom:16px}.local-viewer-picker{display:flex;align-items:center;gap:8px;font-size:12px}.local-viewer-picker select{padding:8px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit}.blueprints-page{width:100%}.blueprint-toolbar{display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-bottom:16px}.blueprint-tabs{display:flex;gap:4px;margin-right:auto;border-bottom:1px solid var(--line)}.blueprint-tabs button{padding:9px 12px;border:0;background:transparent;color:var(--muted);border-bottom:2px solid transparent}.blueprint-tabs button[aria-selected="true"]{color:var(--ink);border-bottom-color:var(--accent)}.blueprint-empty{padding:32px 0;display:flex;align-items:center;gap:12px;flex-wrap:wrap}.blueprint-empty p{flex-basis:100%;margin:0 0 4px;color:var(--muted)}.blueprint-title{display:flex;gap:12px;align-items:center;flex-wrap:wrap}.blueprint-title h2{font-size:15px;margin:0}.blueprint-viewer canvas{width:100%;height:320px;display:block;touch-action:none}.materials{max-height:280px;overflow:auto;margin-top:12px}table{width:100%;font-size:12px;border-collapse:collapse}th,td{padding:9px;text-align:left;border-bottom:1px solid var(--line);overflow-wrap:anywhere}td:last-child,th:last-child{text-align:right}
.texture-controls{display:flex;align-items:center;gap:12px;flex-wrap:wrap;margin:12px 0;font-size:12px}.texture-controls label{display:flex;align-items:center;gap:6px}.texture-controls select{padding:6px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;max-width:240px}
.material-actions{display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin:12px 0}.material-actions :deep(.select-field-trigger){width:auto;min-width:96px}.material-actions .search-input{width:200px;min-height:40px;padding:8px 12px;border:1px solid var(--ui-border-strong);border-radius:8px;background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:14px}.texture-controls :deep(.select-field-trigger){width:auto;max-width:240px}
</style>

