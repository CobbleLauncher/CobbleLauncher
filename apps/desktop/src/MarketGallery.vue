<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { DialogClose, DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle, DialogTrigger } from 'reka-ui';
import { t } from './i18n';
import MarketImage from './MarketImage.vue';
import AppIcon from './ui/AppIcon.vue';
const props = defineProps<{ provider: 'modrinth' | 'curseforge'; urls: string[]; title: string }>();
const selected = ref('');
const expanded = ref(false);
const thumbnails = ref<HTMLElement>();
const failed = ref(new Set<string>());
const images = computed(() => [...new Set(props.urls)].slice(0, 12).filter(url => !failed.value.has(url)));
const current = computed(() => images.value.includes(selected.value) ? selected.value : images.value[0]);
const index = computed(() => current.value ? images.value.indexOf(current.value) : 0);
watch(() => [props.provider, props.urls], () => { selected.value = ''; failed.value = new Set(); expanded.value = false; });
function fail(url: string) { failed.value = new Set([...failed.value, url]); if (!images.value.length) expanded.value = false; }
function move(event: KeyboardEvent, focus = false) {
  let next: number;
  if (event.key === 'ArrowRight') next = (index.value + 1) % images.value.length;
  else if (event.key === 'ArrowLeft') next = (index.value - 1 + images.value.length) % images.value.length;
  else if (event.key === 'Home') next = 0;
  else if (event.key === 'End') next = images.value.length - 1;
  else return;
  event.preventDefault();
  selected.value = images.value[next] ?? '';
  if (focus) void nextTick(() => thumbnails.value?.querySelectorAll<HTMLButtonElement>('button')[next]?.focus());
}
</script>
<template>
  <div
    v-if="current"
    class="market-gallery"
  >
    <DialogRoot v-model:open="expanded">
      <DialogTrigger
        class="market-gallery-open"
        :aria-label="t('market.openPreview', { name: title })"
      >
        <MarketImage
          :key="current"
          :provider="provider"
          :url="current"
          :title="title"
          preview
          full
          :alt="t('market.previewImage', { name: title, index: index + 1 })"
          @failed="fail(current!)"
        />
      </DialogTrigger>
      <DialogPortal>
        <DialogOverlay class="market-gallery-overlay" />
        <DialogContent
          class="market-gallery-dialog"
          :aria-describedby="undefined"
          @keydown="move($event)"
        >
          <div class="market-gallery-toolbar">
            <DialogTitle>{{ title }}</DialogTitle>
            <span
              class="market-gallery-position"
              aria-live="polite"
            >{{ index + 1 }} / {{ images.length }}</span>
            <DialogClose
              class="market-gallery-close"
              :aria-label="t('market.closePreview')"
            >
              <AppIcon name="x" />
            </DialogClose>
          </div>
          <MarketImage
            :key="current"
            :provider="provider"
            :url="current"
            :title="title"
            preview
            full
            :alt="t('market.previewImage', { name: title, index: index + 1 })"
            @failed="fail(current!)"
          />
          <div
            v-if="images.length > 1"
            class="market-gallery-navigation"
          >
            <button @click="selected = images[(index - 1 + images.length) % images.length]!">
              {{ t('market.previousImage') }}
            </button>
            <button @click="selected = images[(index + 1) % images.length]!">
              {{ t('market.nextImage') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
    <div
      v-if="images.length > 1"
      ref="thumbnails"
      class="market-gallery-thumbnails"
      role="group"
      :aria-label="t('market.previewImages')"
      @keydown="move($event, true)"
    >
      <button
        v-for="(url, imageIndex) in images"
        :key="url"
        class="market-gallery-choice"
        :aria-label="t('market.previewImage', { name: title, index: imageIndex + 1 })"
        :aria-pressed="url === current"
        :tabindex="url === current ? 0 : -1"
        @click="selected = url"
      >
        <MarketImage
          :provider="provider"
          :url="url"
          :title="title"
          preview
          @failed="fail(url)"
        />
      </button>
    </div>
  </div>
</template>
<style scoped>
.market-gallery{max-width:840px;margin:16px 0}
.market-gallery-open{display:block;width:100%;padding:0;border:0;border-radius:12px;background:var(--surface-2);cursor:zoom-in;overflow:hidden}
.market-gallery-open :deep(.market-image.full){aspect-ratio:16/9;max-height:none;border-radius:0}
.market-gallery-thumbnails{display:flex;gap:8px;overflow-x:auto;padding:8px 2px}
.market-gallery-choice{padding:3px;border:1px solid transparent;border-radius:10px;background:transparent;flex-shrink:0;cursor:pointer}
.market-gallery-choice :deep(.market-image.preview){width:112px;height:auto;aspect-ratio:16/9;flex-basis:auto;border-radius:6px}
.market-gallery-choice[aria-pressed=true]{border-color:var(--accent)}
.market-gallery-choice:hover{background:var(--surface-2)}
.market-gallery-open:focus-visible,.market-gallery-choice:focus-visible,.market-gallery-close:focus-visible,.market-gallery-navigation button:focus-visible{outline:2px solid var(--accent);outline-offset:3px}
.market-gallery-overlay{position:fixed;inset:0;z-index:110;background:#000b}
.market-gallery-dialog{position:fixed;z-index:111;left:50%;top:50%;transform:translate(-50%,-50%);box-sizing:border-box;width:min(1200px,calc(100vw - 32px));max-height:calc(100dvh - 32px);padding:16px;overflow:auto;border:1px solid var(--line);border-radius:14px;background:var(--panel);color:var(--ink)}
.market-gallery-toolbar{display:flex;align-items:center;gap:16px;margin-bottom:12px}
.market-gallery-toolbar h2{flex:1;min-width:0;margin:0;font-size:15px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.market-gallery-position{color:var(--muted);font-size:12px;font-variant-numeric:tabular-nums}
.market-gallery-close{display:grid;place-items:center;flex:none;width:32px;height:32px;border:0;border-radius:8px;background:transparent;color:var(--ink);cursor:pointer}
.market-gallery-close:hover{background:var(--hover)}
.market-gallery-close :deep(svg){width:18px;height:18px}
.market-gallery-dialog :deep(.market-image.full){aspect-ratio:16/9;max-height:calc(100dvh - 180px);border-radius:8px}
.market-gallery-navigation{display:flex;justify-content:space-between;gap:16px;margin-top:12px}
.market-gallery-navigation button{padding:7px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:12px;cursor:pointer}
@media(max-width:560px){.market-gallery-dialog{padding:12px;width:calc(100vw - 16px);max-height:calc(100dvh - 16px)}.market-gallery-choice :deep(.market-image.preview){width:88px}}
</style>
