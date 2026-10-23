<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui';
import { t } from './i18n';
import { isSupportedCatalogPlugin, matchesPluginMetadata, type CatalogManifest, type CatalogPlugin, type PluginCatalogView } from './plugin-catalogs';

import PluginIdentity from './PluginIdentity.vue';
import PluginPermissionBadges from './PluginPermissionBadges.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import PluginListPagination from './PluginListPagination.vue';
import { pluginListPageSize, usePluginListPagination } from './plugin-list-pagination';

import type { ExtensionCandidate } from './navigation';
const props = defineProps<{ candidate?: ExtensionCandidate; search?: string; disabled?: boolean; installedManifests?: CatalogManifest[] }>();
const emit = defineEmits<{ prepared: [value: { manifest: CatalogManifest; sourceName: string; updateFrom?: CatalogManifest }]; preparing: [value: boolean] }>();
const catalogs = ref<PluginCatalogView[]>([]);
const visibleCatalogs=computed(()=>props.candidate && props.search===props.candidate.pluginId ? catalogs.value.filter(catalog=>catalog.source.id===props.candidate!.sourceId) : catalogs.value);
const catalogPages = usePluginListPagination(visibleCatalogs, [() => props.search, catalogs]);
const entryPages = ref<Record<string, number>>({});
const name = ref('');
const url = ref('');
const pending = ref<string | null>(null);
const pendingCommand = ref<string | null>(null);
const openCatalogMenu = ref<string | null>(null);
const loading = ref(true);
const error = ref('');

async function load() {
  loading.value = true;
  try { catalogs.value = await invoke<PluginCatalogView[]>('list_plugin_catalogs'); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function update(command: string, args: Record<string, unknown>, key: string) {
  if (pending.value || props.disabled) return;
  pending.value = key; pendingCommand.value = command; error.value = '';
  try { catalogs.value = await invoke<PluginCatalogView[]>(command, args); }
  catch (cause) { error.value = String(cause); }
  finally { pending.value = null; pendingCommand.value = null; }
}
async function add() {
  if (!name.value.trim() || !url.value.trim()) return;
  await update('add_plugin_catalog', { name: name.value.trim(), url: url.value.trim() }, 'add');
  if (!error.value) { name.value = ''; url.value = ''; }
}
async function prepare(source: PluginCatalogView, plugin: CatalogPlugin) {
  if (!source.source.enabled || !isSupportedCatalogPlugin(plugin) || pending.value || props.disabled) return;
  const installed = installedManifest(plugin);
  if (installed && sameBuild(installed, plugin.manifest)) return;
  pending.value = `${source.source.id}:${plugin.manifest.id}`; error.value = '';
  emit('preparing', true);
  try {
    const manifest = await invoke<CatalogManifest>('preview_catalog_plugin', { sourceId: source.source.id, pluginId: plugin.manifest.id });
    emit('prepared', { manifest, sourceName: source.source.name, ...(installed ? { updateFrom: installed } : {}) });
  } catch (cause) { error.value = String(cause); }
  finally { pending.value = null; emit('preparing', false); }
}
function matchingPlugins(catalog: PluginCatalogView) { return (catalog.snapshot?.index.plugins ?? []).filter(plugin => matchesPluginMetadata(plugin.manifest, props.search ?? '', plugin.description)); }
function pagePlugins(catalog: PluginCatalogView) {
  const page = entryPages.value[catalog.source.id] ?? 1;
  const start = (page - 1) * pluginListPageSize;
  return matchingPlugins(catalog).slice(start, start + pluginListPageSize);
}
function entryPageCount(catalog: PluginCatalogView) { return Math.max(1, Math.ceil(matchingPlugins(catalog).length / pluginListPageSize)); }
function setEntryPage(catalog: PluginCatalogView, page: number) { entryPages.value[catalog.source.id] = page; }
watch([() => props.search, catalogs], () => {
  entryPages.value = {};
  for (const catalog of catalogs.value) entryPages.value[catalog.source.id] = 1;
}, { flush: 'sync' });
watch(catalogs, value => {
  for (const catalog of value) {
    const count = entryPageCount(catalog);
    entryPages.value[catalog.source.id] = Math.min(entryPages.value[catalog.source.id] ?? 1, count);
  }
}, { deep: true, flush: 'sync' });
function supported(plugin: CatalogPlugin) {
  return isSupportedCatalogPlugin(plugin);
}
function installedManifest(plugin: CatalogPlugin) {
  return props.installedManifests?.find(manifest => manifest.id === plugin.manifest.id);
}
function manifestIdentity(manifest: CatalogManifest) {
  function sorted(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(sorted);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined).sort(([left], [right]) => left.localeCompare(right)).map(([key, item]) => [key, sorted(item)]));
    return value;
  }
  // These optional fields have the same native meaning when omitted or empty.
  const normalized: Record<string, unknown> = { ...manifest, tags: manifest.tags ?? [], report_protocols: manifest.report_protocols ?? [], provider_origins: manifest.provider_origins ?? [], composer_permissions: manifest.composer_permissions ?? [] };
  for (const key of ['description', 'icon', 'contributions']) if (normalized[key] == null) delete normalized[key];
  return JSON.stringify(sorted(normalized));
}
function sameBuild(installed: CatalogManifest, catalog: CatalogManifest) {
  return manifestIdentity(installed) === manifestIdentity(catalog);
}
function isCurrentBuild(plugin: CatalogPlugin) {
  const installed = installedManifest(plugin);
  return !!installed && sameBuild(installed, plugin.manifest);
}
function catalogActionLabel(plugin: CatalogPlugin) {
  const installed = installedManifest(plugin);
  if (!installed) return t('plugins.installCatalogPlugin');
  return sameBuild(installed, plugin.manifest) ? t('plugins.catalogInstalled') : t('plugins.catalogReviewUpdate');
}
function publisherChange(plugin: CatalogPlugin) {
  const installed = installedManifest(plugin);
  return installed && installed.publisher !== plugin.manifest.publisher
    ? t('plugins.publisherChanged', { old: installed.publisher, current: plugin.manifest.publisher })
    : '';
}
onMounted(() => { void load(); });
</script>

