<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import AppIcon from './ui/AppIcon.vue';
const props = defineProps<{ id: string; available?: boolean }>();
const container = ref<HTMLElement | null>(null);
const image = ref<string | null>(null);
let observer: IntersectionObserver | undefined;
let visible = false;
let revision = 0;
async function load() {
  const current = ++revision;
  if (!visible || !props.available) { image.value = null; return; }
  try { const value = await invoke<string | null>('read_blueprint_thumbnail', { id: props.id }); if (current === revision) image.value = value; }
  catch { if (current === revision) image.value = null; }
}
watch(() => [props.id, props.available], load);
onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') { visible = true; void load(); return; }
  observer = new IntersectionObserver(entries => { if (entries.some(entry => entry.isIntersecting)) { visible = true; observer?.disconnect(); void load(); } }, { rootMargin: '160px' });
  if (container.value) observer.observe(container.value);
});
onUnmounted(() => { revision++; observer?.disconnect(); });
</script>
<template>
  <span
    ref="container"
    class="blueprint-thumbnail"
    aria-hidden="true"
  >
    <img
      v-if="image"
      :src="image"
      alt=""
      loading="lazy"
    >
    <AppIcon
      v-else
      name="puzzle"
      :size="36"
    />
  </span>
</template>
<style scoped>
.blueprint-thumbnail{height:140px;width:100%;display:grid;place-items:center;color:var(--muted);background:var(--bg);border-radius:8px;overflow:hidden}.blueprint-thumbnail img{width:100%;height:100%;object-fit:contain}
</style>
