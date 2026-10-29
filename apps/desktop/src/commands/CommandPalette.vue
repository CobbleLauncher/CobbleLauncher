<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle } from 'reka-ui';
import { locale } from '../i18n';
import AppIcon from '../ui/AppIcon.vue';
import { preparePinyinSearch, searchCommands, type CommandAction } from './registry';
import { readShortcuts, shortcutLabel } from './shortcuts';

const props = defineProps<{ open: boolean; commands: CommandAction[]; anchor?: HTMLElement }>();
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
function restoreFocus(event: Event) {
  if (!props.anchor) return;
  event.preventDefault();
  requestAnimationFrame(() => {
    const target = previousFocus?.isConnected && previousFocus !== document.body ? previousFocus : props.anchor?.querySelector<HTMLElement>('[data-command-trigger]');
    target?.focus({ preventScroll: true });
  });
}
const anchorStyle = ref<Record<string,string>>();
let anchorObserver: ResizeObserver | undefined;
function positionAnchor() {
  const bounds = props.anchor?.getBoundingClientRect();
  anchorStyle.value = bounds ? { top: `${bounds.top}px`, left: `${bounds.left}px`, width: `${bounds.width}px`, transform: 'none' } : undefined;
}
watch(() => [props.open,props.anchor], async () => {
  anchorObserver?.disconnect(); await nextTick(); positionAnchor();
  if (props.open && props.anchor && typeof ResizeObserver !== 'undefined') { anchorObserver = new ResizeObserver(positionAnchor); anchorObserver.observe(props.anchor); }
}, { immediate:true });
onMounted(() => window.addEventListener('resize', positionAnchor));
onBeforeUnmount(() => { anchorObserver?.disconnect(); window.removeEventListener('resize', positionAnchor); });
const emit = defineEmits<{ close: []; error: [message: string] }>();
const query = ref('');
const selected = ref(0);
const list = ref<HTMLElement | null>(null);
const running = ref(false);
const pinyinReady = ref(0);
const results = computed(() => { if (query.value.trim()) void pinyinReady.value; return searchCommands(props.commands, query.value); });
const visibleLimit = ref(50);
const visibleResults = computed(() => results.value.slice(0, visibleLimit.value));
const shortcuts = ref(readShortcuts());
const copy = computed(() => locale.value === 'zh-CN'
  ? { search: '搜索操作', empty: '没有匹配的操作', failed: '操作未完成', close: '关闭搜索' }
  : { search: 'Search actions', empty: 'No matching actions', failed: 'Action failed', close: 'Close search' });
