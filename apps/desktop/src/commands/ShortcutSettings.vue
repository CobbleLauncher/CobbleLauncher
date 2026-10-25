<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { locale, t } from '../i18n';
import ConfirmationDialog from '../ui/ConfirmationDialog.vue';
import type { CommandAction } from './registry';
import { clearShortcut, readShortcuts, replaceShortcuts, resetShortcuts, saveShortcut, shortcutConflict, shortcutChangedEvent, shortcutDefaults, shortcutFromEvent } from './shortcuts';

const props = withDefaults(defineProps<{ commands?: CommandAction[]; staged?: boolean }>(), { commands: () => [], staged: false });
const emit = defineEmits<{ dirty: [value: boolean] }>();
const shortcuts = ref(readShortcuts());
const saved = ref({ ...shortcuts.value });
const dirty = computed(() => props.staged && JSON.stringify(shortcuts.value) !== JSON.stringify(saved.value));
watch(dirty, value => emit('dirty', value), { immediate: true });
const resetOpen = ref(false);
const recording = ref<string | null>(null);
const query = ref('');
const error = ref('');
const copy = computed(() => locale.value === 'zh-CN'
  ? { title: '快捷键', search: '搜索操作', reset: '恢复默认', clear: '清除', record: '按下快捷键', suggested: '建议', unavailable: '不可用操作', conflict: '快捷键已被占用', invalid: '请使用修饰键', names: ['搜索操作', '搜索当前列表', '返回', '启动游戏', '打开设置'] }
  : { title: 'Shortcuts', search: 'Search actions', reset: 'Reset defaults', clear: 'Clear', record: 'Press shortcut', suggested: 'Suggested', unavailable: 'Unavailable action', conflict: 'Shortcut already in use', invalid: 'Use a modifier key', names: ['Search actions', 'Search current list', 'Back', 'Launch game', 'Open settings'] });
const entries = computed(() => {
  const defaults = Object.keys(shortcutDefaults).map((id, index) => ({ id, label: copy.value.names[index]!, suggested: '' }));
  const existing = new Set(defaults.map(item => item.id));
  const available = props.commands.filter(command => !existing.has(command.shortcutId ?? command.id)).map(command => ({ id: command.shortcutId ?? command.id, label: command.label, suggested: command.suggestedShortcut ?? '' }));
  for (const entry of available) existing.add(entry.id);
  const unavailable = Object.keys(shortcuts.value).filter(id => !existing.has(id) && shortcuts.value[id] !== null).map(id => ({ id, label: `${copy.value.unavailable}: ${id}`, suggested: '' }));
  return [...defaults, ...available, ...unavailable]
    .filter(entry => entry.label.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase()));
});
function changed() { if (!dirty.value) { shortcuts.value = readShortcuts(); saved.value = { ...shortcuts.value }; } }
window.addEventListener(shortcutChangedEvent, changed);
onUnmounted(() => window.removeEventListener(shortcutChangedEvent, changed));
function capture(event: KeyboardEvent, id: string) {
  event.preventDefault(); event.stopPropagation();
  if (event.key === 'Escape') { recording.value = null; error.value = ''; return; }
  const value = shortcutFromEvent(event);
  if (!value) return;
  const result = props.staged ? shortcutConflict(id, value, shortcuts.value) ? 'conflict' : 'saved' : saveShortcut(id, value);
  if (props.staged && result === 'saved') shortcuts.value = { ...shortcuts.value, [id]: value };
  if (result === 'saved') { recording.value = null; error.value = ''; }
  else error.value = result === 'conflict' ? copy.value.conflict : copy.value.invalid;
}
function reset() { if (props.staged) shortcuts.value = { ...shortcutDefaults }; else resetShortcuts(); recording.value = null; error.value = ''; }
function clear(id: string) { if (props.staged) shortcuts.value = { ...shortcuts.value, [id]: null }; else clearShortcut(id); recording.value = null; error.value = ''; }
function save() {
  if (!replaceShortcuts(shortcuts.value)) { error.value = t('settingsLayout.saveFailed'); return false; }
  saved.value = { ...shortcuts.value }; recording.value = null; return true;
}
function cancel() { shortcuts.value = { ...saved.value }; recording.value = null; error.value = ''; }
defineExpose({ dirty, save, cancel });
</script>
<template>
  <section
    class="shortcut-settings"
    :aria-label="copy.title"
  >
    <input
      v-model="query"
      type="search"
      :aria-label="copy.search"
      :placeholder="copy.search"
    >
    <div
      v-for="entry in entries"
      :key="entry.id"
      class="shortcut-row"
    >
      <span>{{ entry.label }}</span>
      <button
        type="button"
        :aria-label="`${entry.label}: ${recording === entry.id ? copy.record : shortcuts[entry.id] ?? ''}`"
        @click="recording = entry.id; error = ''"
        @keydown="recording === entry.id && capture($event, entry.id)"
      >
        {{ recording === entry.id ? copy.record : (shortcuts[entry.id] ?? '—') }}
      </button>
      <button
        type="button"
        class="shortcut-clear"
        :aria-label="`${copy.clear} ${entry.label}`"
        @click="clear(entry.id)"
      >
        {{ copy.clear }}
      </button>
      <small v-if="!shortcuts[entry.id] && entry.suggested">{{ copy.suggested }}: {{ entry.suggested }}</small>
    </div>
    <p
      v-if="error"
      role="alert"
    >
      {{ error }}
    </p>
    <button
      type="button"
      class="shortcut-reset"
      @click="resetOpen = true"
    >
      {{ copy.reset }}
    </button>
    <ConfirmationDialog
      v-model:open="resetOpen"
      :title="t('settingsLayout.editor.resetShortcutsTitle')"
      :description="staged ? t('settingsLayout.editor.resetDraftDescription') : undefined"
      :confirm-label="t('settingsLayout.editor.restore')"
      @confirm="reset"
    />
  </section>
</template>
<style scoped>
.shortcut-settings{display:grid;gap:8px}.shortcut-settings input{box-sizing:border-box;width:100%;padding:8px;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--ink);font:inherit}.shortcut-row{display:flex;align-items:center;flex-wrap:wrap;gap:8px 16px;min-height:38px}.shortcut-row small{flex-basis:100%;color:var(--muted);text-align:right}.shortcut-row span{flex:1}.shortcut-row button{min-width:110px;padding:7px 10px;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--ink);font:inherit}.shortcut-row .shortcut-clear{min-width:0;border:0;background:transparent;color:var(--muted);cursor:pointer}.shortcut-reset{justify-self:start;padding:7px 0;border:0;background:transparent;color:var(--ink);text-decoration:underline;cursor:pointer}.shortcut-settings p{margin:0;color:var(--danger,#b43131)}
</style>


