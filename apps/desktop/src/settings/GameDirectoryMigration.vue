<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { FolderInput, ArrowRight } from '@lucide/vue';
import { locale } from '../i18n';
defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ open: [] }>();
</script>
<template>
  <button
    type="button"
    class="migration-entry"
    :disabled="disabled"
    @click="emit('open')"
  >
    <FolderInput
      :size="18"
      aria-hidden="true"
    /><span>{{ locale === 'zh-CN' ? '搬迁游戏' : 'Move games' }}</span><ArrowRight
      :size="16"
      aria-hidden="true"
    />
  </button>
</template>
<style scoped>
.migration-entry{display:flex;align-items:center;gap:var(--ui-space-3);width:100%;padding:var(--ui-space-4) 0;border:0;border-bottom:1px solid var(--ui-border);background:transparent;color:var(--ui-text);font:inherit;font-size:var(--ui-text-label);font-weight:var(--ui-weight-semibold);text-align:start;cursor:pointer}.migration-entry>svg:first-child{color:var(--ui-text-muted)}.migration-entry>svg:last-child{margin-inline-start:auto;color:var(--ui-text-muted)}.migration-entry:hover:not(:disabled){color:var(--ui-accent)}.migration-entry:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}.migration-entry:disabled{opacity:.5;cursor:default}
</style>
