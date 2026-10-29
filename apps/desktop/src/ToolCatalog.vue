<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import SearchField from './ui/SearchField.vue';
import { ContextMenuRoot, ContextMenuTrigger, ContextMenuPortal, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import PluginIdentity, { type PluginIdentityManifest } from './PluginIdentity.vue';
import PluginListPagination from './PluginListPagination.vue';
import { usePluginListPagination } from './plugin-list-pagination';
import { usePageWorkspace } from './use-page-workspace';
import { navigateListWithArrows } from './list-keyboard';
import { pluginFallbackIcon } from './plugin-presentation';

export type ToolCatalogPlugin = {
  manifest: PluginIdentityManifest & { publisher?: string; capabilities?: string[] };
  commands: { id: string; label: string }[];
};
const props = defineProps<{ plugins: ToolCatalogPlugin[]; gameId?: string; pendingPluginId?: string; loading?: boolean; loadFailed?: boolean }>();
const emit = defineEmits<{
  blueprints: [];
  plugin: [id: string];
  command: [pluginId: string, commandId: string];
  extensionDetails: [pluginId: string];
}>();
const workspace = usePageWorkspace('tools-catalog', props.gameId, () => ({ query: '', page: 1, returnToolId: '', scrollTop: 0, restoreReturn: false }));
const root = ref<HTMLElement>();
function scrollContainer() { return root.value?.closest<HTMLElement>('.page-scroll, .app-content') ?? document.scrollingElement; }
function rememberReturn(id: string) {
  workspace.returnToolId = id;
  workspace.scrollTop = scrollContainer()?.scrollTop ?? 0;
  workspace.restoreReturn = true;
}
function openBlueprints() { rememberReturn('builtin:blueprints'); emit('blueprints'); }
function openPlugin(id: string) { rememberReturn(id); emit('plugin', id); }
function openCommand(pluginId: string, commandId: string) { rememberReturn(pluginId); emit('command', pluginId, commandId); }
function openDetails(pluginId: string) { rememberReturn(pluginId); emit('extensionDetails', pluginId); }
function restoreReturn() {
  if (!workspace.restoreReturn || props.loading) return;
  void nextTick(() => {
    if (!workspace.restoreReturn || props.loading || props.loadFailed) return;
    const action = [...(root.value?.querySelectorAll<HTMLButtonElement>('[data-tool-id]') ?? [])].find(button => button.dataset.toolId === workspace.returnToolId);
    (action ?? root.value?.querySelector<HTMLInputElement>('input[type=search]'))?.focus({ preventScroll: true });
    const scroll = scrollContainer();
    if (scroll) scroll.scrollTop = workspace.scrollTop;
    workspace.restoreReturn = false;
  });
}
watch(() => props.loading, value => { if (!value) restoreReturn(); });
onMounted(restoreReturn);
const query = ref(workspace.query);
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
const pages = usePluginListPagination(filteredEntries, [query]);
pages.page.value = workspace.page;
watch(query, value => { workspace.query = value; });
watch(pages.page, value => { workspace.page = value; });
</script>

<template>
  <section
    ref="root"
    class="tool-catalog"
    :aria-busy="loading || !!pendingPluginId"
    :aria-label="t('tools.title')"
  >
    <div class="tool-catalog-toolbar">
      <SearchField
        v-model="query"
        class="tool-search"
        :label="t('tools.search')"
        :placeholder="t('tools.search')"
        :clear-label="t('tools.clearSearch')"
      />
      <slot name="actions" />
    </div>
    <ul
      class="tool-list"
      @keydown="navigateListWithArrows($event, 'button.tool-card')"
    >
      <ContextMenuRoot
        v-for="entry in pages.pageItems.value"
        :key="entry.kind === 'blueprints' ? 'blueprints' : entry.plugin.manifest.id"
      >
        <ContextMenuTrigger as-child>
          <li :class="`tool-entry-${entry.kind === 'blueprints' ? 'creative' : toolTone(entry.plugin)}`">
            <button
              v-if="entry.kind === 'blueprints'"
              class="tool-card"
              data-tool-id="builtin:blueprints"
              @click="openBlueprints"
            >
              <span
                class="tool-card-icon"
                aria-hidden="true"
              ><AppIcon name="folder" /></span>
              <span class="tool-card-copy"><strong>{{ t('tools.blueprintsName') }}</strong><span>{{ t('tools.blueprintsDescription') }}</span></span>
              <AppIcon
                name="chevron-down"
                class="tool-open-indicator"
              />
            </button>
            <button
              v-else-if="entry.plugin.commands.length <= 1"
              class="tool-card"
              :aria-label="entry.plugin.manifest.name"
              :data-tool-id="entry.plugin.manifest.id"
              :aria-busy="pendingPluginId === entry.plugin.manifest.id"
              :disabled="pendingPluginId === entry.plugin.manifest.id"
              @click="entry.plugin.commands.length ? openCommand(entry.plugin.manifest.id, entry.plugin.commands[0]?.id ?? '') : openPlugin(entry.plugin.manifest.id)"
            >
              <PluginIdentity
                :manifest="entry.plugin.manifest"
                :fallback-icon="pluginFallbackIcon(entry.plugin.manifest)"
                fetch-icon
                :show-tags="false"
              />
              <AppIcon
                v-if="pendingPluginId === entry.plugin.manifest.id"
                name="loader-circle"
                class="ui-busy-icon tool-action-indicator"
              />
              <AppIcon
                v-else
                name="chevron-down"
                class="tool-open-indicator"
              />
            </button>
            <DropdownMenuRoot v-else>
              <DropdownMenuTrigger as-child>
                <button
                  class="tool-card tool-plugin-card"
                  :aria-label="entry.plugin.manifest.name"
                  :data-tool-id="entry.plugin.manifest.id"
                  :aria-busy="pendingPluginId === entry.plugin.manifest.id"
                  :disabled="pendingPluginId === entry.plugin.manifest.id"
                >
                  <PluginIdentity
                    :manifest="entry.plugin.manifest"
                    :fallback-icon="pluginFallbackIcon(entry.plugin.manifest)"
                    fetch-icon
                    :show-tags="false"
                  />
                  <AppIcon
                    :name="pendingPluginId === entry.plugin.manifest.id ? 'loader-circle' : 'chevron-down'"
                    class="tool-action-indicator"
                    :class="{'ui-busy-icon': pendingPluginId === entry.plugin.manifest.id}"
                  />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuPortal>
                <DropdownMenuContent
                  class="profile-menu"
                  align="end"
                  :side-offset="6"
                  :collision-padding="12"
                >
                  <DropdownMenuItem
                    v-for="command in entry.plugin.commands"
                    :key="command.id"
                    class="profile-menu-item"
                    @select="openCommand(entry.plugin.manifest.id, command.id)"
                  >
                    {{ command.label }}
                  </DropdownMenuItem>
                  <DropdownMenuSeparator class="profile-menu-separator" />
                  <DropdownMenuItem
                    class="profile-menu-item"
                    @select="openDetails(entry.plugin.manifest.id)"
                  >
                    <AppIcon name="settings" />{{ t('tools.manageExtension') }}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenuPortal>
            </DropdownMenuRoot>
          </li>
        </ContextMenuTrigger>
        <ContextMenuPortal>
          <ContextMenuContent
            class="profile-menu"
            :collision-padding="12"
          >
            <ContextMenuItem
              v-if="entry.kind === 'blueprints'"
              class="profile-menu-item"
              @select="openBlueprints"
            >
              {{ t('tools.open') }}
            </ContextMenuItem>
            <template v-else>
              <ContextMenuItem
                v-for="command in entry.plugin.commands"
                :key="command.id"
                class="profile-menu-item"
                :disabled="pendingPluginId === entry.plugin.manifest.id"
                @select="openCommand(entry.plugin.manifest.id, command.id)"
              >
                {{ command.label }}
              </ContextMenuItem>
              <ContextMenuItem
                v-if="!entry.plugin.commands.length"
                class="profile-menu-item"
                @select="openPlugin(entry.plugin.manifest.id)"
              >
                {{ t('tools.open') }}
              </ContextMenuItem>
              <ContextMenuSeparator class="profile-menu-separator" />
              <ContextMenuItem
                class="profile-menu-item"
                @select="openDetails(entry.plugin.manifest.id)"
              >
                <AppIcon name="settings" />{{ t('tools.manageExtension') }}
              </ContextMenuItem>
            </template>
          </ContextMenuContent>
        </ContextMenuPortal>
      </ContextMenuRoot>
      <li
        v-if="!loading && !loadFailed && !showBlueprints && !filteredPlugins.length"
        class="tool-empty"
      >
        {{ t('tools.noResults') }}
      </li>
    </ul>
    <PluginListPagination
      v-if="!loading && !loadFailed"
      :page="pages.page.value"
      :page-count="pages.pageCount.value"
      :total="filteredEntries.length"
      @page="pages.page.value = $event"
    />
  </section>
</template>
<style scoped>
.tool-catalog{width:100%;min-width:0}.tool-catalog-toolbar{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-3);padding-bottom:var(--ui-space-4);border-bottom:1px solid var(--ui-border)}.tool-search{flex:1;min-width:220px}.tool-list{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));column-gap:var(--ui-space-5)}.tool-list>li{min-width:0;border-bottom:1px solid var(--ui-border);--tool-tone:var(--ui-text-muted)}.tool-list>.tool-entry-creative{--tool-tone:var(--ui-creative)}.tool-list>.tool-entry-data{--tool-tone:var(--ui-info)}.tool-list>.tool-entry-diagnostic{--tool-tone:var(--ui-warning)}
.tool-card{width:100%;height:100%;display:flex;align-items:center;gap:var(--ui-space-3);min-height:88px;padding:var(--ui-space-4) var(--ui-space-2);border:0;border-radius:var(--ui-radius-sm);background:transparent;color:var(--ui-text);text-align:left;font:inherit;cursor:pointer;transition:background-color var(--ui-duration-fast)}.tool-card:hover:not(:disabled){background:var(--ui-surface-hover)}.tool-card:focus-visible{outline:var(--ui-focus-ring);outline-offset:-2px}.tool-card :deep(.plugin-identity){flex:1;gap:var(--ui-space-3);align-items:center}.tool-card-icon,.tool-card :deep(.plugin-identity-icon){display:grid;place-items:center;flex:none;width:44px;height:44px;border:0;border-radius:var(--ui-radius-md);background:color-mix(in srgb,var(--tool-tone) 10%,transparent);color:var(--tool-tone)}.tool-card-icon :deep(svg){width:var(--ui-icon-md);height:var(--ui-icon-md)}.tool-card-copy{flex:1;display:grid;gap:var(--ui-space-1);min-width:0}.tool-card-copy>strong,.tool-card :deep(.plugin-identity-copy>strong){font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:600}.tool-card-copy>span,.tool-card :deep(.plugin-identity-copy>p){display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;margin:var(--ui-space-1) 0 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.tool-open-indicator{flex:none;transform:rotate(-90deg);color:var(--ui-text-muted)}.tool-action-indicator{flex:none;color:var(--ui-text-muted)}.tool-empty{grid-column:1/-1;padding:var(--ui-space-5) var(--ui-space-2);color:var(--ui-text-muted);font-size:var(--ui-text-body);line-height:var(--ui-leading-body)}
@media(max-width:680px){.tool-list{grid-template-columns:minmax(0,1fr)}.tool-card{padding:var(--ui-space-3) var(--ui-space-1)}}
</style>