watch(() => props.open, open => {
  if (!open) return;
  query.value = ''; selected.value = 0; visibleLimit.value = 50; shortcuts.value = readShortcuts();
  void preparePinyinSearch().then(ready => { if (ready && props.open) pinyinReady.value++; });
}, { immediate: true });
watch(results, () => { selected.value = 0; visibleLimit.value = 50; });
watch(selected, async () => { await nextTick(); list.value?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' }); });
async function choose(action: CommandAction | undefined) {
  if (!action || running.value) return;
  running.value = true;
  try { await action.run(); emit('close'); }
  catch { emit('error', copy.value.failed); }
  finally { running.value = false; }
}
function keydown(event: KeyboardEvent) {
  if (event.isComposing || event.key === 'Process') return;
  if (event.key === 'ArrowDown') { event.preventDefault(); selected.value = Math.min(selected.value + 1, results.value.length - 1); if (selected.value >= visibleLimit.value) visibleLimit.value += 50; }
  else if (event.key === 'ArrowUp') { event.preventDefault(); selected.value = Math.max(0, selected.value - 1); }
  else if (event.key === 'Enter') { event.preventDefault(); void choose(results.value[selected.value]); }
}
function scroll(event: Event) {
  const element = event.target;
  if (element instanceof HTMLElement && element.scrollTop + element.clientHeight >= element.scrollHeight - 80 && visibleLimit.value < results.value.length) visibleLimit.value += 50;
}
</script>
<template>
  <DialogRoot
    :open="open"
    @update:open="!$event && emit('close')"
  >
    <DialogPortal>
      <DialogOverlay
        class="command-overlay"
        :class="{ 'command-overlay-header': anchor }"
      />
      <DialogContent
        class="command-dialog"
        :class="{ 'command-dialog-header': anchor }"
        :style="anchorStyle"
        :aria-describedby="undefined"
        @close-auto-focus="restoreFocus"
      >
        <DialogTitle class="visually-hidden">
          {{ copy.search }}
        </DialogTitle>
        <div class="command-search-row">
          <AppIcon
            v-if="anchor"
            name="search"
            class="command-search-icon"
          />
          <input
            v-model="query"
            autofocus
            type="search"
            :placeholder="copy.search"
            :aria-label="copy.search"
            :aria-activedescendant="results.length ? `command-${selected}` : undefined"
            aria-controls="command-results"
            autocomplete="off"
            @keydown="keydown"
          >
          <button
            v-if="anchor"
            type="button"
            class="command-search-close"
            :aria-label="copy.close"
            :title="`${copy.close} (Esc)`"
            @click="emit('close')"
          >
            <AppIcon name="x" />
          </button>
        </div>
        <div
          id="command-results"
          ref="list"
          class="command-results"
          role="listbox"
          :aria-label="copy.search"
          @scroll="scroll"
        >
          <button
            v-for="(action, index) in visibleResults"
            :id="`command-${index}`"
            :key="action.id"
            type="button"
            role="option"
            :aria-selected="index === selected"
            :disabled="running"
            @mousemove="selected = index"
            @click="choose(action)"
          >
            <AppIcon
              v-if="action.icon"
              :name="action.icon"
            />
            <span>{{ action.label }}</span>
            <kbd v-if="shortcutLabel(action.shortcutId ?? action.id, shortcuts)">{{ shortcutLabel(action.shortcutId ?? action.id, shortcuts) }}</kbd>
          </button>
          <p v-if="!results.length">
            {{ copy.empty }}
          </p>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.command-overlay{position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,.48)}.command-dialog{position:fixed;z-index:1001;top:12vh;left:50%;transform:translateX(-50%);width:min(calc(100vw - 32px),520px);max-height:76vh;overflow:hidden;border:1px solid var(--line);border-radius:12px;background:var(--panel);color:var(--ink);box-shadow:0 16px 50px rgba(0,0,0,.24)}input{box-sizing:border-box;width:100%;padding:17px 18px;border:0;border-bottom:1px solid var(--line);outline:0;background:transparent;color:inherit;font:inherit}.command-results{max-height:60vh;overflow:auto;padding:6px}.command-results button{display:flex;align-items:center;gap:10px;width:100%;padding:11px 12px;border:0;border-radius:7px;text-align:left;background:transparent;color:inherit;font:inherit;cursor:pointer}.command-results button[aria-selected=true]{background:var(--hover)}.command-results button span{flex:1}.command-results kbd{color:var(--muted);font:inherit;font-size:12px}.command-results p{margin:16px 12px;color:var(--muted)}.visually-hidden{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0,0,0,0)}
</style>
<style scoped>
input::-webkit-search-cancel-button{-webkit-appearance:none}.command-search-row{position:relative}.command-search-icon{position:absolute;left:9px;top:9px;pointer-events:none;color:var(--ui-text-muted)}.command-search-close{position:absolute;right:3px;top:2px;display:grid;place-items:center;width:30px;height:30px;padding:0;border:0;border-radius:5px;background:transparent;color:var(--ui-text-muted)}.command-search-close:hover{background:var(--ui-surface-hover);color:var(--ui-text)}
.command-overlay-header{background:transparent}.command-dialog-header{border-radius:8px;max-height:calc(100vh - 64px);box-shadow:var(--ui-shadow-menu)}.command-dialog-header input{height:34px;padding:4px 40px 4px 42px;font-size:var(--ui-text-label)}.command-dialog-header .command-results{max-height:min(420px,calc(100vh - 120px))}.command-dialog-header .command-results button span{min-width:0;overflow-wrap:anywhere}
@keyframes command-search-reveal{from{opacity:.35;clip-path:inset(0 80% 0 0 round 8px)}to{opacity:1;clip-path:inset(0 round 8px)}}
@keyframes command-results-reveal{from{opacity:0;transform:translateY(-5px)}to{opacity:1;transform:translateY(0)}}
.command-dialog-header .command-search-row{animation:command-search-reveal 160ms ease-out}.command-dialog-header .command-results{animation:command-results-reveal 160ms ease-out}
@media(prefers-reduced-motion:reduce){.command-dialog-header .command-search-row,.command-dialog-header .command-results{animation:none}}
[data-motion=reduced] .command-dialog-header .command-search-row,[data-motion=reduced] .command-dialog-header .command-results{animation:none}
</style>
