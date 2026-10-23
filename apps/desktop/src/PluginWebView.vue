<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';

const props = withDefaults(defineProps<{
  pluginId: string;
  instanceId?: string;
  keybindingTargetId?: string;
  active?: boolean;
  suspended?: boolean;
}>(), { instanceId: undefined, keybindingTargetId: undefined, active: true, suspended: false });
const emit = defineEmits<{ unsupported: [] }>();
const container = ref<HTMLElement>();
const pending = ref(false);
const failed = ref(false);
let viewId: string | undefined;
let revision = 0;
const openings = new Set<Promise<{ view_id: string }>>();
let disposed = false;
let locallySuspended = false;
let visibilityRevision = 0;
let observer: ResizeObserver | undefined;
let themeObserver: MutationObserver | undefined;
let frame = 0;
const media = window.matchMedia('(prefers-color-scheme: dark)');

function theme(): 'light' | 'dark' {
  const value = document.documentElement.dataset.theme;
  return value === 'dark' || (value !== 'light' && media.matches) ? 'dark' : 'light';
}
function bounds() {
  const rect = container.value?.getBoundingClientRect();
  if (!rect || rect.width < 1 || rect.height < 1) return undefined;
  const x = Math.max(0, rect.left);
  const y = Math.max(0, rect.top);
  const width = Math.min(rect.right, window.innerWidth) - x;
  const height = Math.min(rect.bottom, window.innerHeight) - y;
  return width >= 1 && height >= 1 ? { x, y, width, height } : undefined;
}
async function close() {
  ++revision;
  const id = viewId;
  viewId = undefined;
  if (id) await invoke('close_plugin_view', { viewId: id });
}
async function open() {
  const current = ++revision;
  const previous = viewId;
  viewId = undefined;
  pending.value = true;
  failed.value = false;
  try {
    if (previous) await invoke('close_plugin_view', { viewId: previous });
    await nextTick();
    if (current !== revision || disposed || !props.active || props.suspended || locallySuspended) return;
    const area = bounds();
    if (!area) return;
    const opening = invoke<{ view_id: string }>('open_plugin_view', {
      pluginId: props.pluginId, instanceId: props.instanceId ?? null,
      keybindingTargetId: props.keybindingTargetId ?? null,
      bounds: area, locale: locale.value, theme: theme(),
    });
    openings.add(opening);
    let result: { view_id: string };
    try { result = await opening; } finally { openings.delete(opening); }
    if (current !== revision || disposed || !props.active) {
      await invoke('close_plugin_view', { viewId: result.view_id });
      return;
    }
    viewId = result.view_id;
    if (props.suspended || locallySuspended) await invoke('set_plugin_view_visible', { viewId: result.view_id, visible: false });
  } catch (error) {
    if (current === revision && !disposed) {
      if (['PLUGIN_FRONTEND_ISOLATION_UNVERIFIED', 'PLUGIN_VIEW_RUNTIME_UNSUPPORTED'].includes(String(error))) emit('unsupported');
      else failed.value = true;
    }
  } finally {
    if (current === revision) pending.value = false;
  }
}
function themeChanged() {
  if (props.active && !props.suspended && !locallySuspended) void open();
}
function resize() {
  cancelAnimationFrame(frame);
  frame = requestAnimationFrame(() => {
    const area = bounds();
    const id = viewId;
    if (id && area) void invoke('resize_plugin_view', { viewId: id, bounds: area }).catch(() => {
      if (viewId === id) failed.value = true;
    });
    else if (!id && area && !pending.value && props.active && !props.suspended && !locallySuspended) void open();
  });
}
async function suspend() {
  const current = ++visibilityRevision;
  locallySuspended = true;
  await Promise.allSettled([...openings]);
  if (current === visibilityRevision && locallySuspended && viewId) {
    await invoke('set_plugin_view_visible', { viewId, visible: false });
  }
}
async function resume() {
  const current = ++visibilityRevision;
  locallySuspended = false;
  if (!props.active || props.suspended) return;
  const area = bounds();
  const id = viewId;
  if (id && area) {
    await invoke('resize_plugin_view', { viewId: id, bounds: area });
    if (current === visibilityRevision && viewId === id && props.active && !props.suspended && !locallySuspended) {
      await invoke('set_plugin_view_visible', { viewId: id, visible: true });
    }
  } else await open();
}
watch(() => [props.pluginId, props.instanceId, props.keybindingTargetId, props.active, locale.value], () => {
  if (!props.active) void close();
  else void open();
});
watch(() => props.suspended, value => { if (value) void suspend(); else void resume(); });
onMounted(() => {
  observer = new ResizeObserver(resize);
  if (container.value) observer.observe(container.value);
  themeObserver = new MutationObserver(() => { if (props.active && !props.suspended && !locallySuspended) void open(); });
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  media.addEventListener('change', themeChanged);
  window.addEventListener('resize', resize);
  window.addEventListener('scroll', resize, true);
  void open();
});
onUnmounted(() => {
  disposed = true;
  observer?.disconnect();
  themeObserver?.disconnect();
  cancelAnimationFrame(frame);
  media.removeEventListener('change', themeChanged);
  window.removeEventListener('resize', resize);
  window.removeEventListener('scroll', resize, true);
  void close();
});
defineExpose({ suspend, resume, confirmLeave: async () => { await close(); return true; } });
</script>
<template>
  <div
    ref="container"
    class="plugin-web-view"
    :aria-busy="pending"
  >
    <LoadingIndicator
      v-if="pending"
      :label="t('navigation.loading')"
    />
    <p
      v-if="failed"
      role="alert"
      class="settings-error"
    >
      {{ t('navigation.loadError') }}
      <button
        class="text-button"
        @click="open"
      >
        {{ t('navigation.retry') }}
      </button>
    </p>
  </div>
</template>
<style scoped>
.plugin-web-view{width:100%;min-width:0;height:max(360px,calc(100dvh - 180px));position:relative;overflow:hidden}
</style>
