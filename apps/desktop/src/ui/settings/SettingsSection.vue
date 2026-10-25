<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from '../AppIcon.vue';
import type { AppIconName } from '../icons';
defineProps<{ title?: string; icon?: AppIconName }>();
</script>
<template>
  <section
    class="settings-section"
    :class="{ 'settings-section-titled': title }"
  >
    <div
      v-if="title"
      class="settings-section-heading"
    >
      <AppIcon
        v-if="icon"
        :name="icon"
        class="settings-section-icon"
      />
      <h3
        class="settings-section-title"
      >
        {{ title }}
      </h3>
    </div>
    <slot />
  </section>
</template>
<style scoped>
.settings-section{min-width:0;margin:var(--ui-space-5) 0 var(--ui-space-6);padding:var(--ui-space-3) 0 0;border-top:1px solid var(--ui-border)}
.settings-section-titled{border-top:0;padding-top:0}
.settings-section-heading{display:flex;align-items:center;gap:var(--ui-space-3);padding-bottom:var(--ui-space-3);margin-bottom:var(--ui-space-1);border-bottom:1px solid var(--ui-border-strong)}
.settings-section-icon{width:20px;height:20px;flex:none;color:var(--ui-text-muted)}
.settings-section-title{font-size:var(--ui-text-section);font-weight:var(--ui-weight-semibold);line-height:var(--ui-leading-section);margin:0;overflow-wrap:anywhere}
.settings-section :deep(.settings-row:last-child){border-bottom:0}
.settings-section :deep(.settings-actions){padding-bottom:var(--ui-space-2)}
@media(max-width:600px){.settings-section{margin:var(--ui-space-4) 0 var(--ui-space-5);padding-top:var(--ui-space-2)}.settings-section-titled{padding-top:0}}
</style>
