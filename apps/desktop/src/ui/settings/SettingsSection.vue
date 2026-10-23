<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
defineProps<{ title?: string }>();
</script>
<template>
  <section class="settings-section">
    <h3
      v-if="title"
      class="settings-section-title"
    >
      {{ title }}
    </h3>
    <slot />
  </section>
</template>
<style scoped>
.settings-section{min-width:0;margin:var(--ui-space-5,24px) 0 var(--ui-space-6,32px);padding:var(--ui-space-3,12px) 0 0;border-top:1px solid var(--ui-border)}
.settings-section-title{font-size:var(--ui-text-section,18px);font-weight:var(--ui-weight-semibold,600);line-height:var(--ui-leading-heading,1.3);margin:0;padding:0 0 var(--ui-space-2,8px);overflow-wrap:anywhere}
.settings-section :deep(.settings-row:last-child){border-bottom:0}
.settings-section :deep(.settings-actions){padding-bottom:var(--ui-space-2,8px)}
@media(max-width:600px){.settings-section{margin:var(--ui-space-4,16px) 0 var(--ui-space-5,24px);padding-top:var(--ui-space-2,8px)}}
</style>
