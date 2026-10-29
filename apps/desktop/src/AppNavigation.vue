<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import AppIcon from './ui/AppIcon.vue';
import { t } from './i18n';
import type { MainSection } from './navigation';
defineProps<{ active: MainSection; disabled?: boolean; compact?: boolean }>();
defineEmits<{ navigate: [section: MainSection]; settings: [] }>();
const sections: Exclude<MainSection,'settings'>[] = ['library', 'discover-packs', 'tools', 'extensions'];
const keys = { library: 'navigation.games', 'discover-packs': 'navigation.market', mesh: 'mesh.connect', tools: 'navigation.tools', extensions: 'navigation.extensions' } as const;
</script>
<template>
  <nav
    class="main-navigation"
    :class="{ 'main-navigation-compact': compact }"
    :aria-label="t('navigation.label')"
  >
    <div class="main-sections">
      <button
        v-for="section in sections"
        :key="section"
        :aria-current="active === section ? 'page' : undefined"
        :disabled="disabled"
        @click="$emit('navigate', section)"
      >
        {{ t(keys[section]) }}
      </button>
    </div>
    <div
      v-if="!compact"
      class="navigation-account"
    >
      <slot /><button
        class="navigation-settings"
        :aria-current="active === 'settings' ? 'page' : undefined"
        :aria-label="t('navigation.settings')"
        :title="t('navigation.settings')"
        :disabled="disabled"
        @click="$emit('settings')"
      >
        <AppIcon name="settings" />
      </button>
    </div>
  </nav>
</template>
<style scoped>
.main-navigation{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 24px;border-bottom:1px solid var(--line)}.main-sections,.navigation-account{display:flex;align-items:center;gap:4px;min-width:0}.main-sections button{padding:9px 11px;border:0;background:transparent;border-radius:8px;color:var(--muted);white-space:nowrap;font-size:13px}.main-sections button[aria-current=page]{background:var(--hover);color:var(--ink);font-weight:600}.navigation-settings{display:grid;place-items:center;width:34px;height:34px;border:0;border-radius:8px;background:transparent;color:var(--ink)}.navigation-settings:hover{background:var(--hover)}@media(max-width:640px){.main-navigation{padding:6px 12px;gap:6px}.main-sections button{padding:9px 8px}.navigation-account :deep(.offline-badge){display:none}.navigation-account :deep(.account-button){max-width:144px;overflow:hidden}.navigation-account :deep(.account-button>span:not(.profile-avatar):not(.profile-chevron)){overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}
</style>
<style scoped>
.main-navigation-compact{justify-content:center;padding:0;border:0;min-width:0}.main-navigation-compact .main-sections{justify-content:center;gap:12px;width:100%}.main-navigation-compact .main-sections button{flex:1;min-width:0;max-width:80px;font-size:var(--ui-text-label);padding:6px 10px;min-height:36px;border-radius:7px}.main-navigation-compact .main-sections button:hover{background:var(--ui-surface-hover)}@media(max-width:850px){.main-navigation-compact .main-sections{gap:2px}.main-navigation-compact .main-sections button{flex:1 0 auto;max-width:none;padding:6px 4px}}
</style>
