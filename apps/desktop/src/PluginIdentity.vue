<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';
import AppIcon from './ui/AppIcon.vue';

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
}>(), { iconData: undefined, descriptionOverride: undefined, fetchIcon: false, bundledIcon: false, showName: true, showDescription: true, showTags: true, iconSize: 'medium' });

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
        name="puzzle"
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
.plugin-identity{display:flex;align-items:flex-start;gap:12px;min-width:0}.plugin-identity-icon{display:grid;place-items:center;flex:none;width:44px;height:44px;border:1px solid var(--line);border-radius:10px;background:var(--panel);color:var(--muted);overflow:hidden}.plugin-identity-icon img{width:100%;height:100%;object-fit:cover}.plugin-identity-icon :deep(svg){width:20px;height:20px}.plugin-identity-copy{min-width:0;overflow-wrap:anywhere}.plugin-identity-copy>strong{display:block;font-size:14px;font-weight:600;line-height:1.4}.plugin-identity-copy>p{margin:4px 0 0;color:var(--muted);font-size:12px;line-height:1.55}.plugin-identity-tags{display:flex;flex-wrap:wrap;gap:5px;list-style:none;padding:0;margin:7px 0 0}.plugin-identity-tags li{padding:2px 7px;border:1px solid var(--line);border-radius:999px;color:var(--muted);font-size:10px;line-height:1.5}.plugin-identity-small .plugin-identity-icon{width:34px;height:34px;border-radius:8px}.plugin-identity-small .plugin-identity-copy>strong{font-size:13px}
</style>
