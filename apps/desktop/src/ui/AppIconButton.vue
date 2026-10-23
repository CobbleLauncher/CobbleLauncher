<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import type { AppIconName } from './icons';
defineProps<{ icon: AppIconName; label: string }>();
</script>
<template>
  <button
    type="button"
    class="ui-icon-button"
    :aria-label="label"
    :title="label"
  >
    <AppIcon :name="icon" />
  </button>
</template>
