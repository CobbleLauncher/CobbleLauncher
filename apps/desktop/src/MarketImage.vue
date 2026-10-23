<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { marketImage } from './market-images';
import { t } from './i18n';
const props = defineProps<{ provider: 'modrinth' | 'curseforge'; url?: string | null; title: string; preview?: boolean; artwork?: boolean; full?: boolean; alt?: string }>();
const emit = defineEmits<{ failed: [] }>();
const root = ref<HTMLElement>();
const image = ref<string | null>(null);
const loading = ref(false);
let observer: IntersectionObserver | undefined;
let generation = 0;
async function load(current: number) {
  observer?.disconnect();
  if (!props.url) return;
  const result = await marketImage(props.provider, props.url, props.preview ? 'preview' : 'icon');
  if (current !== generation) return;
  image.value = result;
  loading.value = false;
  if (!result) emit('failed');
}
function observe() {
  const current = ++generation; image.value = null; loading.value = !!props.url; observer?.disconnect();
  if (!props.url || !root.value) return;
  if (typeof IntersectionObserver === 'undefined') { void load(current); return; }
  observer = new IntersectionObserver(entries => { if (entries.some(entry => entry.isIntersecting)) void load(current); }, { rootMargin: '80px' });
  observer.observe(root.value);
}
onMounted(observe);
watch(() => [props.url, props.provider, props.preview], observe);
onUnmounted(() => { ++generation; observer?.disconnect(); });
</script>
<template>
  <span
    ref="root"
    class="market-image"
    :class="{ preview, artwork, full }"
    :aria-busy="loading"
  >
    <img
      v-if="image"
      :src="image"
      :alt="alt ?? ''"
      loading="lazy"
      decoding="async"
      referrerpolicy="no-referrer"
      @error="image = null; emit('failed')"
    >
    <span
      v-else-if="preview && loading"
      class="market-image-loading"
      role="status"
    >{{ t('market.imageLoading') }}</span>
    <span
      v-else
      aria-hidden="true"
    >{{ title.slice(0, 1) }}</span>
  </span>
</template>
<style scoped>
.market-image{display:grid;place-items:center;flex:0 0 48px;width:48px;height:48px;border-radius:10px;overflow:hidden;background:var(--surface-2);color:var(--muted);font-weight:700;font-size:20px}
.market-image img{display:block;width:100%;height:100%;object-fit:contain}
.market-image.preview{width:144px;height:auto;aspect-ratio:16/9;flex-basis:144px}
.market-image.preview img{object-fit:cover}
.market-image.artwork{width:144px;height:auto;aspect-ratio:16/9;flex-basis:144px}
.market-image.artwork.preview img{object-fit:cover}
.market-image.full{width:100%;height:auto;aspect-ratio:16/9;max-height:400px;flex-basis:auto}
.market-image.full img{object-fit:contain}
.market-image-loading{font-size:12px;font-weight:400}
@media(max-width:560px){.market-image.preview:not(.full){width:104px;flex-basis:104px}}
@media(max-width:560px){.market-image.artwork:not(.full){width:104px;flex-basis:104px}}
</style>