<template>
  <section class="plugin-catalogs">
    <form
      class="catalog-add"
      @submit.prevent="add"
    >
      <div class="form-field">
        <label for="plugin-catalog-name">{{ t('plugins.catalogName') }}</label><input
          id="plugin-catalog-name"
          v-model="name"
          required
          maxlength="120"
          :disabled="props.disabled || !!pending"
        >
      </div>
      <div class="form-field">
        <label for="plugin-catalog-url">{{ t('plugins.catalogUrl') }}</label><input
          id="plugin-catalog-url"
          v-model="url"
          type="url"
          inputmode="url"
          placeholder="https://"
          required
          :disabled="props.disabled || !!pending"
        >
      </div>
      <button
        class="secondary-button"
        type="submit"
        :disabled="props.disabled || !!pending || !name.trim() || !url.trim()"
      >
        {{ t('plugins.subscribeCatalog') }}
      </button>
    </form>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <LoadingIndicator
      v-if="loading && !catalogs.length"
      :label="t('plugins.catalogsLoading')"
      skeleton
    />
    <LoadingIndicator
      v-else-if="pendingCommand === 'refresh_plugin_catalog'"
      :label="t('plugins.catalogRefreshing')"
      compact
    />
    <ul class="catalog-list">
      <li
        v-for="catalog in catalogPages.pageItems.value"
        :key="catalog.source.id"
        class="catalog-source"
      >
        <div class="catalog-heading">
          <div class="catalog-source-info">
            <h3>{{ catalog.source.name }}</h3>
            <span class="catalog-url">{{ catalog.source.url }}</span>
          </div>
          <span
            v-if="!catalog.source.enabled"
            class="catalog-state"
          >{{ t('plugins.catalogDisabled') }}</span>
        </div>
        <div class="catalog-actions">
          <button
            class="mod-toggle catalog-enabled-toggle"
            type="button"
            role="switch"
            :aria-label="t('plugins.enabledState', { name: catalog.source.name })"
            :aria-checked="catalog.source.enabled"
            :disabled="props.disabled || !!pending"
            @click="update('set_plugin_catalog_enabled', { id: catalog.source.id, enabled: !catalog.source.enabled }, catalog.source.id)"
          >
            <span />
          </button>
          <AppIconButton
            icon="refresh-cw"
            :label="t('plugins.refreshCatalogNamed', { name: catalog.source.name })"
            :disabled="props.disabled || !!pending || !catalog.source.enabled"
            @click="update('refresh_plugin_catalog', { id: catalog.source.id }, catalog.source.id)"
          />
          <DropdownMenuRoot
            :open="openCatalogMenu === catalog.source.id"
            @update:open="openCatalogMenu = $event ? catalog.source.id : null"
          >
            <DropdownMenuTrigger as-child>
              <AppIconButton
                icon="more-horizontal"
                :label="t('plugins.moreActions', { name: catalog.source.name })"
                :disabled="props.disabled || !!pending"
                :aria-expanded="openCatalogMenu === catalog.source.id"
              />
            </DropdownMenuTrigger>
            <DropdownMenuPortal>
              <DropdownMenuContent
                class="profile-menu"
                align="end"
                :side-offset="6"
                :collision-padding="12"
              >
                <DropdownMenuItem
                  class="profile-menu-item danger-text"
                  :disabled="props.disabled || !!pending"
                  @select="update('remove_plugin_catalog', { id: catalog.source.id }, catalog.source.id)"
                >
                  <AppIcon name="trash" />{{ t('plugins.removeCatalog') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenuPortal>
          </DropdownMenuRoot>
        </div>
        <p
          v-if="catalog.error"
          class="settings-error"
          role="alert"
        >
          {{ catalog.error }}
        </p>
        <ul
          v-if="catalog.snapshot"
          class="catalog-entries"
        >
          <li
            v-for="plugin in pagePlugins(catalog)"
            :key="plugin.manifest.id"
            class="catalog-entry"
          >
            <div class="catalog-entry-copy">
              <PluginIdentity
                :manifest="plugin.manifest"
                :description-override="plugin.description"
                :icon-data="plugin.icon_png_base64 ? `data:image/png;base64,${plugin.icon_png_base64}` : null"
              />
              <p class="catalog-attribution">
                {{ plugin.manifest.publisher }} · {{ plugin.manifest.version }} · {{ catalog.snapshot.index.name }}
              </p>
              <PluginPermissionBadges :manifest="plugin.manifest" />
              <p
                v-if="publisherChange(plugin)"
                class="catalog-publisher-change"
              >
                {{ publisherChange(plugin) }}
              </p>
              <span
                v-if="!supported(plugin)"
                class="catalog-compatibility catalog-unsupported"
              >{{ t('plugins.catalogUnsupported') }}</span>
            </div>
            <button
              v-if="supported(plugin)"
              class="text-button"
              :disabled="props.disabled || !catalog.source.enabled || !!pending || isCurrentBuild(plugin)"
              @click="prepare(catalog, plugin)"
            >
              {{ pending === `${catalog.source.id}:${plugin.manifest.id}` ? t('plugins.catalogPreparing') : catalogActionLabel(plugin) }}
            </button>
          </li>
        </ul>
        <PluginListPagination
          v-if="catalog.snapshot"
          :page="entryPages[catalog.source.id] ?? 1"
          :page-count="entryPageCount(catalog)"
          :total="matchingPlugins(catalog).length"
          @page="setEntryPage(catalog, $event)"
        />
        <p
          v-else-if="!catalog.error"
          class="catalog-empty"
        >
          {{ t('plugins.catalogNoSnapshot') }}
        </p>
      </li>
    </ul>
    <PluginListPagination
      :page="catalogPages.page.value"
      :page-count="catalogPages.pageCount.value"
      :total="visibleCatalogs.length"
      @page="catalogPages.page.value = $event"
    />
  </section>
</template>

<style scoped>
.plugin-catalogs{margin-top:26px;padding-top:20px;border-top:1px solid var(--line)}
.catalog-add{display:grid;grid-template-columns:minmax(120px,1fr) minmax(200px,2fr) auto;align-items:end;gap:12px}
.catalog-add .form-field{min-width:0;margin:0}
.catalog-list,.catalog-entries{list-style:none;padding:0;margin:14px 0 0}
.catalog-source{min-width:0;padding:14px 0;border-bottom:1px solid var(--line)}
.catalog-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;min-width:0;flex-wrap:wrap}
.catalog-source-info{min-width:0}.catalog-source h3,.catalog-entry h4{margin:0;font-size:13px;font-weight:600;overflow-wrap:anywhere}
.catalog-url{display:block;margin-top:3px;color:var(--muted);font-size:10px;overflow-wrap:anywhere}
.catalog-state,.catalog-compatibility{color:var(--muted);font-size:10px}
.catalog-actions{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap;margin-top:8px}.catalog-enabled-toggle{flex:0 0 32px}
.catalog-entry{display:flex;align-items:flex-start;justify-content:space-between;gap:12px;min-width:0;padding:11px 0 0}
.catalog-entry-copy{min-width:0;overflow-wrap:anywhere}.catalog-entry-copy>p{margin:4px 0 0;font-size:11px;line-height:1.6;color:var(--muted)}
.catalog-attribution{font-size:10px!important}.catalog-unsupported{color:var(--danger,var(--muted))}
.catalog-publisher-change{margin:4px 0 0;color:var(--danger,var(--muted));font-size:10px}
.catalog-empty{margin:9px 0 0;color:var(--muted);font-size:11px}
@media(max-width:620px){.catalog-add{grid-template-columns:minmax(0,1fr)}.catalog-entry{flex-wrap:wrap}.catalog-entry>button{margin-left:auto}}
</style>
