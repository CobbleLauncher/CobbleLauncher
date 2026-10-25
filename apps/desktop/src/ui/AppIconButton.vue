<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import type { AppIconName } from './icons';
withDefaults(defineProps<{ icon: AppIconName; label: string; size?: 'standard' | 'compact' }>(), { size: 'standard' });
</script>
<template>
  <button
    type="button"
    class="ui-icon-button"
    :data-size="size"
    :aria-label="label"
    :title="label"
  >
    <AppIcon :name="icon" />
  </button>
</template>
