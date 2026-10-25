<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

const props = withDefaults(defineProps<{ disabled?: boolean }>(), { disabled: false });
const emit = defineEmits<{ error: [error: unknown] }>();
const directions = ['North', 'NorthEast', 'East', 'SouthEast', 'South', 'SouthWest', 'West', 'NorthWest'] as const;
type Direction = typeof directions[number];
const available = ref(false);
let disposed = false;
let unlisten: (() => void) | undefined;
let checking = 0;

async function syncState() {
  const check = ++checking;
  try {
    const current = getCurrentWindow();
    const [maximized, fullscreen, resizable] = await Promise.all([current.isMaximized(), current.isFullscreen(), current.isResizable()]);
    const enabled = !maximized && !fullscreen && resizable;
    if (!disposed && check === checking) available.value = enabled;
    return enabled;
  } catch {
    if (!disposed && check === checking) available.value = false;
    return false;
  }
}

async function beginResize(event: MouseEvent, direction: Direction) {
  if (event.button !== 0 || props.disabled || !available.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (!await syncState() || disposed || props.disabled) return;
  try { await getCurrentWindow().startResizeDragging(direction); }
  catch (error) { emit('error', error); }
}

function refresh() { void syncState(); }
onMounted(async () => {
  if (!isTauri()) return;
  refresh();
  window.addEventListener('focus', refresh);
  try {
    const disposeListener = await getCurrentWindow().onResized(refresh);
    if (disposed) disposeListener();
    else unlisten = disposeListener;
  } catch { /* Native state is still checked before each resize gesture. */ }
});
onUnmounted(() => {
  disposed = true;
  unlisten?.();
  window.removeEventListener('focus', refresh);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="available && !disabled"
      class="window-resize-edges"
      aria-hidden="true"
    >
      <div
        v-for="direction in directions"
        :key="direction"
        class="window-resize-edge"
        :data-resize-direction="direction"
        @mousedown="beginResize($event, direction)"
      />
    </div>
  </Teleport>
</template>

<style scoped>
/* Cancel appearance zoom so the hit areas remain at the physical window edges. */
.window-resize-edges { position:fixed; inset:0; z-index:1000; pointer-events:none; zoom:calc(1 / var(--appearance-scale, 1)); }
.window-resize-edge { position:absolute; pointer-events:auto; touch-action:none; }
[data-resize-direction="North"] { top:0; left:8px; right:8px; height:4px; cursor:n-resize; }
[data-resize-direction="South"] { bottom:0; left:8px; right:8px; height:4px; cursor:s-resize; }
[data-resize-direction="West"] { top:8px; bottom:8px; left:0; width:4px; cursor:w-resize; }
[data-resize-direction="East"] { top:8px; bottom:8px; right:0; width:4px; cursor:e-resize; }
[data-resize-direction="NorthWest"] { top:0; left:0; width:8px; height:8px; cursor:nw-resize; }
[data-resize-direction="NorthEast"] { top:0; right:0; width:8px; height:8px; cursor:ne-resize; }
[data-resize-direction="SouthWest"] { bottom:0; left:0; width:8px; height:8px; cursor:sw-resize; }
[data-resize-direction="SouthEast"] { bottom:0; right:0; width:8px; height:8px; cursor:se-resize; }
</style>
