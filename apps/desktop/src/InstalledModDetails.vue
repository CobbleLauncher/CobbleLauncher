<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { LocalMod } from './games';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
defineProps<{ mod: LocalMod; busy: boolean }>();
defineEmits<{ back: []; versions: [] }>();
</script>
<template>
  <section class="installed-mod-details">
    <button
      class="secondary-button"
      :disabled="busy"
      @click="$emit('back')"
    >
      <AppIcon name="arrow-left" />{{ t('gameMods.installed') }}
    </button>
    <div class="installed-mod-heading">
      <div>
        <h2>{{ mod.name }}</h2><p class="mod-meta">
          {{ mod.version || t('gameMods.unknownVersion') }} · {{ mod.loader || t('gameMods.unknownLoader') }}<span v-if="!mod.enabled"> · {{ t('gameMods.disabled') }}</span>
        </p>
      </div>
      <button
        class="secondary-button"
        :disabled="busy"
        @click="$emit('versions')"
      >
        {{ t('gameMods.versionsDependencies') }}
      </button>
    </div>
    <p
      v-if="mod.description"
      class="installed-mod-description"
    >
      {{ mod.description }}
    </p>
    <dl>
      <template v-if="mod.authors.length">
        <dt>{{ t('gameMods.authors') }}</dt><dd>{{ mod.authors.join(', ') }}</dd>
      </template>
      <dt>{{ t('gameMods.file') }}</dt><dd>{{ mod.file_name }}</dd>
    </dl>
  </section>
</template>
<style scoped>
.installed-mod-details{margin-top:18px}.installed-mod-details button{display:inline-flex;align-items:center;gap:7px;min-height:var(--ui-control-height,40px);padding:8px 12px}.installed-mod-heading{display:flex;justify-content:space-between;align-items:flex-start;gap:16px;margin:20px 0 14px}.installed-mod-heading h2{font-size:20px;overflow-wrap:anywhere;margin:0 0 8px}.installed-mod-heading button{flex-shrink:0}.installed-mod-description{white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.6}.installed-mod-details dl{display:grid;grid-template-columns:auto minmax(0,1fr);gap:12px 20px;font-size:13px}.installed-mod-details dt{color:var(--muted)}.installed-mod-details dd{margin:0;overflow-wrap:anywhere}@media(max-width:560px){.installed-mod-heading{flex-wrap:wrap}}
</style>

