<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import BlueprintsPage from './BlueprintsPage.vue';
import InspectorPanel from './InspectorPanel.vue';
import PluginReportPanel from './PluginReportPanel.vue';
import PluginWebView from './PluginWebView.vue';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import type { PluginFrontendManifest, ToolFormManifest } from './plugin-catalogs';
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
import { recordToolOpen } from './tool-usage';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
const ToolFormPanel = defineAsyncComponent(() => import('./ToolFormPanel.vue'));
type ToolPlugin = { manifest: PluginIdentityManifest & { frontend?: PluginFrontendManifest; tool_form?: ToolFormManifest; api_version?: string; capabilities: string[]; file_extensions: string[]; report_protocols?: string[]; interfaces?:{provides?:{id:string;version:string}[]}; permissions?:{permission:string}[] }; enabled: boolean; problem?: string };
const props = withDefaults(defineProps<{ location?: ToolLocation; selectedPluginId?: string; selectedCommand?: ResolvedCommand; blueprintsOpenRequest?: number; games?: Game[]; initialGameId?: string; active?: boolean; suspended?: boolean }>(), { location: undefined, selectedPluginId: undefined, selectedCommand: undefined, blueprintsOpenRequest: 0, games: () => [], initialGameId: '', active: true, suspended: false });
const emit = defineEmits<{ market: [value: { gameId: string; query: string }]; extensions: [candidate?: ProviderCandidateSelection]; extensionDetails: [pluginId: string]; command: [value: ResolvedCommand]; navigate: [value: ToolLocation] }>();
const selected = ref<string | null>(props.selectedCommand?.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : props.selectedPluginId ?? null);
const commandContext = ref<ResolvedCommand | undefined>(props.selectedCommand);
const plugins = ref<ToolPlugin[]>([]);
const webView = ref<InstanceType<typeof PluginWebView>>();
const reportPanel = ref<InstanceType<typeof PluginReportPanel>>();
const formPanel = ref<{ confirmLeave: () => Promise<boolean>; cancel: () => Promise<void> }>();
const settingsPanel = ref<InstanceType<typeof ExtensionSettingsPanel>>();
const settingsOpen = ref(false);
const inspectingPack = ref(false);
async function selectInspectorMode(pack: boolean) {
  if (inspectingPack.value === pack || !await confirmLeave()) return;
  settingsOpen.value = false;
  inspectingPack.value = pack;
}
const unsupportedFrontend = ref<string>();
const explicitGameContext = computed(() => commandContext.value?.instanceId ?? props.location?.instanceId ?? (props.initialGameId || undefined));
function generalTool(id: string | null) {
  const manifest = plugins.value.find(item => item.manifest.id === id)?.manifest;
  return id === BLUEPRINTS_TOOL_ID || manifest?.capabilities.includes('viewer') || manifest?.interfaces?.provides?.some(item => ['launcher:materials-import', 'launcher:file-inbox'].includes(item.id));
}
function contextForTool(id: string | null) {
  return props.location?.handoffId || generalTool(id) ? explicitGameContext.value : explicitGameContext.value ?? [...props.games].filter(game => game.status === 'installed').sort((a, b) => (b.last_launched_at ?? 0) - (a.last_launched_at ?? 0))[0]?.id;
}
const gameContext = computed(() => contextForTool(selected.value));
const contextGame = computed(() => props.games.find(game => game.id === gameContext.value));
const frontendRevision = ref(0);
const frontendKey = computed(() => `${plugin.value?.manifest.id ?? ''}:${gameContext.value ?? ''}:${props.location?.keybindingTargetId ?? ''}:${frontendRevision.value}`);
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
  if (formPanel.value && !await formPanel.value.confirmLeave()) return false;
  if (webView.value && !await webView.value.confirmLeave()) return false;
  return true;
}
defineExpose({
  confirmLeave,
  reload: () => { frontendRevision.value += 1; },
  suspend: async () => { await webView.value?.suspend(); },
  resume: () => webView.value?.resume(),
});
const contributions = ref<ExtensionContribution[]>([]);
let recordedTool = '';
function recordOpenedTool() {
  const current = plugin.value;
  if (!current || !props.active) return;
  const command = commandContext.value?.pluginId === current.manifest.id ? commandContext.value.commandId
    : contributions.value.find(item => item.pluginId === current.manifest.id)?.commands.find(item => item.menus.some(menu => menu.position === 'tools'))?.id;
  if (!command) return;
  const key = `${frontendKey.value}:${command}`;
  if (recordedTool === key) return;
  recordedTool = key;
  recordToolOpen({ pluginId: current.manifest.id, commandId: command });
}
const loading = ref(false);
const resolvingPluginId = ref('');
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
  const legacy = plugins.value.filter(item => !commandPlugins.has(item.manifest.id) && (isReportPlugin(item.manifest) || item.manifest.capabilities.includes('tool-form')));
  return [...contributed, ...legacy.map(item => ({ manifest: item.manifest, commands: [] }))];
});
let revision = 0;
let commandRevision = 0;
function invalidateCommand() { ++commandRevision; resolvingPluginId.value = ''; }
async function selectTool(id: string) {
  if (!await confirmLeave()) return;
  invalidateCommand();
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
  resolvingPluginId.value = pluginId;
  error.value = '';
  try {
    const command = await invoke<ResolvedCommand>('resolve_extension_command', { pluginId, commandId, position: 'tools', locale: pluginLocale(), ...(contextForTool(pluginId) ? { context: { instanceId: contextForTool(pluginId) } } : {}) });
    if (current !== commandRevision) return;
    commandContext.value = command;
    selected.value = command.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : command.pluginId;
    emit('navigate', { pluginId: selected.value, command, instanceId: gameContext.value, title: command.label });
  } catch { if (current === commandRevision) error.value = t('navigation.loadError'); }
  finally { if (current === commandRevision) resolvingPluginId.value = ''; }
}
function navigateBlueprint(value: { view: NonNullable<ToolLocation['blueprintView']>; title?: string }) {
  const command = value.view !== 'viewer' && commandContext.value?.workflow === 'viewer' && commandContext.value.blueprintId ? undefined : commandContext.value;
  emit('navigate', { pluginId: BLUEPRINTS_TOOL_ID, command, instanceId: gameContext.value, blueprintView: value.view, title: value.title });
}
watch(() => props.location, value => {
  invalidateCommand();
  selected.value = value?.pluginId ?? (props.selectedCommand?.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : props.selectedPluginId ?? null);
  commandContext.value = value?.command ?? props.selectedCommand;
  error.value = '';
  if (value && !value.title && plugin.value) emit('navigate', { ...value, title: value.command?.label ?? plugin.value.manifest.name });
}, { immediate: true });
watch(() => props.selectedPluginId, value => { invalidateCommand(); selected.value = value ?? null; });
watch(() => props.blueprintsOpenRequest, (value, previous) => {
  if (value === previous) return;
  invalidateCommand();
  selected.value = BLUEPRINTS_TOOL_ID;
  commandContext.value = undefined;
  error.value = '';
});
watch(() => props.selectedCommand, value => {
  invalidateCommand();
  commandContext.value = value;
  if (value) selected.value = value.workflow === 'viewer' ? BLUEPRINTS_TOOL_ID : value.pluginId;
});
watch(locale, load);
watch(frontendKey, () => { settingsOpen.value = false; inspectingPack.value = false; });
onMounted(load);
onUnmounted(() => { ++revision; ++commandRevision; });
</script>
<template>
  <div class="page-content tools-page">
    <ToolCatalog
      v-if="!selected"
      :plugins="toolCatalogPlugins"
      :game-id="location?.instanceId ?? initialGameId"
      :pending-plugin-id="resolvingPluginId"
      @blueprints="selectTool(BLUEPRINTS_TOOL_ID)"
      @plugin="selectTool"
      @command="openCommand"
      @extension-details="emit('extensionDetails', $event)"
    >
      <template #actions>
        <button
          class="secondary-button tools-manage-extensions"
          @click="emit('extensions')"
        >
          <AppIcon name="puzzle" />{{ t('tools.manageExtensions') }}
        </button>
      </template>
    </ToolCatalog>
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
    <LoadingIndicator
      v-if="loading && selected"
      :label="t('navigation.loading')"
    />
    <div
      v-if="!loading && selected && selected !== BLUEPRINTS_TOOL_ID && !plugin && !error"
      class="tools-unavailable"
    >
      <p>{{ t('tools.unavailable') }}</p><button
        class="secondary-button"
        @click="emit('extensionDetails', selected)"
      >
        {{ t('tools.manageExtension') }}
      </button>
    </div>
    <div
      v-if="plugin && selected !== BLUEPRINTS_TOOL_ID && (contextGame || (useFrontend && hasSettings))"
      class="tools-workspace-toolbar"
    >
      <span
        v-if="contextGame"
        class="tools-game-context"
      ><AppIcon name="folder" />{{ contextGame.name }} · Minecraft {{ contextGame.version }}</span>
      <button
        v-if="useFrontend && hasSettings"
        class="text-button"
        :aria-expanded="settingsOpen"
        @click="toggleSettings"
      >
        <AppIcon name="settings" />{{ t('shell.page.settings') }}
      </button>
    </div>
    <div
      v-if="plugin && isInspectorPlugin(plugin.manifest) && useFrontend"
      class="inspector-modes"
      role="group"
      :aria-label="plugin.manifest.name"
    >
      <button
        class="secondary-button"
        :aria-pressed="!inspectingPack"
        @click="selectInspectorMode(false)"
      >
        {{ t('tools.dependencyMap') }}
      </button>
      <button
        class="secondary-button"
        :aria-pressed="inspectingPack"
        @click="selectInspectorMode(true)"
      >
        {{ t('tools.packFile') }}
      </button>
    </div>
    <BlueprintsPage
      v-if="selected === BLUEPRINTS_TOOL_ID"
      :key="gameContext"
      :games="games"
      :initial-game-id="commandContext?.instanceId ?? location?.instanceId ?? initialGameId"
      :selected-command="commandContext"
      :view="location?.blueprintView"
      :initial-blueprint-id="location?.blueprintId"
      @navigate="navigateBlueprint"
      @market="emit('market', $event)"
      @extensions="$emit('extensions', $event)"
      @command="$emit('command', $event)"
    />
    <ToolFormPanel
      v-else-if="plugin?.manifest.tool_form && plugin.manifest.capabilities.includes('tool-form')"
      ref="formPanel"
      :key="frontendKey"
      :plugin-id="plugin.manifest.id"
      :form="plugin.manifest.tool_form"
      :context-version="games.find(game => game.id === gameContext)?.version"
      :disabled="suspended || !active"
      @vue:mounted="recordOpenedTool"
    />
    <InspectorPanel
      v-else-if="plugin && isInspectorPlugin(plugin.manifest) && (!useFrontend || inspectingPack)"
      :key="frontendKey"
      :plugin-id="plugin.manifest.id"
      :instance-id="gameContext"
      :allow-compare="plugin.manifest.file_extensions.includes('mrpack')"
      @vue:mounted="recordOpenedTool"
    />
    <div
      v-else-if="plugin && useFrontend"
      class="plugin-workspace"
    >
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
        :handoff-id="location?.handoffId"
        :config-restore-target="location?.configRestoreTarget"
        :active="active"
        :suspended="suspended || settingsOpen"
        @unsupported="unsupportedFrontend = frontendKey"
        @ready="recordOpenedTool"
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
      @vue:mounted="recordOpenedTool"
    />
  </div>
</template>
<style scoped>
.tools-page :deep(.page-content){padding:0}.tools-manage-extensions{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3)}.tools-workspace-toolbar{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);margin:0 0 var(--ui-space-3)}.tools-game-context{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-width:0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);overflow-wrap:anywhere}.tools-workspace-toolbar>.text-button{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3)}.tools-unavailable{display:grid;justify-items:start;gap:var(--ui-space-4);padding:var(--ui-space-5) 0;font-size:var(--ui-text-body);color:var(--ui-text-muted)}.inspector-modes{display:flex;gap:var(--ui-space-2);flex-wrap:wrap;margin-bottom:var(--ui-space-4)}.inspector-modes button[aria-pressed=true]{background:var(--ui-surface-2);border-color:var(--ui-focus);color:var(--ui-text)}
</style>
