<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import PluginIdentity, { type PluginIdentityManifest } from './PluginIdentity.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';

export type ToolCatalogPlugin = {
  manifest: PluginIdentityManifest & { publisher?: string; capabilities?: string[] };
  commands: { id: string; label: string }[];
};
const props = defineProps<{ plugins: ToolCatalogPlugin[] }>();
const emit = defineEmits<{
  blueprints: [];
  plugin: [id: string];
  command: [pluginId: string, commandId: string];
}>();
const query = ref('');
function toolTone(plugin: ToolCatalogPlugin) {
  const metadata = [...(plugin.manifest.tags ?? []), ...(plugin.manifest.capabilities ?? []), plugin.manifest.id].join(' ').toLowerCase();
  if (/blueprint|schematic|mesh|creative|skin|screenshot/.test(metadata)) return 'creative';
  if (/diagnos|crash|doctor|config/.test(metadata)) return 'diagnostic';
  if (/world|backup|storage|data|pack|mod|server/.test(metadata)) return 'data';
  return 'runtime';
}
const searchable = (values: (string | undefined)[]) => values.filter(Boolean).join(' ').toLocaleLowerCase(locale.value);
function matchesSearch(values: (string | undefined)[], value: string) {
  const haystack = searchable(values);
  return value.trim().toLocaleLowerCase(locale.value).split(/\s+/).filter(Boolean).every(term => haystack.includes(term));
}
function description(manifest: PluginIdentityManifest) {
  const value = manifest.description;
  return typeof value === 'string' ? value : value?.[locale.value === 'zh-CN' ? 'zh-CN' : 'en'] ?? value?.en;
}
const filteredPlugins = computed(() => {
  return props.plugins.filter(plugin => matchesSearch([
    plugin.manifest.name, description(plugin.manifest), plugin.manifest.id, plugin.manifest.publisher,
    ...(plugin.manifest.tags ?? []), ...plugin.commands.map(command => command.label),
  ], query.value));
});
const showBlueprints = computed(() => matchesSearch([
  t('tools.blueprintsName'), t('tools.blueprintsDescription'), ...t('tools.blueprintsTags').split('|'),
], query.value));
const filteredEntries = computed(() => [
  ...(showBlueprints.value ? [{ kind: 'blueprints' as const }] : []),
  ...filteredPlugins.value.map(plugin => ({ kind: 'plugin' as const, plugin })),
]);
const pages = usePluginListPagination(filteredEntries, [query, () => props.plugins]);
</script>

<template>
  <section
    class="tool-catalog"
    :aria-label="t('tools.title')"
  >
    <label class="tool-search">
      <AppIcon name="search" />
      <span class="visually-hidden">{{ t('tools.search') }}</span>
      <input
        v-model="query"
        type="search"
        :placeholder="t('tools.search')"
      >
      <AppIconButton
        v-if="query"
        class="tool-search-clear"
        icon="x"
        :label="t('tools.clearSearch')"
        @click="query = ''"
      />
    </label>
    <ul class="tool-list">
      <li
        v-for="entry in pages.pageItems.value"
        :key="entry.kind === 'blueprints' ? 'blueprints' : entry.plugin.manifest.id"
        :class="`tool-entry-${entry.kind === 'blueprints' ? 'creative' : toolTone(entry.plugin)}`"
      >
        <button
          v-if="entry.kind === 'blueprints'"
          class="tool-card"
          @click="emit('blueprints')"
        >
          <div
            class="tool-card-icon"
            aria-hidden="true"
          >
            <AppIcon name="folder" />
          </div>
          <div class="tool-card-copy">
            <strong>{{ t('tools.blueprintsName') }}</strong>
            <p>{{ t('tools.blueprintsDescription') }}</p>
            <ul
              class="tool-tags"
              :aria-label="t('tools.tags')"
            >
              <li
                v-for="tag in t('tools.blueprintsTags').split('|')"
                :key="tag"
              >
                {{ tag }}
              </li>
            </ul>
          </div>
        </button>
        <template v-else>
          <button
            v-if="entry.plugin.commands.length <= 1"
            class="tool-card"
            :aria-label="entry.plugin.manifest.name"
            @click="entry.plugin.commands.length ? emit('command', entry.plugin.manifest.id, entry.plugin.commands[0]?.id ?? '') : emit('plugin', entry.plugin.manifest.id)"
          >
            <PluginIdentity
              :manifest="entry.plugin.manifest"
              fetch-icon
            />
          </button>
          <div
            v-else
            class="tool-card tool-plugin-card"
          >
            <PluginIdentity
              :manifest="entry.plugin.manifest"
              fetch-icon
            />
            <div class="tool-actions">
              <button
                v-for="command in entry.plugin.commands"
                :key="command.id"
                class="tool-action"
                @click="emit('command', entry.plugin.manifest.id, command.id)"
              >
                {{ command.label }}
              </button>
            </div>
          </div>
        </template>
      </li>
      <li
        v-if="!showBlueprints && !filteredPlugins.length"
        class="tool-empty"
      >
        {{ t('tools.noResults') }}
      </li>
    </ul>
    <PluginListPagination
      :page="pages.page.value"
      :page-count="pages.pageCount.value"
      :total="filteredEntries.length"
      @page="pages.page.value = $event"
    />
  </section>
