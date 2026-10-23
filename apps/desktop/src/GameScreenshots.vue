<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem, ContextMenuCheckboxItem } from 'reka-ui';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import ScreenshotSheetPanel from './ScreenshotSheetPanel.vue';
import {MAX_SHEET_IMAGES} from './screenshot-sheet';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import type { GameScreenshot, GameScreenshotImage, GameScreenshotPage } from './screenshots';

const props = withDefaults(defineProps<{ gameId: string;active?:boolean }>(),{active:true});
const emit=defineEmits<{busy:[value:boolean];extensions:[]}>();
const sheetSelection=ref<GameScreenshot[]>([]);
const sheetBusy=ref(false);
const sheetOpen=ref(false);
watch(()=>sheetSelection.value.length,length=>{if(!length)sheetOpen.value=false;},{flush:'sync'});
watch(sheetBusy,value=>emit('busy',value),{flush:'sync'});
function isChecked(id:string){return sheetSelection.value.some(item=>item.id===id);}
function toggleSelection(item:GameScreenshot){if(sheetBusy.value)return;if(isChecked(item.id))sheetSelection.value=sheetSelection.value.filter(value=>value.id!==item.id);else if(sheetSelection.value.length<MAX_SHEET_IMAGES)sheetSelection.value=[...sheetSelection.value,item];}
const page = ref<GameScreenshotPage>({ items: [], total: 0, next_offset: null });
const offset = ref(0);
const loading = ref(false);
const error = ref('');
const thumbnails = ref<Record<string, GameScreenshotImage>>({});
const failedThumbnails = ref<Record<string, boolean>>({});
const selected = ref<GameScreenshot | null>(null);
const preview = ref<GameScreenshotImage | null>(null);
const previewLoading = ref(false);
const previewError = ref('');
const folderBusy = ref(false);
const originalBusy = ref(false);
const browsing = ref(false);
const selectedIndex = computed(() => page.value.items.findIndex(item => item.id === selected.value?.id));
const previousImage = computed(() => selectedIndex.value > 0 || (selectedIndex.value === 0 && offset.value > 0));
const nextImage = computed(() => selectedIndex.value >= 0 && (selectedIndex.value < page.value.items.length - 1 || page.value.next_offset !== null));
const paginated = computed(() => offset.value > 0 || page.value.next_offset !== null);
const limit = 40;
let revision = 0;
let previewRevision = 0;
let disposed = false;

