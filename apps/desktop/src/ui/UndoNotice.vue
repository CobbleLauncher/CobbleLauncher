<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { t } from '../i18n';
import AppIcon from './AppIcon.vue';
defineProps<{ message: string }>();
const emit = defineEmits<{ undo: []; dismiss: [] }>();
const root = ref<HTMLElement>();
let timer: ReturnType<typeof setTimeout> | undefined;
const hovered = ref(false);
function pause() { clearTimeout(timer); }
function resume() {
  pause();
  if (!hovered.value && !root.value?.contains(document.activeElement)) timer = setTimeout(() => emit('dismiss'), 12000);
}
function focusOut(event: FocusEvent) { if (!root.value?.contains(event.relatedTarget as Node | null)) { pause(); if (!hovered.value) timer = setTimeout(() => emit('dismiss'), 12000); } }
onMounted(resume); onUnmounted(pause);
</script>
<template>
  <div
    ref="root"
    class="undo-notice"
    @mouseenter="hovered = true; pause()"
    @mouseleave="hovered = false; resume()"
    @focusin="pause"
    @focusout="focusOut"
  >
    <p role="status">
      <AppIcon name="check" />{{ message }}
    </p>
    <button
      type="button"
      class="secondary-button"
      @click="emit('undo')"
    >
      {{ t('common.undo') }}
    </button>
  </div>
</template>
<style scoped>
.undo-notice{position:sticky;bottom:16px;z-index:10;display:flex;align-items:center;gap:16px;justify-content:space-between;padding:12px 16px;margin-top:16px;border:1px solid var(--ui-border-strong);border-radius:12px;background:var(--ui-surface-1);box-shadow:var(--ui-shadow-menu);max-width:680px}
.undo-notice p{display:flex;align-items:center;gap:8px;font-size:var(--ui-text-body);margin:0;min-width:0;overflow-wrap:anywhere}.undo-notice p .app-icon{color:var(--ui-success)}
</style>