</template>

<style scoped>
.tool-catalog{width:100%;margin:0}.tool-search{display:flex;align-items:center;gap:10px;min-height:var(--ui-control-height);padding:0 11px;border:1px solid var(--line);border-radius:var(--ui-radius-md);background:var(--panel);color:var(--muted)}.tool-search>svg{width:17px;height:17px;flex:none}.tool-search input{flex:1;min-width:0;border:0;outline:0;background:transparent;color:var(--ink);font:inherit;font-size:14px}.tool-search input:focus-visible{outline:0}.tool-search input::-webkit-search-decoration,.tool-search input::-webkit-search-cancel-button{appearance:none}.tool-search:focus-within{border-color:var(--accent);box-shadow:0 0 0 2px color-mix(in srgb,var(--accent) 20%,transparent)}.tool-search-clear{min-height:28px;width:28px}
.tool-list{list-style:none;padding:0;margin:14px 0 0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));column-gap:28px;row-gap:0}.tool-list>li{min-width:0;border-bottom:1px solid var(--line);--tool-tone:var(--muted)}.tool-list>.tool-entry-creative{--tool-tone:var(--ui-creative,#8b6bc1)}.tool-list>.tool-entry-data{--tool-tone:var(--ui-info,#4f8ab5)}.tool-list>.tool-entry-diagnostic{--tool-tone:var(--ui-warning,#b58b37)}
.tool-card{width:100%;min-height:100%;display:flex;align-items:flex-start;gap:14px;padding:20px 8px;border:0;border-radius:0;background:transparent;color:var(--ink);text-align:left;transition:background-color .14s ease}.tool-card:hover{background:color-mix(in srgb,var(--tool-tone) 6%,transparent)}button.tool-card{font:inherit;cursor:pointer}.tool-card:focus-visible{outline:2px solid var(--tool-tone);outline-offset:-2px}
.tool-card-icon,.tool-card :deep(.plugin-identity-icon){display:grid;place-items:center;flex:none;width:46px;height:46px;border:0;border-radius:var(--ui-radius-md);background:color-mix(in srgb,var(--tool-tone) 13%,transparent);color:var(--tool-tone)}.tool-card-icon :deep(svg){width:23px;height:23px}.tool-card-copy{min-width:0;overflow-wrap:anywhere}.tool-card-copy>strong,.tool-card :deep(.plugin-identity-copy>strong){font-size:16px;font-weight:650;line-height:1.4}.tool-card-copy>p,.tool-card :deep(.plugin-identity-copy>p){margin:5px 0 0;color:var(--muted);font-size:14px;line-height:1.5}.tool-card :deep(.plugin-identity){gap:14px}.tool-tags,.tool-card :deep(.plugin-identity-tags){display:flex;flex-wrap:wrap;gap:5px 10px;list-style:none;padding:0;margin:9px 0 0}.tool-tags li,.tool-card :deep(.plugin-identity-tags li){padding:0;border:0;border-radius:0;background:transparent;color:var(--tool-tone);font-size:12px;line-height:1.5}.tool-card :deep(.plugin-identity-tags li:nth-child(n+4)){display:none}
.tool-plugin-card{display:block}.tool-actions{display:flex;flex-wrap:wrap;gap:8px;margin:12px 0 0 60px}.tool-action{min-height:32px;padding:5px 10px;border:0;border-radius:var(--ui-radius-sm);background:color-mix(in srgb,var(--tool-tone) 9%,transparent);color:var(--ink);font:inherit;font-size:14px;cursor:pointer}.tool-action:hover{background:color-mix(in srgb,var(--tool-tone) 17%,transparent)}.tool-empty{grid-column:1/-1;padding:24px 8px;color:var(--muted);font-size:14px}.visually-hidden{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}@media(max-width:620px){.tool-list{grid-template-columns:minmax(0,1fr)}.tool-card{padding:18px 4px}.tool-actions{margin-left:60px}}
</style>