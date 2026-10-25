<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle } from 'reka-ui';
import { locale } from '../i18n';
import AppIcon from '../ui/AppIcon.vue';
import { preparePinyinSearch, searchCommands, type CommandAction } from './registry';
import { readShortcuts, shortcutLabel } from './shortcuts';

const props = defineProps<{ open: boolean; commands: CommandAction[] }>();
const emit = defineEmits<{ close: []; error: [message: string] }>();
const query = ref('');
const selected = ref(0);
const list = ref<HTMLElement | null>(null);
const running = ref(false);
const pinyinReady = ref(0);
const results = computed(() => { void pinyinReady.value; return searchCommands(props.commands, query.value); });
const visibleLimit = ref(50);
const visibleResults = computed(() => results.value.slice(0, visibleLimit.value));
const shortcuts = ref(readShortcuts());
const copy = computed(() => locale.value === 'zh-CN'
  ? { search: '搜索操作', empty: '没有匹配的操作', failed: '操作未完成' }
  : { search: 'Search actions', empty: 'No matching actions', failed: 'Action failed' });
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
      <DialogOverlay class="command-overlay" />
      <DialogContent
        class="command-dialog"
        :aria-describedby="undefined"
      >
        <DialogTitle class="visually-hidden">
          {{ copy.search }}
        </DialogTitle>
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


