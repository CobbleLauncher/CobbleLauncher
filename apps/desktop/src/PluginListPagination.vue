<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
defineProps<{ page: number; pageCount: number; total: number }>();
const emit = defineEmits<{ page: [value: number] }>();
</script>

<template>
  <nav
    v-if="pageCount > 1"
    class="plugin-list-pagination"
    :aria-label="t('pluginList.pagination')"
  >
    <button
      class="text-button"
      :disabled="page <= 1"
      @click="emit('page', page - 1)"
    >
      {{ t('pluginList.previous') }}
    </button>
    <span aria-live="polite">{{ t('pluginList.page', { page, pageCount, total }) }}</span>
    <button
      class="text-button"
      :disabled="page >= pageCount"
      @click="emit('page', page + 1)"
    >
      {{ t('pluginList.next') }}
    </button>
  </nav>
</template>

<style scoped>
.plugin-list-pagination{display:flex;align-items:center;justify-content:center;gap:12px;margin:12px 0;color:var(--muted);font-size:var(--ui-text-caption,12px)}
.plugin-list-pagination button{min-height:32px}
</style>