function closePreview() {
  ++previewRevision; browsing.value = false; selected.value = null; preview.value = null; previewError.value = ''; previewLoading.value = false;
}
async function loadThumbnails(items: GameScreenshot[], gameId: string, current: number) {
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(4, items.length) }, async () => {
    while (!disposed && current === revision) {
      const item = items[next++];
      if (!item) return;
      if (thumbnails.value[item.id]) continue;
      try {
        const image = await invoke<GameScreenshotImage>('get_game_screenshot', { id: gameId, screenshotId: item.id, thumbnail: true });
        if (!disposed && current === revision) thumbnails.value[item.id] = image;
      } catch { if (!disposed && current === revision) failedThumbnails.value[item.id] = true; }
    }
  }));
}
async function load(nextOffset = 0, preservePage = false) {
  if(sheetBusy.value)return;
  const current = ++revision;
  const gameId = props.gameId;
  const keepPage = preservePage && nextOffset === offset.value;
  closePreview(); loading.value = true; error.value = '';
  if (!keepPage) {
    thumbnails.value = {}; failedThumbnails.value = {};
    page.value = { items: [], total: 0, next_offset: null };
  }
  offset.value = nextOffset;
  try {
    const result = await invoke<GameScreenshotPage>('list_game_screenshots', { id: gameId, offset: nextOffset, limit });
    if (disposed || current !== revision) return;
    thumbnails.value = Object.fromEntries(result.items.filter(item => thumbnails.value[item.id]).map(item => [item.id, thumbnails.value[item.id]!]));
    failedThumbnails.value = {};
    page.value = result;
    void loadThumbnails(result.items, gameId, current);
  } catch { if (!disposed && current === revision) error.value = t('screenshots.listFailed'); }
  finally { if (!disposed && current === revision) loading.value = false; }
}
async function show(item: GameScreenshot) {
  const current = ++previewRevision;
  const gameId = props.gameId;
  selected.value = item; preview.value = null; previewError.value = ''; previewLoading.value = true;
  try {
    const image = await invoke<GameScreenshotImage>('get_game_screenshot', { id: gameId, screenshotId: item.id, thumbnail: false });
    if (!disposed && current === previewRevision) preview.value = image;
  } catch { if (!disposed && current === previewRevision) previewError.value = t('screenshots.previewFailed'); }
  finally { if (!disposed && current === previewRevision) previewLoading.value = false; }
}
async function browseImage(direction: -1 | 1) {
  if (previewLoading.value || browsing.value || !selected.value || !(direction < 0 ? previousImage.value : nextImage.value)) return;
  const neighbor = page.value.items[selectedIndex.value + direction];
  if (neighbor) { await show(neighbor); return; }
  const nextOffset = direction > 0 ? page.value.next_offset : Math.max(0, offset.value - limit);
  if (nextOffset === null) return;
  const current = revision;
  const selection = ++previewRevision;
  const gameId = props.gameId;
  browsing.value = true; previewError.value = '';
  try {
    const result = await invoke<GameScreenshotPage>('list_game_screenshots', { id: gameId, offset: nextOffset, limit });
    if (disposed || current !== revision || selection !== previewRevision) return;
    const thumbnailsRevision = ++revision;
    thumbnails.value = Object.fromEntries(result.items.filter(item => thumbnails.value[item.id]).map(item => [item.id, thumbnails.value[item.id]!]));
    failedThumbnails.value = {}; page.value = result; offset.value = nextOffset;
    void loadThumbnails(result.items, gameId, thumbnailsRevision);
    browsing.value = false;
    const target = direction > 0 ? result.items[0] : result.items.at(-1);
    if (target) await show(target); else closePreview();
  } catch { if (!disposed && current === revision && selection === previewRevision) previewError.value = t('screenshots.listFailed'); }
  finally { if (!disposed && selection === previewRevision) browsing.value = false; }
}
function previewKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || (event.target instanceof HTMLElement && event.target.closest('input,textarea,select,[contenteditable="true"]'))) return;
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
  event.preventDefault();
  void browseImage(event.key === 'ArrowLeft' ? -1 : 1);
}
async function openFolder() {
  if (folderBusy.value) return;
  folderBusy.value = true;
  const current = revision;
  try { await invoke('open_game_screenshots_folder', { id: props.gameId }); }
  catch { if (!disposed && current === revision) error.value = t('screenshots.openFailed'); }
  finally { folderBusy.value = false; }
}
async function openOriginal() {
  if (!selected.value || originalBusy.value) return;
  const current = previewRevision;
  originalBusy.value = true;
  try { await invoke('open_game_screenshot', { id: props.gameId, screenshotId: selected.value.id }); }
  catch { if (!disposed && current === previewRevision) previewError.value = t('screenshots.openFailed'); }
  finally { originalBusy.value = false; }
}
watch(() => props.gameId, () => {sheetSelection.value=[];sheetOpen.value=false;sheetBusy.value=false; void load(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++revision; ++previewRevision; });
</script>

<template>
  <section class="game-screenshots">
    <div class="screenshot-actions">
      <button
        class="secondary-button"
        :disabled="folderBusy"
        @click="openFolder"
      >
        <AppIcon name="folder" />{{ t('screenshots.openFolder') }}
      </button>
      <button
        class="text-button"
        :disabled="loading || sheetBusy"
        @click="load(offset, true)"
      >
        <AppIcon name="refresh-cw" />{{ t('screenshots.refresh') }}
      </button>
    </div>
    <div
      v-if="sheetSelection.length"
      class="screenshot-selection"
    >
      <div class="screenshot-selection-heading">
        <strong>{{ t('screenshotSheet.selected',{count:sheetSelection.length,max:MAX_SHEET_IMAGES}) }}</strong><button
          v-if="!sheetOpen"
          class="secondary-button"
          @click="sheetOpen=true"
        >
          {{ t('screenshotSheet.preview') }}
        </button><button
          class="text-button"
          :disabled="sheetBusy"
          @click="sheetSelection=[]"
        >
          {{ t('screenshotSheet.clear') }}
        </button>
      </div><ol class="screenshot-selected-items">
        <li
          v-for="(item,index) in sheetSelection"
          :key="item.id"
        >
          <span :title="item.name">{{ index+1 }}. {{ item.name }}</span><AppIconButton
            icon="x"
            :label="t('screenshotSheet.deselect',{name:item.name})"
            :disabled="sheetBusy"
            @click="toggleSelection(item)"
          />
        </li>
      </ol><ScreenshotSheetPanel
        v-if="sheetOpen"
        :key="gameId"
        :game-id="gameId"
        :screenshots="sheetSelection"
        :active="active"
        @busy="sheetBusy=$event"
        @extensions="emit('extensions')"
      />
    </div>
    <LoadingIndicator
      v-if="loading"
      :label="t('screenshots.loading')"
      :compact="page.items.length > 0"
    />
    <p
      v-else-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <p
      v-else-if="!page.items.length"
      class="screenshot-empty"
    >
      {{ t('screenshots.empty') }}
    </p>
    <ul class="screenshot-grid">
      <ContextMenuRoot
        v-for="item in page.items"
        :key="item.id"
      >
        <ContextMenuTrigger as-child>
          <li>
            <label class="screenshot-selection-toggle"><input
              type="checkbox"
              :checked="isChecked(item.id)"
              :aria-label="t('screenshotSheet.select',{name:item.name})"
              :disabled="sheetBusy || (!isChecked(item.id) && sheetSelection.length >= MAX_SHEET_IMAGES)"
              @change="toggleSelection(item)"
            ><span
              v-if="isChecked(item.id)"
              aria-hidden="true"
            >{{ sheetSelection.findIndex(value=>value.id===item.id)+1 }}</span></label>
            <button
              class="screenshot-card"
              :aria-label="t('screenshots.preview', { name: item.name })"
              @click="show(item)"
            >
              <span class="screenshot-thumbnail">
                <img
                  v-if="thumbnails[item.id]"
                  :src="thumbnails[item.id]!.data_url"
                  alt=""
                  :width="thumbnails[item.id]!.width"
                  :height="thumbnails[item.id]!.height"
                >
                <span
                  v-else-if="failedThumbnails[item.id]"
                  class="screenshot-thumbnail-error"
                ><AppIcon name="alert-circle" />{{ t('screenshots.imageError') }}</span>
                <AppIcon
                  v-else
                  name="loader-circle"
                  class="ui-busy-icon"
                />
              </span>
              <span
                class="screenshot-name"
                :title="item.name"
              >{{ item.name }}</span>
            </button>
          </li>
        </ContextMenuTrigger>
        <ContextMenuPortal>
          <ContextMenuContent
            class="profile-menu"
            :collision-padding="12"
          >
            <ContextMenuItem
              class="profile-menu-item"
              @select="show(item)"
            >
              {{ t('screenshots.preview', { name: item.name }) }}
            </ContextMenuItem>
            <ContextMenuCheckboxItem
              class="profile-menu-item"
              :checked="isChecked(item.id)"
              :disabled="sheetBusy || (!isChecked(item.id) && sheetSelection.length >= MAX_SHEET_IMAGES)"
              @select="toggleSelection(item)"
            >
              {{ t('screenshotSheet.select', { name: item.name }) }}
            </ContextMenuCheckboxItem>
          </ContextMenuContent>
        </ContextMenuPortal>
      </ContextMenuRoot>
    </ul>
    <div
      v-if="paginated && !loading && !error"
      class="screenshot-pages"
    >
      <button
        class="text-button"
        :disabled="offset === 0 || sheetBusy"
        @click="load(Math.max(0, offset - limit))"
      >
        {{ t('screenshots.previous') }}
      </button>
      <span>{{ t('screenshots.page', { first: page.items.length ? offset + 1 : 0, last: page.items.length ? offset + page.items.length : 0, total: page.total }) }}</span>
      <button
        class="text-button"
        :disabled="page.next_offset === null || sheetBusy"
        @click="load(page.next_offset ?? 0)"
      >
        {{ t('screenshots.next') }}
      </button>
    </div>
    <DialogRoot
      :open="selected !== null"
      @update:open="value => { if (!value) closePreview(); }"
    >
      <DialogPortal>
        <DialogOverlay class="screenshot-overlay" />
        <DialogContent
          class="screenshot-dialog"
          :aria-describedby="undefined"
          @keydown="previewKeydown"
        >
          <div class="screenshot-preview-heading">
            <DialogTitle>{{ selected?.name }}</DialogTitle>
            <AppIconButton
              icon="x"
              :label="t('screenshots.close')"
              @click="closePreview"
            />
          </div>
          <LoadingIndicator
            v-if="previewLoading || browsing"
            :label="t('screenshots.loadingImage')"
          />
          <p
            v-if="previewError"
            class="settings-error"
            role="alert"
          >
            {{ previewError }} <button
              class="text-button"
              :disabled="previewLoading"
              @click="selected && show(selected)"
            >
              {{ t('screenshots.retry') }}
            </button>
          </p>
          <img
            v-if="preview"
            class="screenshot-preview"
            :src="preview.data_url"
            :alt="selected?.name"
            :width="preview.width"
            :height="preview.height"
          >
          <div class="screenshot-preview-actions">
            <span v-if="preview">{{ preview.original_width }} × {{ preview.original_height }}</span>
            <div class="screenshot-browse">
              <AppIconButton
                icon="arrow-left"
                :label="t('screenshots.previousImage')"
                aria-keyshortcuts="ArrowLeft"
                :disabled="!previousImage || previewLoading || browsing"
                @click="browseImage(-1)"
              />
              <span
                v-if="selectedIndex >= 0"
                aria-live="polite"
                aria-atomic="true"
              >{{ t('screenshots.position', { current: offset + selectedIndex + 1, total: page.total }) }}</span>
              <AppIconButton
                class="screenshot-next"
                icon="arrow-left"
                :label="t('screenshots.nextImage')"
                aria-keyshortcuts="ArrowRight"
                :disabled="!nextImage || previewLoading || browsing"
                @click="browseImage(1)"
              />
            </div>
            <button
              class="secondary-button"
              :disabled="originalBusy"
              @click="openOriginal"
            >
              <AppIcon name="external-link" />{{ t('screenshots.openOriginal') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
  </section>
</template>
<style scoped>
.screenshot-browse{display:flex;align-items:center;gap:10px}.screenshot-next :deep(svg){transform:rotate(180deg)}.game-screenshots{width:100%;min-width:0}.screenshot-actions,.screenshot-pages,.screenshot-preview-actions{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.screenshot-actions{margin-bottom:18px}.screenshot-actions button,.screenshot-preview-actions button{display:inline-flex;align-items:center;gap:7px}.screenshot-empty{color:var(--muted);margin:28px 0}.screenshot-grid{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fill,minmax(min(220px,100%),1fr));gap:16px}.screenshot-grid li{min-width:0}.screenshot-card{display:block;padding:0;width:100%;border:1px solid var(--line);border-radius:10px;overflow:hidden;background:var(--panel);color:var(--ink);font:inherit;text-align:left;cursor:pointer}.screenshot-card:hover{border-color:var(--accent)}.screenshot-thumbnail{display:flex;align-items:center;justify-content:center;aspect-ratio:16/9;background:var(--hover);overflow:hidden}.screenshot-thumbnail img{width:100%;height:100%;object-fit:contain}.screenshot-thumbnail-error{display:flex;align-items:center;gap:6px;padding:12px;color:var(--muted);font-size:12px}.screenshot-name{display:block;padding:10px 12px;font-size:14px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.screenshot-pages{margin-top:20px;font-size:12px}.screenshot-pages span{color:var(--muted)}.screenshot-overlay{position:fixed;inset:0;background:#0009;z-index:100}.screenshot-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(1120px,calc(100vw - 32px));max-height:calc(100vh - 32px);overflow:auto;box-sizing:border-box;padding:18px;border:1px solid var(--line);border-radius:12px;background:var(--panel);color:var(--ink);z-index:101}.screenshot-preview-heading{display:flex;align-items:center;gap:12px;margin-bottom:16px}.screenshot-preview-heading h2{font-size:var(--ui-text-section);margin:0;overflow-wrap:anywhere;flex:1;min-width:0}.screenshot-preview{display:block;width:100%;height:auto;max-height:calc(100vh - 190px);object-fit:contain}.screenshot-preview-actions{margin-top:16px;justify-content:space-between;font-size:14px}.screenshot-preview-actions span{color:var(--muted)}
</style>

<style scoped>.screenshot-grid>li{position:relative}.screenshot-selection-toggle{position:absolute;z-index:1;right:9px;top:9px;display:flex;align-items:center;gap:5px;padding:5px;border:1px solid var(--line);border-radius:7px;background:var(--panel);font-size:var(--ui-text-caption)}.screenshot-selection-toggle input{margin:0;width:16px;height:16px;accent-color:var(--accent)}.screenshot-selection{margin:16px 0 22px;padding:16px;border:1px solid var(--line);border-radius:12px;background:var(--panel);min-width:0}.screenshot-selection-heading{display:flex;align-items:center;flex-wrap:wrap;gap:12px;justify-content:space-between}.screenshot-selection-heading strong{font-size:13px}.screenshot-selected-items{display:flex;flex-wrap:wrap;gap:7px;list-style:none;padding:0;margin:12px 0}.screenshot-selected-items li{display:flex;align-items:center;gap:5px;padding:2px 3px 2px 8px;max-width:100%;border:1px solid var(--line);border-radius:7px}.screenshot-selected-items li>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:240px;min-width:0;font-size:var(--ui-text-caption)}.screenshot-selected-items :deep(button){width:26px;min-height:26px;padding:4px}.screenshot-selection-toggle:has(input:focus-visible){outline:2px solid var(--accent);outline-offset:2px}@media(max-width:600px){.screenshot-selection{padding:12px}.screenshot-selected-items li>span{max-width:190px}}</style>
