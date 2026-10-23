<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import BlueprintsPage from './BlueprintsPage.vue';
import InspectorPanel from './InspectorPanel.vue';
import PluginReportPanel from './PluginReportPanel.vue';
import PluginWebView from './PluginWebView.vue';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import type { PluginFrontendManifest } from './plugin-catalogs';
import { isInspectorPlugin, isReportPlugin } from './plugin-reports';
import { pluginLocale } from './extensions/contributions';
import type { ExtensionContribution, ResolvedCommand } from './extensions/contributions';
import type { Game } from './games';
import ToolCatalog from './ToolCatalog.vue';
import type { ToolCatalogPlugin } from './ToolCatalog.vue';
import type { PluginIdentityManifest } from './PluginIdentity.vue';
import { BLUEPRINTS_TOOL_ID } from './tools';
import type { ToolLocation } from './navigation';
import type { ProviderCandidateSelection } from './online-blueprints';
type ToolPlugin = { manifest: PluginIdentityManifest & { frontend?: PluginFrontendManifest; api_version?: string; capabilities: string[]; file_extensions: string[]; report_protocols?: string[]; permissions?:{permission:string}[] }; enabled: boolean; problem?: string };
const props = withDefaults(defineProps<{ location?: ToolLocation; selectedPluginId?: string; selectedCommand?: ResolvedCommand; blueprintsOpenRequest?: number; games?: Game[]; initialGameId?: string; active?: boolean; suspended?: boolean }>(), { location: undefined, selectedPluginId: undefined, selectedCommand: undefined, blueprintsOpenRequest: 0, games: () => [], initialGameId: '', active: true, suspended: false });
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; extensions: [candidate?: ProviderCandidateSelection]; command: [value: ResolvedCommand]; navigate: [value: ToolLocation] }>();
const selected = ref<string | null>(props.selectedCommand?.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : props.selectedPluginId ?? null);
const commandContext = ref<ResolvedCommand | undefined>(props.selectedCommand);
const plugins = ref<ToolPlugin[]>([]);
const webView = ref<InstanceType<typeof PluginWebView>>();
const reportPanel = ref<InstanceType<typeof PluginReportPanel>>();
const settingsPanel = ref<InstanceType<typeof ExtensionSettingsPanel>>();
const settingsOpen = ref(false);
const unsupportedFrontend = ref<string>();
const gameContext = computed(() => commandContext.value?.instanceId ?? props.location?.instanceId ?? (props.initialGameId || undefined) ?? [...props.games].filter(game => game.status === 'installed').sort((a, b) => (b.last_launched_at ?? 0) - (a.last_launched_at ?? 0))[0]?.id);
const frontendKey = computed(() => `${plugin.value?.manifest.id ?? ''}:${gameContext.value ?? ''}:${props.location?.keybindingTargetId ?? ''}`);
const useFrontend = computed(() => !!plugin.value?.manifest.frontend && unsupportedFrontend.value !== frontendKey.value);
const hasSettings = computed(() => contributions.value.some(item => item.pluginId === selected.value && item.settings.length > 0));
async function toggleSettings() {
  if (settingsOpen.value) {
    if (settingsPanel.value && !await settingsPanel.value.confirmLeave()) return;
    settingsOpen.value = false;
    await webView.value?.resume();
  } else {
    await webView.value?.suspend();
    settingsOpen.value = true;
  }
}
async function confirmLeave() {
  if (settingsPanel.value && !await settingsPanel.value.confirmLeave()) return false;
  if (reportPanel.value && !await reportPanel.value.confirmLeave()) return false;
  await webView.value?.confirmLeave();
  return true;
}
defineExpose({
  confirmLeave,
  suspend: async () => { await webView.value?.suspend(); },
  resume: () => webView.value?.resume(),
});
const contributions = ref<ExtensionContribution[]>([]);
const loading = ref(false);
const error = ref('');
const plugin = computed(() => plugins.value.find(item => item.manifest.id === selected.value));
const toolCatalogPlugins = computed<ToolCatalogPlugin[]>(() => {
  const commandPlugins = new Set(contributions.value.flatMap(item => item.commands.some(command => command.menus.some(menu => menu.position === 'tools')) ? [item.pluginId] : []));
  const contributed = [...commandPlugins].flatMap(pluginId => {
    const installed = plugins.value.find(item => item.manifest.id === pluginId);
    if (!installed) return [];
    const contribution = contributions.value.find(item => item.pluginId === pluginId);
    return [{
      manifest: installed.manifest,
      commands: (contribution?.commands ?? []).filter(command => command.menus.some(menu => menu.position === 'tools')).map(command => ({ id: command.id, label: command.label })),
    }];
  });
  const legacy = plugins.value.filter(item => !commandPlugins.has(item.manifest.id) && isReportPlugin(item.manifest));
  return [...contributed, ...legacy.map(item => ({ manifest: item.manifest, commands: [] }))];
});
let revision = 0;
let commandRevision = 0;
async function selectTool(id: string) {
  if (!await confirmLeave()) return;
  ++commandRevision;
  selected.value = id;
  commandContext.value = undefined;
  error.value = '';
  emit('navigate', { pluginId: id, title: plugins.value.find(item => item.manifest.id === id)?.manifest.name, instanceId: gameContext.value, ...(id === BLUEPRINTS_TOOL_ID ? { blueprintView: 'library' } : {}) });
}
async function load() {
  const current = ++revision; loading.value = true; error.value = '';
  try {
    const [installed, available] = await Promise.all([invoke<ToolPlugin[]>('list_plugins'), invoke<ExtensionContribution[]>('list_extension_contributions', { locale: pluginLocale() })]);
    if (current !== revision) return;
    plugins.value = installed.filter(item => item.enabled && !item.problem);
    contributions.value = available;
    if (plugin.value && props.location && !props.location.title) emit('navigate', { ...props.location, title: commandContext.value?.label ?? plugin.value.manifest.name });
  } catch { if (current === revision) error.value = t('navigation.loadError'); }
  finally { if (current === revision) loading.value = false; }
}
async function openCommand(pluginId: string, commandId: string) {
  if (!await confirmLeave()) return;
  const current = ++commandRevision;
  error.value = '';
  try {
    const command = await invoke<ResolvedCommand>('resolve_extension_command', { pluginId, commandId, position: 'tools', locale: pluginLocale(), ...(gameContext.value ? { context: { instanceId: gameContext.value } } : {}) });
    if (current !== commandRevision) return;
    commandContext.value = command;
    selected.value = command.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : command.pluginId;
    emit('navigate', { pluginId: selected.value, command, instanceId: gameContext.value, title: command.label });
  } catch { if (current === commandRevision) error.value = t('navigation.loadError'); }
}
function navigateBlueprint(value: { view: NonNullable<ToolLocation['blueprintView']>; title?: string }) {
  const command = value.view !== 'viewer' && commandContext.value?.workflow === 'viewer' && commandContext.value.blueprintId ? undefined : commandContext.value;
  emit('navigate', { pluginId: BLUEPRINTS_TOOL_ID, command, instanceId: gameContext.value, blueprintView: value.view, title: value.title });
}
watch(() => props.location, value => {
  ++commandRevision;
  selected.value = value?.pluginId ?? (props.selectedCommand?.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : props.selectedPluginId ?? null);
  commandContext.value = value?.command ?? props.selectedCommand;
  error.value = '';
  if (value && !value.title && plugin.value) emit('navigate', { ...value, title: value.command?.label ?? plugin.value.manifest.name });
}, { immediate: true });
watch(() => props.selectedPluginId, value => { ++commandRevision; selected.value = value ?? null; });
watch(() => props.blueprintsOpenRequest, (value, previous) => {
  if (value === previous) return;
  ++commandRevision;
  selected.value = BLUEPRINTS_TOOL_ID;
  commandContext.value = undefined;
  error.value = '';
});
watch(() => props.selectedCommand, value => {
  ++commandRevision;
  commandContext.value = value;
  if (value) selected.value = value.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : value.pluginId;
});
watch(locale, load);
watch(frontendKey, () => { settingsOpen.value = false; });
onMounted(load);
onUnmounted(() => { ++revision; ++commandRevision; });
</script>
<template>
  <div class="page-content tools-page">
    <ToolCatalog
      v-if="!selected"
      :plugins="toolCatalogPlugins"
      @blueprints="selectTool(BLUEPRINTS_TOOL_ID)"
      @plugin="selectTool"
      @command="openCommand"
    />
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ error }} <button
        class="text-button"
        @click="load"
      >
        {{ t('navigation.retry') }}
      </button>
    </p>
    <p
      v-if="loading && selected"
      role="status"
    >
      {{ t('navigation.loading') }}
    </p>
    <BlueprintsPage
      v-if="selected === BLUEPRINTS_TOOL_ID"
      :key="gameContext"
      :games="games"
      :initial-game-id="gameContext"
      :selected-command="commandContext"
      :view="location?.blueprintView"
      @navigate="navigateBlueprint"
      @market="emit('market', $event)"
      @extensions="$emit('extensions', $event)"
      @command="$emit('command', $event)"
    />
    <InspectorPanel
      v-else-if="plugin && isInspectorPlugin(plugin.manifest)"
      :key="frontendKey"
      :plugin-id="plugin.manifest.id"
      :instance-id="gameContext"
      :allow-compare="plugin.manifest.file_extensions.includes('mrpack')"
    />
    <div
      v-else-if="plugin && useFrontend"
      class="plugin-workspace"
    >
      <button
        v-if="hasSettings"
        class="text-button"
        :aria-expanded="settingsOpen"
        @click="toggleSettings"
      >
        {{ t('shell.page.settings') }}
      </button>
      <ExtensionSettingsPanel
        v-if="settingsOpen"
        ref="settingsPanel"
        :plugin-id="plugin.manifest.id"
        :instance-id="gameContext"
      />
      <PluginWebView
        v-show="!settingsOpen"
        ref="webView"
        :key="frontendKey"
        :plugin-id="plugin.manifest.id"
        :instance-id="location?.keybindingTargetId ? undefined : gameContext"
        :keybinding-target-id="location?.keybindingTargetId"
        :active="active"
        :suspended="suspended || settingsOpen"
        @unsupported="unsupportedFrontend = frontendKey"
      />
    </div>
    <PluginReportPanel
      v-else-if="plugin && isReportPlugin(plugin.manifest)"
      ref="reportPanel"
      :key="frontendKey"
      :plugin-id="plugin.manifest.id"
      :manifest="plugin.manifest"
      :games="games"
      :instance-id="gameContext"
    />
  </div>
</template>
<style scoped>
.tools-page :deep(.page-content){padding:0}
</style>
