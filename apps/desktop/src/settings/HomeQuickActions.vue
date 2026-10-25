<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import type { CommandAction } from '../commands/registry';
import SearchField from '../ui/SearchField.vue';
import AppIcon from '../ui/AppIcon.vue';
import { ArrowUp, ArrowDown } from '@lucide/vue';
import { t } from '../i18n';
const props = defineProps<{ modelValue: string[]; commands: CommandAction[] }>();
const emit = defineEmits<{ 'update:modelValue': [value: string[]] }>();
const query = ref('');
const available = computed(() => props.commands.filter(action => action.enabled !== false));
const candidates = computed(() => {
  const value = query.value.trim().toLocaleLowerCase();
  return available.value.filter(action => !value || action.label.toLocaleLowerCase().includes(value) || action.keywords?.some(keyword => keyword.toLocaleLowerCase().includes(value))).sort((a, b) => Number(!!b.quickEntry) - Number(!!a.quickEntry)).slice(0, 20);
});
const unavailable = computed(() => props.modelValue.filter(id => !available.value.some(action => action.id === id)));
function label(id: string) { return props.commands.find(action => action.id === id)?.label ?? t('settingsLayout.homeLayout.unavailableAction'); }
function move(id: string, direction: -1 | 1) {
  const order = [...props.modelValue], index = order.indexOf(id), target = index + direction;
  if (index < 0 || target < 0 || target >= order.length) return;
  [order[index], order[target]] = [order[target]!, order[index]!]; emit('update:modelValue', order);
}
function toggle(id: string) {
  emit('update:modelValue', props.modelValue.includes(id) ? props.modelValue.filter(value => value !== id) : [...props.modelValue, id].slice(0, 6));
}
</script>
<template>
  <div class="home-quick-editor">
    <ol
      v-if="modelValue.length"
      class="home-quick-selected"
    >
      <li
        v-for="(id, index) in modelValue"
        :key="id"
      >
        <span>{{ label(id) }}</span>
        <button
          type="button"
          :disabled="index === 0"
          :aria-label="t('home.moveUp', { name: label(id) })"
          @click="move(id, -1)"
        >
          <ArrowUp :size="16" />
        </button>
        <button
          type="button"
          :disabled="index === modelValue.length - 1"
          :aria-label="t('home.moveDown', { name: label(id) })"
          @click="move(id, 1)"
        >
          <ArrowDown :size="16" />
        </button>
        <button
          type="button"
          :aria-label="t('settingsLayout.homeLayout.removeAction', { name: label(id) })"
          @click="toggle(id)"
        >
          <AppIcon name="x" />
        </button>
      </li>
    </ol>
    <SearchField
      v-model="query"
      :label="t('home.quickSearch')"
      :placeholder="t('home.quickSearch')"
    />
    <div class="home-quick-candidates">
      <label
        v-for="action in candidates"
        :key="action.id"
      >
        <input
          type="checkbox"
          :checked="modelValue.includes(action.id)"
          :disabled="!modelValue.includes(action.id) && modelValue.length >= 6"
          @change="toggle(action.id)"
        >
        <span>{{ action.label }}</span>
      </label>
    </div>
    <button
      v-if="unavailable.length"
      class="text-button"
      @click="$emit('update:modelValue', modelValue.filter(id => !unavailable.includes(id)))"
    >
      {{ t('home.removeUnavailableQuick', { count: unavailable.length }) }}
    </button>
  </div>
</template>
<style scoped>
.home-quick-editor{display:grid;gap:var(--ui-space-3)}.home-quick-candidates{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--ui-space-2) var(--ui-space-5)}.home-quick-candidates label{display:flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}.home-quick-candidates span{overflow-wrap:anywhere}.home-quick-candidates input{flex:none;accent-color:var(--ui-focus)}
.home-quick-selected{list-style:none;margin:0;padding:0}.home-quick-selected li{display:flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding-block:var(--ui-space-1);border-bottom:1px solid var(--ui-border)}.home-quick-selected span{flex:1;min-width:0;font-size:var(--ui-text-label);line-height:var(--ui-leading-label);overflow-wrap:anywhere}.home-quick-selected button{display:grid;place-items:center;flex:none;width:var(--ui-control-compact);height:var(--ui-control-compact);padding:0;border:0;border-radius:var(--ui-radius-sm);background:transparent;color:var(--ui-text-muted)}.home-quick-selected button:hover:not(:disabled){color:var(--ui-text);background:var(--ui-surface-hover)}.home-quick-selected button:disabled{opacity:.3}
@media(max-width:600px){.home-quick-candidates{grid-template-columns:1fr}}
</style>
