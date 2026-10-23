<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { CellPreview } from './types';
import { t } from '../i18n';
defineProps<{ cell: CellPreview | null; label: string }>();
defineEmits<{ details: [] }>();
</script>
<template>
  <template v-if="cell">
    <span class="inspector-cell-text">{{ cell.text }}</span>
    <button
      v-if="!cell.complete"
      class="text-button"
      :aria-label="label"
      @click="$emit('details')"
    >
      {{ t('inspector.details') }}
    </button>
  </template>
</template>
<style scoped>
.inspector-cell-text{white-space:pre-wrap;overflow-wrap:anywhere}button{display:block;margin-top:6px}
</style>
