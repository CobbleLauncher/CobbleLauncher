<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './AppIcon.vue';

withDefaults(defineProps<{ label: string; compact?: boolean; skeleton?: boolean }>(), { compact: false, skeleton: false });
</script>

<template>
  <div
    v-if="skeleton"
    class="loading-skeleton"
    role="status"
    aria-live="polite"
    aria-atomic="true"
  >
    <span class="visually-hidden">{{ label }}</span>
    <span
      v-for="line in 3"
      :key="line"
      class="skeleton-row"
      aria-hidden="true"
    >
      <i /><span><i /><i /></span>
    </span>
  </div>
  <div
    v-else
    class="loading-indicator"
    :class="{ compact }"
    role="status"
    aria-live="polite"
    aria-atomic="true"
  >
    <AppIcon
      name="loader-circle"
      class="ui-busy-icon"
    />
    <span>{{ label }}</span>
  </div>
</template>

<style scoped>
.loading-indicator{display:flex;align-items:center;gap:8px;min-height:42px;color:var(--muted);font-size:12px;line-height:1.5}
.loading-indicator :deep(svg){width:16px;height:16px}
.loading-indicator.compact{min-height:28px;gap:6px;font-size:11px}
.loading-indicator.compact :deep(svg){width:14px;height:14px}
.loading-skeleton{display:grid;gap:8px;padding:10px 0}
.skeleton-row{display:grid;grid-template-columns:36px minmax(0,1fr);align-items:center;gap:11px;min-height:48px;padding:8px 11px;border:1px solid var(--line);border-radius:9px;background:var(--panel)}
.skeleton-row>i{width:32px;height:32px;border-radius:8px;background:var(--hover);animation:loading-pulse 1.4s ease-in-out infinite alternate}
.skeleton-row>span{display:grid;gap:7px;max-width:70%}
.skeleton-row>span>i{height:7px;border-radius:6px;background:var(--hover);animation:loading-pulse 1.4s ease-in-out infinite alternate}
.skeleton-row>span>i:last-child{width:72%}
@keyframes loading-pulse{from{opacity:.42}to{opacity:.76}}
.visually-hidden{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
</style>
