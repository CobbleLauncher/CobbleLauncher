<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui';
import { locale, t } from './i18n';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { readAppearance } from './theme';

const props = withDefaults(defineProps<{
  pluginId: string;
  instanceId?: string;
  keybindingTargetId?: string;
  handoffId?: string;
  configRestoreTarget?: {configId:string;backupId:string};
  active?: boolean;
  suspended?: boolean;
}>(), { instanceId: undefined, keybindingTargetId: undefined, handoffId: undefined, configRestoreTarget: undefined, active: true, suspended: false });
const emit = defineEmits<{ unsupported: []; ready: [] }>();
const container = ref<HTMLElement>();
const pending = ref(false);
const failed = ref(false);
const leaveCheckError = ref(false);
const leaveActionFailed = ref(false);
const leavePrompt = ref(false);
let viewId: string | undefined;
let revision = 0;
let submittedHandoffId: string | undefined;
const openings = new Set<Promise<{ view_id: string }>>();
let disposed = false;
let deferredReopen = false;
let pendingLeave: ((allow: boolean) => void) | undefined;
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
  if (!id) return;
  await invoke('close_plugin_view', { viewId: id });
  if (viewId === id) viewId = undefined;
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
    const handoffId = props.handoffId && props.handoffId !== submittedHandoffId ? props.handoffId : null;
    const opening = invoke<{ view_id: string }>('open_plugin_view', {
      pluginId: props.pluginId, instanceId: props.instanceId ?? null,
      keybindingTargetId: props.keybindingTargetId ?? null,
      handoffId,
      configRestoreTarget: props.configRestoreTarget ?? null,
      bounds: area, locale: locale.value, theme: theme(), appearance: readAppearance(),
    });
    openings.add(opening);
    let result: { view_id: string };
    try { result = await opening; } finally { openings.delete(opening); }
    if (current !== revision || disposed || !props.active) {
      await invoke('close_plugin_view', { viewId: result.view_id });
      return;
    }
    viewId = result.view_id;
    if (handoffId) submittedHandoffId = handoffId;
    if (props.suspended || locallySuspended) await invoke('set_plugin_view_visible', { viewId: result.view_id, visible: false });
    else emit('ready');
  } catch (error) {
    if (current === revision && !disposed) {
      if (['PLUGIN_FRONTEND_ISOLATION_UNVERIFIED', 'PLUGIN_VIEW_RUNTIME_UNSUPPORTED'].includes(String(error))) emit('unsupported');
      else failed.value = true;
    }
  } finally {
    if (current === revision) pending.value = false;
  }
}
async function requestReopen() {
  if (!props.active) return;
  if (props.suspended || locallySuspended) { deferredReopen = true; return; }
  const id = viewId;
  if (!id) { deferredReopen = false; await open(); return; }
  try {
    if (await invoke<boolean>('plugin_view_is_dirty', { viewId: id })) {
      deferredReopen = true;
      return;
    }
    deferredReopen = false;
    await open();
  } catch {
    deferredReopen = true;
    leaveCheckError.value = true;
  }
}
function themeChanged() {
  void requestReopen();
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
  if (deferredReopen && current === visibilityRevision && !locallySuspended) await requestReopen();
}
async function finishLeave(allow: boolean) {
  leaveActionFailed.value = false;
  try {
    if (allow) await close();
    else await resume();
  } catch {
    leaveActionFailed.value = true;
    return;
  }
  leavePrompt.value = false;
  leaveCheckError.value = false;
  const resolve = pendingLeave;
  pendingLeave = undefined;
  resolve?.(allow);
}
async function confirmLeave(): Promise<boolean> {
  const id = viewId;
  if (!id) return true;
  leaveCheckError.value = false;
  try {
    await suspend();
    const dirty = await invoke<boolean>('plugin_view_is_dirty', { viewId: id });
    if (!dirty) { await close(); return true; }
    leavePrompt.value = true;
    return await new Promise(resolve => { pendingLeave = resolve; });
  } catch {
    leaveCheckError.value = true;
    leavePrompt.value = true;
    return await new Promise(resolve => { pendingLeave = resolve; });
  }
}
watch(() => [props.pluginId, props.instanceId, props.keybindingTargetId, props.handoffId, props.configRestoreTarget, props.active, locale.value], () => {
  if (!props.active) void close();
  else void requestReopen();
});
watch(() => props.suspended, value => { if (value) void suspend(); else void resume(); });
onMounted(() => {
  observer = new ResizeObserver(resize);
  if (container.value) observer.observe(container.value);
  themeObserver = new MutationObserver(themeChanged);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-accent', 'data-density', 'data-motion', 'style'] });
  media.addEventListener('change', themeChanged);
  window.addEventListener('resize', resize);
  window.addEventListener('scroll', resize, true);
  void open();
});
onUnmounted(() => {
  disposed = true;
  pendingLeave?.(false);
  pendingLeave = undefined;
  observer?.disconnect();
  themeObserver?.disconnect();
  cancelAnimationFrame(frame);
  media.removeEventListener('change', themeChanged);
  window.removeEventListener('resize', resize);
  window.removeEventListener('scroll', resize, true);
  void close();
});
defineExpose({ suspend, resume, confirmLeave });
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
  <DialogRoot
    :open="leavePrompt"
    @update:open="!$event && finishLeave(false)"
  >
    <DialogPortal>
      <DialogOverlay class="settings-dialog-overlay" />
      <DialogContent class="settings-leave-dialog">
        <DialogTitle>{{ t('settingsLayout.discardTitle') }}</DialogTitle>
        <p
          v-if="leaveCheckError"
          class="settings-error"
          role="alert"
        >
          {{ t('navigation.pluginDraftCheckFailed') }}
        </p>
        <p
          v-if="leaveActionFailed"
          class="settings-error"
          role="alert"
        >
          {{ t('navigation.pluginDraftLeaveFailed') }}
        </p>
        <div class="settings-inline-actions">
          <button
            class="text-button"
            @click="finishLeave(true)"
          >
            {{ t('settingsLayout.discard') }}
          </button>
          <button
            class="text-button"
            @click="finishLeave(false)"
          >
            {{ t('settingsLayout.continueEditing') }}
          </button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.plugin-web-view{width:100%;min-width:0;height:max(360px,calc(100dvh - 180px));position:relative;overflow:hidden}
.settings-leave-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}
.settings-leave-dialog h2{font-size:16px;margin:0 0 20px}
.settings-dialog-overlay{position:fixed;inset:0;background:#0006;z-index:100}
.settings-inline-actions{display:flex;gap:12px;align-items:center;flex-wrap:wrap}
</style>
