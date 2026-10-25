<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import type { AppIconName } from './ui/icons';

export type PluginIdentityManifest = {
  id: string;
  name: string;
  description?: string | { en: string; 'zh-CN'?: string };
  tags?: string[];
  icon?: { file: string; sha256: string };
};

const props = withDefaults(defineProps<{
  manifest: PluginIdentityManifest;
  iconData?: string | null;
  descriptionOverride?: string;
  fetchIcon?: boolean;
  bundledIcon?: boolean;
  showName?: boolean;
  showDescription?: boolean;
  showTags?: boolean;
  iconSize?: 'small' | 'medium';
  fallbackIcon?: AppIconName;
}>(), { iconData: undefined, descriptionOverride: undefined, fetchIcon: false, bundledIcon: false, showName: true, showDescription: true, showTags: true, iconSize: 'medium', fallbackIcon: 'puzzle' });

const iconCache = new Map<string, string>();
const loadedIcon = ref<string | null>(null);
let request = 0;
async function loadIcon() {
  const current = ++request;
  loadedIcon.value = null;
  if (props.iconData !== undefined) return;
  if (!props.fetchIcon || !props.manifest.icon) return;
  const cacheKey = `${props.bundledIcon ? 'bundled:' : ''}${props.manifest.id}:${props.manifest.icon.sha256}`;
  const cached = iconCache.get(cacheKey);
  if (cached) { loadedIcon.value = cached; return; }
  try {
    const value = await invoke<string | null>('get_plugin_icon', { id: props.manifest.id, ...(props.bundledIcon ? { bundled: true } : {}) });
    if (current === request) {
      loadedIcon.value = value;
      if (value) iconCache.set(cacheKey, value);
    }
  } catch { if (current === request) loadedIcon.value = null; }
}
const icon = () => props.iconData !== undefined ? props.iconData : loadedIcon.value;
function description() {
  const value = props.manifest.description;
  if (typeof value === 'string') return value;
  return value?.[locale.value === 'zh-CN' ? 'zh-CN' : 'en'] ?? value?.en ?? props.descriptionOverride ?? '';
}
watch(() => [props.manifest.id, props.manifest.icon?.sha256, props.fetchIcon, props.bundledIcon, props.iconData], () => { void loadIcon(); });
onMounted(() => { void loadIcon(); });
</script>

<template>
  <div
    class="plugin-identity"
    :class="`plugin-identity-${iconSize}`"
  >
    <div
      class="plugin-identity-icon"
      aria-hidden="true"
    >
      <img
        v-if="icon()"
        :src="icon()!"
        alt=""
      >
      <AppIcon
        v-else
        :name="fallbackIcon"
      />
    </div>
    <div class="plugin-identity-copy">
      <strong v-if="showName">{{ manifest.name }}</strong>
      <p v-if="showDescription && (description() || descriptionOverride)">
        {{ description() || descriptionOverride }}
      </p>
      <ul
        v-if="showTags && manifest.tags?.length"
        class="plugin-identity-tags"
      >
        <li
          v-for="tag in manifest.tags"
          :key="tag"
        >
          {{ tag }}
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.plugin-identity{display:flex;align-items:flex-start;gap:var(--ui-space-3);min-width:0}.plugin-identity-icon{display:grid;place-items:center;flex:none;width:44px;height:44px;border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text-muted);overflow:hidden}.plugin-identity-icon img{width:100%;height:100%;object-fit:cover}.plugin-identity-icon :deep(svg){width:20px;height:20px}.plugin-identity-copy{min-width:0;overflow-wrap:anywhere}.plugin-identity-copy>strong{display:block;font-size:var(--ui-text-body);font-weight:600;line-height:var(--ui-leading-body)}.plugin-identity-copy>p{margin:var(--ui-space-1) 0 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-identity-tags{display:flex;flex-wrap:wrap;gap:var(--ui-space-1);list-style:none;padding:0;margin:var(--ui-space-2) 0 0}.plugin-identity-tags li{padding:0 var(--ui-space-2);border:1px solid var(--ui-border);border-radius:var(--ui-radius-sm);color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.plugin-identity-small .plugin-identity-icon{width:34px;height:34px;border-radius:8px}.plugin-identity-small .plugin-identity-copy>strong{font-size:var(--ui-text-label)}
</style>
