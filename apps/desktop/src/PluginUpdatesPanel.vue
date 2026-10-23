<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { matchesPluginMetadata, type CatalogManifest, type PluginCatalogView } from './plugin-catalogs';
import { ArrowRight } from '@lucide/vue';
import { addedPermissionNodes } from './plugin-permissions';
import { availablePluginUpdates, batchCompatible, sameUpdateManifest, type PluginUpdate } from './plugin-updates';
import PluginPermissions from './PluginPermissions.vue';
import PluginIdentity from './PluginIdentity.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';
import AppIcon from './ui/AppIcon.vue';
const props = defineProps<{ installed: CatalogManifest[]; disabled?: boolean; search?: string }>();
const emit = defineEmits<{ preparing: [boolean]; prepared: [value: {manifest: CatalogManifest; sourceName: string; updateFrom: CatalogManifest}]; updated: [] }>();
const catalogs = ref<PluginCatalogView[]>([]), bundled = ref<CatalogManifest[]>([]);
const busy = ref(false), error = ref(''), completed = ref(0);
const updates = computed(() => availablePluginUpdates(props.installed, bundled.value, catalogs.value));
const filteredUpdates = computed(() => updates.value.filter(item => matchesPluginMetadata(item.manifest, props.search ?? '')));
const pages = usePluginListPagination(filteredUpdates, [() => props.search, updates]);
// An ID supplied by two sources always requires choosing and reviewing one source.
const batch = computed(() => updates.value.filter(item => batchCompatible(item.manifest, item.previous)
  && updates.value.filter(other => other.manifest.id === item.manifest.id).length === 1));
async function load() {
  const results = await Promise.allSettled([invoke<PluginCatalogView[]>('list_plugin_catalogs'), invoke<CatalogManifest[]>('list_bundled_plugins')]);
  if (results[0].status === 'fulfilled') catalogs.value = results[0].value;
  if (results[1].status === 'fulfilled') bundled.value = results[1].value;
  error.value = results.filter(result => result.status === 'rejected').map(result => String(result.reason)).join('\n');
}
async function preview(item: PluginUpdate) {
  return item.sourceId ? invoke<CatalogManifest>('preview_catalog_plugin', { sourceId: item.sourceId, pluginId: item.manifest.id })
    : invoke<CatalogManifest>('preview_bundled_plugin', { id: item.manifest.id });
}
async function review(item: PluginUpdate) {
  if (busy.value || props.disabled) return;
  busy.value = true; error.value = ''; emit('preparing', true);
  try { emit('prepared', { manifest: await preview(item), sourceName: item.sourceName, updateFrom: item.previous }); }
  catch (cause) { error.value = String(cause); }
  finally { busy.value = false; emit('preparing', false); }
}
async function updateBatch() {
  if (busy.value || props.disabled) return;
  const selected = [...batch.value];
  busy.value = true; error.value = ''; completed.value = 0; emit('preparing', true);
  try {
    for (const item of selected) {
      try {
        const manifest = await preview(item);
        // A refreshed/downloaded package must still match the displayed selection.
        if (!sameUpdateManifest(manifest, item.manifest) || !batchCompatible(manifest, item.previous)) throw new Error('PLUGIN_REVIEW_MISMATCH');
        const signature = await invoke<{ status: string; rootId?: string; publisher?: string }>('get_pending_plugin_signature');
        if (signature.status !== 'verified' || !signature.rootId || signature.publisher !== manifest.publisher) throw new Error(t('plugins.batchSignatureReview'));
        await invoke('update_plugin', { id: manifest.id, reviewedManifest: manifest });
        completed.value++;
      } catch (cause) {
        error.value += `${item.manifest.name}: ${String(cause)}\n`;
        await invoke('cancel_plugin_preview').catch(() => {});
      }
    }
  } finally { busy.value = false; emit('preparing', false); emit('updated'); }
}
onMounted(() => { void load(); });
</script>
<template>
  <section
    v-if="updates.length || error"
    class="plugin-updates"
  >
    <div class="updates-heading">
      <h2>{{ t('plugins.availableUpdates', { count: updates.length }) }}</h2>
      <button
        v-if="batch.length"
        class="secondary-button"
        :disabled="busy || disabled"
        @click="updateBatch"
      >
        <AppIcon name="download" />{{ t('plugins.updateCompatible', { count: batch.length }) }}
      </button>
    </div>
    <ul>
      <li
        v-for="item in pages.pageItems.value"
        :key="`${item.sourceId ?? 'bundled'}:${item.manifest.id}`"
      >
        <div class="update-copy">
          <PluginIdentity
            :manifest="item.manifest"
            :show-description="false"
            :show-tags="false"
            icon-size="small"
          />
          <div class="update-metadata">
            <span class="version-chip old-version">{{ item.previous.version }}</span><ArrowRight
              :size="13"
              aria-hidden="true"
            /><span class="version-chip new-version">{{ item.manifest.version }}</span><span class="update-source">{{ item.sourceName }}</span>
          </div>
          <div
            v-if="addedPermissionNodes(item.manifest, item.previous).length"
            class="update-permissions"
          >
            <PluginPermissions
              :manifest="item.manifest"
              :nodes="addedPermissionNodes(item.manifest, item.previous)"
            />
          </div>
          <p
            v-else-if="!batch.includes(item)"
            class="update-review-note"
          >
            {{ t('plugins.updateIndividualReview') }}
          </p>
        </div>
        <button
          class="secondary-button update-review"
          :disabled="busy || disabled"
          @click="review(item)"
        >
          <AppIcon name="refresh-cw" />{{ t('plugins.catalogReviewUpdate') }}
        </button>
      </li>
    </ul>
    <PluginListPagination
      :page="pages.page.value"
      :page-count="pages.pageCount.value"
      :total="filteredUpdates.length"
      @page="pages.page.value = $event"
    />
    <p
      v-if="completed"
      role="status"
    >
      {{ t('plugins.updatesCompleted', { count: completed }) }}
    </p>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
  </section>
</template>
<style scoped>
.plugin-updates{margin:16px 0;padding:16px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}
.updates-heading,.plugin-updates li{display:flex;justify-content:space-between;gap:12px;align-items:center}.updates-heading{flex-wrap:wrap}.updates-heading h2{font-size:14px;margin:0}.updates-heading button,.update-review{display:inline-flex;align-items:center;justify-content:center;gap:7px;flex:none}
.plugin-updates ul{list-style:none;padding:0;margin:12px 0 0}.plugin-updates li{padding:14px 0;border-top:1px solid var(--line)}.update-copy{min-width:0;flex:1;overflow-wrap:anywhere}.update-metadata{display:flex;align-items:center;gap:7px;flex-wrap:wrap;margin:7px 0 0 40px;color:var(--muted);font-size:11px}.version-chip{padding:2px 6px;border-radius:5px;background:var(--bg);font-variant-numeric:tabular-nums}.new-version{color:var(--ink);font-weight:600}.update-source{overflow-wrap:anywhere}.update-permissions{margin:10px 0 0 40px;padding:8px 10px;border-left:2px solid var(--accent);border-radius:0 6px 6px 0;background:var(--bg)}.update-review-note{margin-left:40px!important}.plugin-updates p{font-size:12px;color:var(--muted);white-space:pre-line;margin:5px 0}.plugin-updates .settings-error{color:var(--danger)}
@media(max-width:620px){.plugin-updates{padding:12px}.plugin-updates li{align-items:flex-start;flex-wrap:wrap}.update-review{margin-left:40px}.update-copy{flex-basis:100%}}
</style>
