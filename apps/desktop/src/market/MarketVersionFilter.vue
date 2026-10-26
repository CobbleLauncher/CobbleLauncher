<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui';
import MinecraftVersionTiles from '../MinecraftVersionTiles.vue';
import AppIcon from '../ui/AppIcon.vue';
import { readDownloadSettings } from '../download-settings';
import type { GameVersion } from '../games';
import { t } from '../i18n';

const props = defineProps<{ modelValue: string; installedVersions: string[]; disabled?: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [version: string] }>();
const open = ref(false), loading = ref(false), failed = ref(false);
const versions = ref<GameVersion[]>([]);
let loadedSource = '';
const preferred = computed(() => [...new Set([
  versions.value.find(version => version.kind === 'release')?.id ?? '',
  ...props.installedVersions.slice(0, 2), '1.20.1', '1.16.5', '1.12.2',
])].filter(Boolean));

async function load() {
  if (loading.value) return;
  loading.value = true; failed.value = false;
  try {
    const source = await readDownloadSettings();
    const key = JSON.stringify(source);
    if (key !== loadedSource || !versions.value.length) {
      // Reuse the host metadata cache; no catalog fetch on market entry.
      const result = await invoke<GameVersion[]>('list_game_versions', { source, includeAll: true });
      versions.value = result;
      loadedSource = key;
      failed.value = !result.length;
    }
  } catch { failed.value = true; }
  finally { loading.value = false; }
}
watch(open, value => { if (value) void load(); });
watch(() => props.disabled, disabled => { if (disabled) open.value = false; });
function select(value: string) { emit('update:modelValue', value); open.value = false; }
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <button
        type="button"
        class="market-version-trigger"
        :disabled="disabled"
        :aria-label="`${t('createGame.version')}: ${modelValue || t('market.anyVersion')}`"
      >
        <AppIcon name="blocks" /><span>{{ modelValue || t('market.anyVersion') }}</span><AppIcon name="chevron-down" />
      </button>
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        class="market-version-popover"
        align="start"
        :side-offset="8"
        :collision-padding="12"
      >
        <MinecraftVersionTiles
          id="market-version-tiles"
          :model-value="modelValue"
          :versions="versions"
          :preferred-versions="preferred"
          :allow-any="true"
          :loading="loading && !versions.length"
          @update:model-value="select"
        />
        <div
          v-if="failed"
          class="market-version-error"
          role="status"
        >
          <span>{{ t('createGame.versionsFailed') }}</span>
          <button
            type="button"
            class="text-button"
            :disabled="loading"
            @click="load"
          >
            <AppIcon name="refresh-cw" />{{ t('tasks.retryTask') }}
          </button>
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
.market-version-trigger{display:flex;align-items:center;gap:var(--ui-space-2);width:100%;min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label);text-align:start}
.market-version-trigger>span{flex:1;min-width:0;overflow-wrap:anywhere;font-variant-numeric:tabular-nums}.market-version-trigger>.app-icon{width:16px;height:16px;color:var(--ui-text-muted)}.market-version-trigger:hover:not(:disabled){background:var(--ui-surface-hover)}.market-version-trigger:focus-visible{outline:var(--ui-focus-ring);outline-offset:var(--ui-focus-offset)}.market-version-trigger:disabled{opacity:.5;cursor:default}
</style>
<style>
.market-version-popover{z-index:80;width:420px;max-width:calc(100vw - 24px);max-height:min(520px,calc(var(--reka-popover-content-available-height,100vh) - 16px));overflow-y:auto;padding:var(--ui-space-4);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-popup,0 8px 24px #0002)}
.market-version-popover .minecraft-version-browser{position:sticky;top:0;z-index:1;background:var(--ui-surface-1);padding-bottom:var(--ui-space-2)}.market-version-popover .minecraft-version-grid{grid-template-columns:repeat(3,minmax(0,1fr))}.market-version-popover .minecraft-version-tile{min-height:64px;padding:var(--ui-space-2) var(--ui-space-3)}.market-version-popover .minecraft-version-number{font-size:var(--ui-text-label)}.market-version-error{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);margin-top:var(--ui-space-3);font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.market-version-error button{display:inline-flex;gap:var(--ui-space-1);align-items:center;white-space:nowrap}
@media(prefers-reduced-motion:no-preference){.market-version-popover[data-state=open]{animation:market-version-enter 120ms ease-out}@keyframes market-version-enter{from{opacity:0;transform:translateY(-3px)}to{opacity:1;transform:translateY(0)}}}
</style>
