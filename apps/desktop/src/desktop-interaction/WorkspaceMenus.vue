<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription } from 'reka-ui';
import AppIcon from '../ui/AppIcon.vue';
import type { CommandAction } from '../commands/registry';
import { locale } from '../i18n';
import { editingActions } from './edit-actions';
import { blankWorkspaceTarget, clampCenter, editableElement, interfaceZoom, selectableElement } from './targets';

const props = defineProps<{ actions: CommandAction[]; contextKey: string; disabled?: boolean }>();
const emit = defineEmits<{ error: [message: string] }>();
const menuOpen = ref(false), radialOpen = ref(false);
const menuItems = ref<CommandAction[]>([]);
const anchor = ref({ x: 0, y: 0 }), center = ref({ x: 0, y: 0 });
const focusOrigin = ref<HTMLElement | null>(null);
const radial = computed(() => props.actions.filter(action => action.enabled !== false).slice(0, 6));
const labels = computed(() => locale.value === 'zh-CN' ? { title: '快捷操作', close: '关闭', copy: '复制', cut: '剪切', paste: '粘贴', selectAll: '全选', error: '操作未完成，请重试' } : { title: 'Quick actions', close: 'Close', copy: 'Copy', cut: 'Cut', paste: 'Paste', selectAll: 'Select all', error: 'Could not complete the action. Try again.' });
let timer: ReturnType<typeof setTimeout> | undefined;
let press: { x: number; y: number; pointerId: number } | null = null;
let swallowClick = false;
function stopPress() { if (timer) clearTimeout(timer); timer = undefined; press = null; }
function close() { stopPress(); menuOpen.value = false; radialOpen.value = false; swallowClick = false; }
function returnFocus(event: Event) { event.preventDefault(); if (focusOrigin.value?.isConnected) focusOrigin.value.focus({ preventScroll: true }); }
function rememberFocus(target: EventTarget | null) {
  focusOrigin.value = target instanceof HTMLElement && editableElement(target) ? target : document.activeElement instanceof HTMLElement ? document.activeElement : null;
}
function run(action: CommandAction) {
  close();
  if (action.enabled === false) return;
  void Promise.resolve().then(() => action.run()).catch(() => emit('error', labels.value.error));
}
function onContext(event: MouseEvent) {
  if (event.defaultPrevented) return; // Component-owned row menus retain priority.
  event.preventDefault();
  stopPress();
  if (event.target instanceof Element && event.target.closest('[role="menu"],.window-controls,.window-resize-edge')) return;
  const editable = editableElement(event.target), selectable = selectableElement(event.target);
  if (props.disabled && !editable && !selectable) return;
  const items = editable ? editingActions(editable, labels.value, true) : selectable ? editingActions(selectable, labels.value, false) : blankWorkspaceTarget(event.target) ? props.actions : [];
  if (!items.length) return;
  rememberFocus(event.target);
  radialOpen.value = false;
  menuItems.value = [...items];
  const zoom = interfaceZoom();
  const rect = event.target instanceof Element ? event.target.getBoundingClientRect() : null;
  anchor.value = { x: (event.clientX || rect?.left || 12) / zoom, y: (event.clientY || rect?.bottom || 12) / zoom };
  menuOpen.value = true;
}
function onPointerDown(event: PointerEvent) {
  stopPress();
  if (props.disabled || event.button !== 0 || event.pointerType === 'touch' || !blankWorkspaceTarget(event.target) || document.querySelector('[role="dialog"][aria-modal="true"],[role="menu"]')) return;
  press = { x: event.clientX, y: event.clientY, pointerId: event.pointerId };
  rememberFocus(event.target);
  timer = setTimeout(() => {
    timer = undefined;
    if (!press || !radial.value.length || props.disabled) return;
    const zoom = interfaceZoom();
    center.value = clampCenter(press.x / zoom, press.y / zoom, window.innerWidth / zoom, window.innerHeight / zoom, 304);
    swallowClick = true;
    menuOpen.value = false; radialOpen.value = true;
  }, 550);
}
function onPointerMove(event: PointerEvent) {
  if (press && timer && (event.pointerId !== press.pointerId || Math.hypot(event.clientX - press.x, event.clientY - press.y) > 8)) stopPress();
}
function onPointerUp() {
  stopPress();
  if (swallowClick) setTimeout(() => { swallowClick = false; }, 0);
}
function onClick(event: MouseEvent) { if (swallowClick) { swallowClick = false; event.preventDefault(); event.stopImmediatePropagation(); } }
function onKey(event: KeyboardEvent) { if (event.key === 'Escape') stopPress(); }
function radialPosition(index: number) {
  const angle = -Math.PI / 2 + index * 2 * Math.PI / radial.value.length;
  return { left: `${152 + 104 * Math.cos(angle)}px`, top: `${152 + 104 * Math.sin(angle)}px` };
}
function radialKeys(event: KeyboardEvent) {
  if (!['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  const items = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[data-radial-action]')];
  if (!items.length) return;
  event.preventDefault();
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (index + (event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length;
  items[next]?.focus();
}
function focusRadial(event: Event) { event.preventDefault(); void nextTick(() => document.querySelector<HTMLButtonElement>('[data-radial-action]')?.focus({ preventScroll: true })); }
watch(() => props.contextKey, close);
watch(() => props.disabled, value => { if (value) close(); });
onMounted(() => {
  document.addEventListener('contextmenu', onContext);
  document.addEventListener('pointerdown', onPointerDown);
  document.addEventListener('pointermove', onPointerMove);
  document.addEventListener('pointerup', onPointerUp);
  document.addEventListener('pointercancel', stopPress);
  document.addEventListener('click', onClick, true);
  document.addEventListener('keydown', onKey);
  window.addEventListener('blur', close);
  window.addEventListener('resize', close);
  window.addEventListener('scroll', stopPress, true);
});
onUnmounted(() => {
  close(); document.removeEventListener('contextmenu', onContext); document.removeEventListener('pointerdown', onPointerDown);
  document.removeEventListener('pointermove', onPointerMove); document.removeEventListener('pointerup', onPointerUp); document.removeEventListener('pointercancel', stopPress);
  document.removeEventListener('click', onClick, true); document.removeEventListener('keydown', onKey);
  window.removeEventListener('blur', close); window.removeEventListener('resize', close); window.removeEventListener('scroll', stopPress, true);
});
</script>
<template>
  <DropdownMenuRoot
    v-model:open="menuOpen"
    :modal="false"
  >
    <DropdownMenuTrigger
      class="workspace-menu-anchor"
      :style="{ left: `${anchor.x}px`, top: `${anchor.y}px` }"
      tabindex="-1"
      :aria-label="labels.title"
    />
    <DropdownMenuPortal v-if="menuOpen">
      <div class="workspace-menu-positioner">
        <DropdownMenuContent
          class="profile-menu workspace-menu"
          style="pointer-events: auto"
          position-strategy="absolute"
          align="start"
          :side-offset="0"
          :collision-padding="12"
          @close-auto-focus="returnFocus"
        >
          <DropdownMenuItem
            v-for="item in menuItems"
            :key="item.id"
            class="profile-menu-item"
            :disabled="item.enabled === false"
            @select="run(item)"
          >
            <AppIcon :name="item.icon ?? 'more-horizontal'" />{{ item.label }}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </div>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
  <DialogRoot v-model:open="radialOpen">
    <DialogPortal v-if="radialOpen">
      <DialogOverlay
        class="radial-overlay"
        @pointerdown.self="close"
      />
      <DialogContent
        class="radial-menu"
        :style="{ left: `${center.x}px`, top: `${center.y}px` }"
        @close-auto-focus="returnFocus"
        @open-auto-focus="focusRadial"
        @keydown="radialKeys"
      >
        <DialogTitle class="sr-only">
          {{ labels.title }}
        </DialogTitle>
        <DialogDescription class="sr-only">
          {{ locale === 'zh-CN' ? '选择操作，或按 Escape 关闭' : 'Choose an action, or press Escape to close' }}
        </DialogDescription>
        <button
          v-for="(item, index) in radial"
          :key="item.id"
          type="button"
          class="radial-action"
          :style="radialPosition(index)"
          :data-radial-action="item.id"
          @click="run(item)"
        >
          <AppIcon
            :name="item.icon ?? 'puzzle'"
            :size="20"
          /><span>{{ item.label }}</span>
        </button>
        <button
          type="button"
          class="radial-close"
          :aria-label="labels.close"
          @click="close"
        >
          <AppIcon
            name="x"
            :size="20"
          />
        </button>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.workspace-menu-anchor{position:fixed;width:1px;height:1px;opacity:0;pointer-events:none;padding:0;border:0}
.workspace-menu-positioner{position:fixed;inset:0;z-index:90;pointer-events:none}
.workspace-menu-positioner :deep(.workspace-menu){min-width:220px;max-width:min(360px,calc(100vw - 24px))}
.workspace-menu-positioner :deep(.profile-menu-item){font-size:var(--ui-text-label);min-height:40px;line-height:var(--ui-leading-label)}
.radial-overlay{position:fixed;inset:0;z-index:90;background:color-mix(in srgb,var(--ui-text) 8%,transparent)}
.radial-menu{position:fixed;z-index:91;transform:translate(-50%,-50%);width:304px;height:304px;border-radius:50%;outline:none;background:color-mix(in srgb,var(--ui-surface-1) 95%,transparent);box-shadow:0 12px 42px #0003;border:1px solid var(--ui-border)}
.radial-action{position:absolute;transform:translate(-50%,-50%);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:6px;width:96px;min-height:64px;padding:8px;border:0;border-radius:12px;background:transparent;color:var(--ui-text);font:inherit;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);text-align:center}
.radial-action:hover,.radial-action:focus-visible{background:var(--ui-surface-hover);color:var(--ui-text);outline:var(--ui-focus-ring)}
.radial-action span{overflow-wrap:anywhere;max-width:100%}.radial-action svg{color:var(--ui-text-muted)}
.radial-close{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:48px;height:48px;border:1px solid var(--ui-border);border-radius:50%;display:grid;place-items:center;background:var(--ui-surface-2);color:var(--ui-text-muted)}
.radial-close:focus-visible{outline:var(--ui-focus-ring)}
</style>
