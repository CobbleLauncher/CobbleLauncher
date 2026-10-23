<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { matchesPluginMetadata, type CatalogManifest } from './plugin-catalogs';
import PluginIdentity from './PluginIdentity.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';

const props = defineProps<{ disabled?: boolean; search?: string; installedIds: string[]; installedManifests?: CatalogManifest[] }>();
const emit = defineEmits<{ prepared: [value: { manifest: CatalogManifest; sourceName: string; updateFrom?: CatalogManifest }]; preparing: [value: boolean] }>();
const plugins = ref<CatalogManifest[]>([]);
const available = computed(() => plugins.value);
const filtered = computed(() => available.value.filter(plugin => matchesPluginMetadata(plugin, props.search ?? '')));
const pages = usePluginListPagination(filtered, [() => props.search, plugins]);
const pending = ref<string | null>(null);
const error = ref('');
async function load() {
  error.value = '';
  try { plugins.value = await invoke<CatalogManifest[]>('list_bundled_plugins'); }
  catch (cause) { error.value = String(cause); }
}
async function prepare(id: string) {
  if (pending.value || props.disabled) return;
  pending.value = id; error.value = ''; emit('preparing', true);
  try {
    const manifest = await invoke<CatalogManifest>('preview_bundled_plugin', { id });
    const installed = props.installedManifests?.find(plugin => plugin.id === id);
    emit('prepared', { manifest, sourceName: t('plugins.bundled'), ...(installed ? { updateFrom: installed } : {}) });
  } catch (cause) { error.value = String(cause); }
  finally { pending.value = null; emit('preparing', false); }
}
onMounted(() => { void load(); });
watch(() => [props.installedIds, props.installedManifests], () => { void load(); }, { deep: true });
</script>
<template>
  <section
    v-if="available.length || error"
    class="bundled-plugins"
  >
    <h2>{{ t('plugins.bundled') }}</h2>
    <ul>
      <li
        v-for="plugin in pages.pageItems.value"
        :key="plugin.id"
      >
        <PluginIdentity
          :manifest="plugin"
          fetch-icon
          bundled-icon
        />
        <button
          class="text-button"
          :disabled="disabled || !!pending"
          @click="prepare(plugin.id)"
        >
          {{ pending === plugin.id ? t('plugins.catalogPreparing') : t(installedIds.includes(plugin.id) ? 'plugins.catalogReviewUpdate' : 'plugins.installCatalogPlugin') }}
        </button>
      </li>
    </ul>
    <PluginListPagination
      :page="pages.page.value"
      :page-count="pages.pageCount.value"
      :total="filtered.length"
      @page="pages.page.value = $event"
    />
    <p
      v-if="search?.trim() && !filtered.length"
      class="setting-hint"
      role="status"
    >
      {{ t('plugins.noMatches') }}
    </p>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <button
      v-if="error"
      class="text-button"
      :disabled="disabled || !!pending"
      @click="load"
    >
      {{ t('plugins.retry') }}
    </button>
  </section>
</template>
<style scoped>
.bundled-plugins{margin-top:20px}.bundled-plugins h2{font-size:14px}.bundled-plugins ul{list-style:none;padding:0}.bundled-plugins li{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 0}.bundled-plugins li>:first-child{overflow-wrap:anywhere;min-width:0}
</style>
