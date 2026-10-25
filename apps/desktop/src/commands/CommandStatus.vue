<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import AppIcon from '../ui/AppIcon.vue';
import { locale } from '../i18n';
defineProps<{ status: string; busy?: boolean; shortcut?: string | null; open?: boolean }>();
defineEmits<{ activate: [] }>();
const label = computed(() => locale.value === 'zh-CN' ? '搜索游戏、工具和操作' : 'Search games, tools and actions');
</script>
<template>
  <button
    class="command-status"
    type="button"
    aria-haspopup="dialog"
    :aria-expanded="open"
    :aria-label="label"
    :title="shortcut ? `${label} (${shortcut})` : label"
    @click="$emit('activate')"
  >
    <span class="command-status-current">
      <AppIcon
        :name="busy ? 'loader-circle' : 'search'"
        :class="{ 'ui-busy-icon': busy }"
        :size="16"
      />
      <span>{{ status }}</span>
    </span>
    <span
      class="command-status-search"
      aria-hidden="true"
    >
      <AppIcon
        name="search"
        :size="16"
      /><span>{{ label }}</span>
      <kbd v-if="shortcut">{{ shortcut }}</kbd>
    </span>
  </button>
</template>
<style scoped>
.command-status{position:relative;display:grid;place-items:center;min-width:0;width:100%;min-height:32px;padding:4px 12px;border:1px solid transparent;border-radius:var(--ui-radius-md);background:var(--ui-surface-soft);color:var(--ui-text-muted);font:inherit;font-size:var(--ui-text-caption);line-height:var(--ui-leading-control);overflow:hidden}
.command-status-current,.command-status-search{display:flex;align-items:center;justify-content:center;gap:8px;min-width:0;width:100%;transition:opacity var(--ui-duration-fast) ease,transform var(--ui-duration-fast) ease}
.command-status-current>span,.command-status-search>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.command-status-search{position:absolute;inset:0;padding:0 10px;opacity:0;transform:translateY(4px);pointer-events:none}
.command-status-search kbd{flex-shrink:0;margin-left:auto;border:1px solid var(--ui-border);border-radius:4px;padding:1px 4px;font:inherit;font-size:var(--ui-text-caption)}
.command-status:hover,.command-status:focus-visible,.command-status[aria-expanded=true]{border-color:var(--line);color:var(--ink);background:var(--panel)}
.command-status:hover .command-status-current,.command-status:focus-visible .command-status-current,.command-status[aria-expanded=true] .command-status-current{opacity:0;transform:translateY(-4px)}
.command-status:hover .command-status-search,.command-status:focus-visible .command-status-search,.command-status[aria-expanded=true] .command-status-search{opacity:1;transform:none}
@media(max-width:600px){.command-status-search kbd{display:none}}
@media(prefers-reduced-motion:reduce){.command-status-current,.command-status-search{transition:none}}
:global([data-motion=reduced]) .command-status-current,:global([data-motion=reduced]) .command-status-search{transition:none}
</style>
