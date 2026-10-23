<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { appIcons, type AppIconName } from './icons';
withDefaults(defineProps<{ name: AppIconName; size?: number }>(), { size: 16 });
</script>
<template>
  <component
    :is="appIcons[name]"
    class="app-icon"
    :size="size"
    :stroke-width="1.75"
    aria-hidden="true"
    focusable="false"
  />
</template>
