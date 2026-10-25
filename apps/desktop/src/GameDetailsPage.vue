<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { isActiveTask, type TaskSnapshot } from './tasks';
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, TabsRoot, TabsList, TabsTrigger, TabsContent } from 'reka-ui';
import type { Game, GameAction, LocalPackKind, PlayerProfile, ProfileLibrary } from './games';
import { gameLoaderLabel } from './games';
import type { MarketKind } from './market';
import GameModsPanel from './GameModsPanel.vue';
import GameModHistory from './GameModHistory.vue';
import GameImportResult from './GameImportResult.vue';
import type { ModsActivity } from './game-mods';
import GameRuntimeSettings from './GameRuntimeSettings.vue';
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import GameAppearancePanel from './GameAppearancePanel.vue';
import AppIcon from './ui/AppIcon.vue';
import SelectField from './ui/SelectField.vue';
import { isGamePageTab, type GamePageTab } from './game-detail-tabs';
import LocalPacks from './LocalPacks.vue';
import GameWorlds from './GameWorlds.vue';
import type { SnapshotRestoreTarget } from './world-snapshots';
import GameScreenshots from './GameScreenshots.vue';
import { lazyPage } from './lazy-page';
const GameServersPanel = lazyPage(() => import('./GameServersPanel.vue'));
import GameConfigsPanel from './GameConfigsPanel.vue';
import GamePackUpdatePanel from './GamePackUpdatePanel.vue';
import GameDiagnosticsPanel from './GameDiagnosticsPanel.vue';
import LaunchRecoveryPanel from './LaunchRecoveryPanel.vue';
import { t } from './i18n';
import ExtensionMenuItems from './ExtensionMenuItems.vue';
import InstanceExtensionSettings from './InstanceExtensionSettings.vue';
import type { ResolvedCommand } from './extensions/contributions';
import type { RecoveryDetailTarget } from './recovery-navigation';

const props = defineProps<{ tasks?: TaskSnapshot[]; active?: boolean; snapshotRestore?: SnapshotRestoreTarget; recoveryTarget?: RecoveryDetailTarget; game: Game; starting: boolean; profile: PlayerProfile | null; library: ProfileLibrary }>();
const emit = defineEmits<{ play: [id: string]; openGame: [id: string]; retrySessionRecovery: [id: string]; serverPlay: [id: string, serverId: string]; renamed: [game: Game]; profileChanged: [game: Game]; action: [action: GameAction]; contentBusy: [value: boolean]; market: [kind: MarketKind]; extensions: []; mesh: []; readiness: []; login: []; extensionCommand: [command: ResolvedCommand] }>();
const iconUrl = ref<string | null>(null);
watch(() => [props.game.id, props.game.icon_ref] as const, async ([id, iconRef], _previous, onCleanup) => {
  let current = true;
  onCleanup(() => { current = false; });
  iconUrl.value = null;
  if (!iconRef) return;
  try {
    const icons = await invoke<{ id: string; icon_ref: string; data_url: string }[]>('get_game_icons', { ids: [id] });
    const icon = icons.find(icon => icon.id === id && icon.icon_ref === iconRef);
    if (current && icon?.data_url.startsWith('data:image/png;base64,')) iconUrl.value = icon.data_url;
  } catch { /* Preserve the instance preset when a custom icon is unavailable. */ }
}, { immediate: true });
const recoveryBusy = ref(false);
const repairingGame = computed(() => props.tasks?.some(task => task.game_id === props.game.id && task.kind === 'game_repair' && isActiveTask(task)) ?? false);
const historyBusy = ref(false);
const historyOpen = ref(false);
const recoveryHistory = ref<InstanceType<typeof GameModHistory> | null>(null);
const supportsMods = computed(() => !!props.game.loader_version);
const moreOpen = ref(false);
const packUpdate = ref<InstanceType<typeof GamePackUpdatePanel> | null>(null);
const packPreviewBusy = ref(false);
const diagnostics = ref<InstanceType<typeof GameDiagnosticsPanel> | null>(null);
function extensionSelected(command: ResolvedCommand) { moreOpen.value = false; emit('extensionCommand', command); }
const pageTab = ref<GamePageTab>('content');
const screenshotsVisited = ref(false);
const configsVisited = ref(false);
const configBusy = ref(false);
const screenshotBusy=ref(false);
const configSettings = ref<InstanceType<typeof GameConfigsPanel> | null>(null);
const pageTabs = computed<{id: GamePageTab; label: string}[]>(() => [
  {id: 'content', label: t('gameDetails.content')}, {id: 'worlds', label: t('gameDetails.worlds')},
  {id: 'screenshots', label: t('gameDetails.screenshots')},
  {id: 'servers', label: t('gameServers.title')},
  ...(supportsMods.value ? [{id: 'configs' as const, label: t('gameDetails.configs')}] : []),
  {id: 'runtime', label: t('gameDetails.runtime')},
]);
const worldsVisited = ref(false);
const runtimeVisited = ref(false);
const runtimeSettings = ref<InstanceType<typeof GameRuntimeSettings> | null>(null);
const instanceSettings = ref<InstanceType<typeof InstanceExtensionSettings> | null>(null);
function canLeave() { return !contentBusy.value && (runtimeSettings.value?.canLeave() ?? true) && (instanceSettings.value?.canLeave() ?? true) && (configSettings.value?.canLeave() ?? true); }
async function confirmLeave() {
  if (contentBusy.value) return false;
  if (runtimeSettings.value && !await runtimeSettings.value.confirmLeave()) return false;
  if (instanceSettings.value && !await instanceSettings.value.confirmLeave()) return false;
  return await configSettings.value?.confirmLeave() ?? true;
}
async function selectPageTab(value: GamePageTab) {
  if (contentBusy.value) return false;
  if (value === pageTab.value) return true;
  if ((pageTab.value === 'runtime' || pageTab.value === 'configs') && !await confirmLeave()) return false;
  pageTab.value = value; return true;
}
function requestPageTab(value: unknown) { if (isGamePageTab(value)) void selectPageTab(value); }
async function openMods() { if (await selectPageTab('content')) { contentType.value = supportsMods.value ? 'mod' : 'resource_pack'; modsPanel.value?.showInstalled(); } }
async function openContent(kind: MarketKind) {
  if (kind === 'mod') return openMods();
  if ((kind === 'resource_pack' || kind === 'shader_pack') && await selectPageTab('content')) contentType.value = kind;
}
const worldExportBusy = ref(false);
watch(pageTab, (tab) => { if (tab === 'worlds') worldsVisited.value = true; if (tab === 'screenshots') screenshotsVisited.value = true; if (tab === 'configs') configsVisited.value = true; if (tab === 'runtime') runtimeVisited.value = true; });
const profileBindingBusy = ref(false);
const profileBindingError = ref('');
const profileOptions = computed(() => {
  const current = props.library.profiles.find(profile => profile.id === props.library.active_id);
  return [
    { value: '__current__', label: current ? t('gameDetails.currentProfileShort', { name: current.name }) : t('gameDetails.chooseProfile') },
    ...(props.game.profile_id && !props.library.profiles.some(profile => profile.id === props.game.profile_id) ? [{ value: props.game.profile_id, label: t('gameDetails.removedProfile'), disabled: true }] : []),
    ...props.library.profiles.map(profile => ({ value: profile.id, label: t('gameDetails.pinProfile', { name: profile.name, kind: profileKindLabel(profile) }) })),
  ];
});
async function bindProfile(value: string) {
  if (profileBindingBusy.value || locked.value) return;
  const profileId = value === '__current__' ? null : value;
  profileBindingBusy.value = true;
  profileBindingError.value = '';
  try { emit('profileChanged', await invoke<Game>('set_game_profile', { id: props.game.id, profileId })); }
  catch { profileBindingError.value = t('gameDetails.profileError'); }
  finally { profileBindingBusy.value = false; }
}
const editingName = ref(false);
const editedName = ref('');
const nameInput = ref<HTMLInputElement | null>(null);
const actionBusy = ref(false);
const actionError = ref('');
const actionStatus = ref('');
let disposed = false;
async function editName() {
  if (locked.value) return;
  editedName.value = props.game.name;
  actionError.value = '';
  actionStatus.value = '';
  editingName.value = true;
  await nextTick();
  nameInput.value?.focus();
  nameInput.value?.select();
}
async function saveName() {
  if (actionBusy.value || locked.value) return;
  actionError.value = '';
  const name = editedName.value.trim();
  if (!name || name.length > 80) { actionError.value = t('gameDetails.nameInvalid'); return; }
  actionBusy.value = true;
  try {
    const game = await invoke<Game>('rename_game', { id: props.game.id, name });
    if (!disposed) { emit('renamed', game); editingName.value = false; actionStatus.value = t('gameDetails.nameSaved'); }
  } catch (cause) { if (!disposed) actionError.value = t('gameDetails.renameError', { cause: String(cause) }); }
  finally { actionBusy.value = false; }
}
async function openLocation(command: 'open_game_directory' | 'open_game_log') {
  if (actionBusy.value) return;
  actionError.value = '';
  actionStatus.value = '';
  actionBusy.value = true;
  try { await invoke(command, { id: props.game.id }); }
  catch (cause) { if (!disposed) actionError.value = t('gameDetails.openError', { target: command === 'open_game_log' ? t('gameDetails.log') : t('gameDetails.directory'), cause: String(cause) }); }
  finally { actionBusy.value = false; }
}
const contentType = ref<'mod' | LocalPackKind>('mod');
const visitedPacks = ref<LocalPackKind[]>([]);
const packMutationBusy = ref(false);
const contentRevision = ref(0);
watch(contentType, (kind) => { if (kind !== 'mod' && !visitedPacks.value.includes(kind)) visitedPacks.value.push(kind); });
const modsPanel = ref<InstanceType<typeof GameModsPanel> | null>(null);
const modsActivity = ref<ModsActivity>({ busy: false, installing: false, updating: false, recognizing: false });
const contentInstalling = computed(() => modsActivity.value.installing || modsActivity.value.updating || !!props.game.content_installing);
watch(contentInstalling, (active, previous) => { if (previous && !active) contentRevision.value++; });
const contentBusy = computed(() => historyBusy.value || recoveryBusy.value || modsActivity.value.busy || packMutationBusy.value || worldExportBusy.value || configBusy.value || screenshotBusy.value || packPreviewBusy.value);
watch(contentBusy, value => emit('contentBusy', value));
const locked = computed(() => repairingGame.value || historyBusy.value || recoveryBusy.value || screenshotBusy.value || packPreviewBusy.value || contentInstalling.value || packMutationBusy.value || worldExportBusy.value || configBusy.value || props.starting || props.game.launch_state === 'preparing' || props.game.launch_state === 'running' || props.game.launch_state === 'verifying' || props.game.launch_state === 'recovery_failed');
const playDisabled = computed(() => locked.value || modsActivity.value.busy || profileBindingBusy.value);
async function play() { if (playDisabled.value || !await confirmLeave() || playDisabled.value) return; emit('play', props.game.id); }
async function playServer(serverId: string) { if (playDisabled.value || !await confirmLeave() || playDisabled.value) return; emit('serverPlay', props.game.id, serverId); }
async function refreshContent() { contentRevision.value++; await modsPanel.value?.refreshContent(); }
defineExpose({ refreshContent, canLeave, confirmLeave, openMods, openContent, selectPageTab });
watch(() => props.game.id, () => { pageTab.value = 'content'; historyOpen.value = false; runtimeVisited.value = false; worldsVisited.value = false; screenshotsVisited.value = false; configsVisited.value = false; configBusy.value = false; contentType.value = supportsMods.value ? 'mod' : 'resource_pack'; visitedPacks.value = supportsMods.value ? [] : ['resource_pack']; modsActivity.value = {busy: false, installing: false, updating: false, recognizing: false}; }, { immediate: true });
watch(() => props.snapshotRestore, target => { if (target) void selectPageTab('worlds'); }, { immediate: true });
function profileKindLabel(profile: PlayerProfile) { return t(profile.kind === 'microsoft' ? 'profiles.kindMicrosoft' : 'profiles.kindOffline'); }
const playLabel = computed(() => {
  if (props.game.launch_state === 'verifying') return t('gameDetails.sessionVerifying');
  if (props.game.launch_state === 'recovery_failed') return t('gameDetails.retrySessionCheck');
  if (repairingGame.value) return t('tasks.repairGame');
  if (screenshotBusy.value) return t('gameDetails.playAction');
  if (configBusy.value) return t('gameDetails.configBusy');
  if (worldExportBusy.value) return t('gameDetails.worldBusy');
  if (packMutationBusy.value) return t('gameDetails.contentBusy');
  if (modsActivity.value.recognizing) return t('gameDetails.recognizing');
  if (modsActivity.value.updating) return t('gameDetails.updating');
  if (modsActivity.value.installing) return t('gameDetails.installing');
  if (contentInstalling.value) return t('gameDetails.contentBusy');
  if (props.game.launch_state === 'running') return t('gameDetails.running');
  if (locked.value) return t('gameDetails.starting');
  return props.profile ? t('gameDetails.playAction') : t('gameDetails.chooseProfileAction');
});

onUnmounted(() => { disposed = true; });
watch(() => [props.game.id, props.recoveryTarget, props.active] as const, async () => {
  const target = props.recoveryTarget;
  const gameId = props.game.id;
  const current = () => props.game.id === gameId && props.recoveryTarget === target && props.active !== false;
  if (!target || !current()) return;
  if (target.kind === 'runtimeSettings') { await selectPageTab('runtime'); return; }
  await openMods(); historyOpen.value = true; await nextTick();
  if (!current()) return;
  await recoveryHistory.value?.openOperation(target.operationId);
}, { immediate: true, flush: 'post' });
</script>

<template>
  <div class="details-page page-content">
    <header class="details-summary">
      <div class="details-heading">
        <span
          class="details-game-mark"
          :data-preset="game.appearance_preset ?? 'grass'"
          aria-hidden="true"
        >
          <img
            v-if="iconUrl"
            :src="iconUrl"
            alt=""
            @error="iconUrl = null"
          >
          <svg
            v-else
            viewBox="0 0 80 80"
          >
            <path
              class="block-top"
              d="m12 24 28-16 28 16-28 16z"
            />
            <path
              class="block-left"
              d="m12 24 28 16v32L12 56z"
            />
            <path
              class="block-right"
              d="m40 40 28-16v32L40 72z"
            />
          </svg>
        </span>
        <div class="details-title">
          <h1>{{ game.name }}</h1><button
            class="details-icon-button details-rename"
            :aria-label="t('gameDetails.rename')"
            :title="t('gameDetails.rename')"
            :disabled="actionBusy || editingName || contentBusy || locked"
            @click="editName"
          >
            <AppIcon name="pencil" />
          </button>
        </div>
        <p class="game-version">
          Minecraft {{ game.version }} · {{ game.loader_version ? gameLoaderLabel(game) : t('gameDetails.vanilla') }}
        </p>
        <div class="details-metadata">
          <span
            v-if="game.pack_name"
            class="pack-origin"
          >{{ t('gameDetails.modpack', {name: game.pack_name}) }}<span v-if="game.pack_version"> · {{ game.pack_version }}</span></span>
        </div>
      </div>
      <div class="details-primary-actions">
        <div
          v-if="library.profiles.length || game.profile_id"
          class="details-profile-picker"
        >
          <AppIcon name="user" />
          <SelectField
            id="game-header-profile"
            :model-value="game.profile_id || '__current__'"
            :options="profileOptions"
            :label="t('gameDetails.profiles')"
            :disabled="profileBindingBusy || locked"
            @update:model-value="bindProfile"
          />
        </div>
        <button
          class="primary-button play-button"
          :disabled="playDisabled && game.launch_state !== 'recovery_failed'"
          @click="game.launch_state === 'recovery_failed' ? emit('retrySessionRecovery', game.id) : play()"
        >
          <AppIcon :name="game.launch_state === 'recovery_failed' ? 'refresh-cw' : 'play'" />{{ playLabel }}
        </button>
        <button
          class="secondary-button details-mesh"
          :disabled="actionBusy || contentBusy"
          @click="emit('mesh')"
        >
          <AppIcon name="network" />{{ t('mesh.title') }}
        </button>
        <DropdownMenuRoot v-model:open="moreOpen">
          <DropdownMenuTrigger
            class="details-icon-button game-more-trigger"
            :aria-label="t('gameDetails.moreLabel')"
            :title="t('gameDetails.moreLabel')"
            :disabled="actionBusy || editingName || contentBusy || locked"
          >
            <AppIcon name="more-horizontal" />
          </DropdownMenuTrigger><DropdownMenuPortal>
            <DropdownMenuContent
              class="profile-menu details-action-menu"
              align="end"
              :side-offset="6"
              :collision-padding="12"
            >
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="actionBusy"
                @select="openLocation('open_game_directory')"
              >
                <AppIcon name="folder" />{{ t('gameDetails.openDirectory') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="actionBusy"
                @select="emit('readiness')"
              >
                <AppIcon name="shield" />{{ t('gameDetails.offlineReadiness') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="actionBusy || !game.log_path"
                @select="openLocation('open_game_log')"
              >
                <AppIcon name="external-link" />{{ t('gameDetails.openLog') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="!game.log_path || diagnostics?.pending || contentBusy"
                @select="diagnostics?.analyze()"
              >
                <AppIcon name="search" />{{ t(diagnostics?.pending ? 'reports.running' : 'reports.analyzeLogs') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                v-if="packUpdate?.supported"
                class="profile-menu-item"
                :disabled="contentBusy"
                @select="packUpdate?.choose()"
              >
                <AppIcon name="search" />{{ t('packUpdate.action') }}
              </DropdownMenuItem>
              <ExtensionMenuItems
                position="instance"
                :context="{instanceId: game.id}"
                :disabled="contentBusy"
                :revision="contentRevision"
                @selected="extensionSelected"
                @error="actionError = $event"
              />
              <DropdownMenuSeparator class="profile-menu-separator" />
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="locked || modsActivity.busy"
                @select="$emit('action', 'copy')"
              >
                <AppIcon name="copy" />{{ t('gameDetails.copy') }}
              </DropdownMenuItem>
              <DropdownMenuItem
                class="profile-menu-item"
                :disabled="locked || modsActivity.busy"
                @select="$emit('action', 'export')"
              >
                <AppIcon name="download" />{{ t('gameDetails.export') }}
              </DropdownMenuItem>
              <DropdownMenuSeparator class="profile-menu-separator" />
              <DropdownMenuItem
                class="profile-menu-item danger-text"
                :disabled="locked || modsActivity.busy"
                @select="$emit('action', 'remove')"
              >
                <AppIcon name="trash" />{{ t('gameDetails.remove') }}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </div>
      <p
        v-if="profileBindingError"
        class="settings-error details-profile-error"
        role="alert"
      >
        {{ profileBindingError }}
      </p>
      <p
        v-if="game.launch_state === 'verifying' || game.launch_state === 'recovery_failed'"
        class="launch-session-status"
        :role="game.launch_state === 'recovery_failed' ? 'alert' : 'status'"
      >
        {{ game.launch_state === 'verifying' ? t('gameDetails.sessionVerifying') : t('gameDetails.sessionRecoveryFailed') }}
      </p>
    </header>
    <GameImportResult
      :game-id="game.id"
      :status="game.status"
    />
    <form
      v-if="editingName"
      class="rename-form"
      @submit.prevent="saveName"
      @keydown.esc="!actionBusy && (editingName = false)"
    >
      <label for="edited-game-name">{{ t('gameDetails.name') }}</label>
      <div class="rename-controls">
        <input
          id="edited-game-name"
          ref="nameInput"
          v-model="editedName"
          maxlength="80"
          autocomplete="off"
          :disabled="actionBusy"
        >
        <button
          class="secondary-button"
          type="submit"
          :disabled="actionBusy"
        >
          {{ actionBusy ? t('gameDetails.saving') : t('gameDetails.saveName') }}
        </button>
        <button
          class="text-button"
          type="button"
          :disabled="actionBusy"
          @click="editingName = false"
        >
          {{ t('gameDetails.cancel') }}
        </button>
      </div>
    </form>
    <p
      v-if="actionError"
      class="settings-error"
      role="alert"
    >
      {{ actionError }}
    </p>
    <p
      v-if="actionStatus"
      class="setting-hint"
      role="status"
    >
      {{ actionStatus }}
    </p>
    <LaunchRecoveryPanel
      v-if="game.launch_error"
      :game="game"
      :tasks="tasks"
      :disabled="locked"
      @login="emit('login')"
      @runtime="selectPageTab('runtime')"
      @retry="emit('play', game.id)"
      @repaired="emit('renamed', $event)"
      @extensions="emit('extensions')"
      @busy="recoveryBusy = $event"
    />
    <GamePackUpdatePanel
      v-if="game.pack_name"
      :key="`pack-update:${game.id}`"
      ref="packUpdate"
      :game-id="game.id"
      @busy="packPreviewBusy = $event"
      @open-game="emit('openGame', $event)"
    />
    <GameDiagnosticsPanel
      ref="diagnostics"
      :key="game.id"
      external-trigger
      :game-id="game.id"
      :disabled="!game.log_path"
      @extensions="emit('extensions')"
      @runtime="selectPageTab('runtime')"
      @mods="openMods()"
    />
    <TabsRoot
      :model-value="pageTab"
      :unmount-on-hide="false"
      activation-mode="manual"
      @update:model-value="requestPageTab"
    >
      <TabsList
        class="content-tabs game-page-tabs"
        :aria-label="t('gameDetails.tabs')"
      >
        <TabsTrigger
          v-for="tab in pageTabs"
          :key="tab.id"
          :value="tab.id"
          :disabled="contentBusy"
          as-child
        >
          <button
            :id="`game-${tab.id}-tab`"
            :aria-controls="`game-${tab.id}-panel`"
          >
            {{ tab.label }}
          </button>
        </TabsTrigger>
      </TabsList>
      <TabsContent
        v-if="pageTab === 'servers'"
        id="game-servers-panel"
        as="section"
        value="servers"
        role="tabpanel"
        aria-labelledby="game-servers-tab"
      >
        <GameServersPanel
          :game-id="game.id"
          :locked="playDisabled"
          :running="game.launch_state === 'running'"
          @play="playServer"
        />
      </TabsContent>
      <TabsContent
        v-if="screenshotsVisited"
        id="game-screenshots-panel"
        as="section"
        value="screenshots"
        class="screenshots-panel"
        role="tabpanel"
        aria-labelledby="game-screenshots-tab"
      >
        <GameScreenshots
          :key="game.id"
          :game-id="game.id"
          :active="active !== false && pageTab === 'screenshots'"
          @busy="screenshotBusy=$event"
          @extensions="emit('extensions')"
        />
      </TabsContent>
      <TabsContent
        v-if="supportsMods && configsVisited"
        id="game-configs-panel"
        as="section"
        value="configs"
        class="configs-panel"
        role="tabpanel"
        aria-labelledby="game-configs-tab"
      >
        <GameConfigsPanel
          :key="game.id"
          ref="configSettings"
          :game="game"
          :locked="locked || modsActivity.busy"
          @busy="configBusy = $event"
        />
      </TabsContent>
      <TabsContent
        v-if="runtimeVisited"
        id="game-runtime-panel"
        as="section"
        value="runtime"
        role="tabpanel"
        aria-labelledby="game-runtime-tab"
        class="runtime-settings"
      >
        <SettingsSection>
          <SettingsRow
            :label="t('gameDetails.profiles')"
            input-id="game-player-profile"
            :help="!game.profile_id && profile ? t('gameDetails.effectiveProfile', {name: profile.name, kind: profileKindLabel(profile)}) : undefined"
            :error="profileBindingError"
          >
            <SelectField
              id="game-player-profile"
              :model-value="game.profile_id || '__current__'"
              :options="profileOptions"
              :label="t('gameDetails.profiles')"
              :disabled="profileBindingBusy || locked"
              @update:model-value="bindProfile"
            />
          </SettingsRow>
        </SettingsSection>
        <fieldset
          :disabled="locked"
          :inert="locked"
          class="runtime-write-controls"
        >
          <GameAppearancePanel
            :game="game"
            @updated="emit('renamed', $event)"
          />
          <GameRuntimeSettings
            ref="runtimeSettings"
            :game="game"
            @updated="$emit('profileChanged', $event)"
          />
          <InstanceExtensionSettings
            ref="instanceSettings"
            :instance-id="game.id"
          />
        </fieldset>
      </TabsContent>
      <TabsContent
        v-if="worldsVisited"
        id="game-worlds-panel"
        as="section"
        value="worlds"
        role="tabpanel"
        aria-labelledby="game-worlds-tab"
      >
        <GameWorlds
          :tasks="tasks"
          :game-id="game.id"
          :snapshot-restore="snapshotRestore"
          :locked="locked || modsActivity.busy"
          :revision="contentRevision"
          @busy="worldExportBusy = $event"
        />
      </TabsContent>
      <TabsContent
        id="game-content-panel"
        as="section"
        value="content"
        role="tabpanel"
        aria-labelledby="game-content-tab"
        class="content-section"
      >
        <div
          v-if="contentType !== 'mod'"
          class="details-content-toolbar"
        >
          <label
            class="sr-only"
            for="game-pack-content-type"
          >{{ t('gameDetails.contentTypes') }}</label><select
            v-if="supportsMods"
            id="game-pack-content-type"
            v-model="contentType"
            :disabled="contentBusy"
          >
            <option
              v-if="supportsMods"
              value="mod"
            >
              {{ t('gameDetails.mods') }}
            </option><option value="resource_pack">
              {{ t('gameDetails.resourcePacks') }}
            </option><option
              v-if="supportsMods"
              value="shader_pack"
            >
              {{ t('gameDetails.shaderPacks') }}
            </option>
          </select><h2
            v-if="!supportsMods"
            class="details-content-title"
          >
            {{ t('gameDetails.resourcePacks') }}
          </h2>
          <button
            class="secondary-button"
            :disabled="contentBusy || locked"
            @click="emit('market', contentType)"
          >
            <AppIcon name="plus" />{{ t(contentType === 'resource_pack' ? 'gameDetails.addResourcePacks' : 'gameDetails.addShaders') }}
          </button>
        </div>
        <GameModsPanel
          v-if="supportsMods"
          v-show="contentType === 'mod'"
          ref="modsPanel"
          :game="game"
          :locked="locked || packMutationBusy || worldExportBusy"
          :revision="contentRevision"
          @activity="modsActivity = $event"
          @history="historyOpen = !historyOpen"
        >
          <template #content-type>
            <SelectField
              id="game-content-type"
              v-model="contentType"
              :label="t('gameDetails.contentTypes')"
              :options="[{value: 'mod', label: t('gameDetails.mods')}, {value: 'resource_pack', label: t('gameDetails.resourcePacks')}, {value: 'shader_pack', label: t('gameDetails.shaderPacks')}]"
            />
          </template>
          <template #primary-action>
            <button
              class="secondary-button details-add-content"
              :disabled="contentBusy || locked"
              @click="emit('market', 'mod')"
            >
              <AppIcon name="plus" />{{ t('gameDetails.addMods') }}
            </button>
          </template>
          <template #history>
            <GameModHistory
              v-if="historyOpen"
              ref="recoveryHistory"
              :game="game"
              :locked="locked || modsActivity.busy"
              :revision="contentRevision"
              default-open
              @busy="historyBusy = $event"
              @updated="refreshContent"
            />
          </template>
        </GameModsPanel>
        <LocalPacks
          v-if="visitedPacks.includes('resource_pack')"
          v-show="contentType === 'resource_pack'"
          installed-only
          :game-id="game.id"
          kind="resource_pack"
          :locked="locked || modsActivity.busy"
          :revision="contentRevision"
          @busy="packMutationBusy = $event"
        />
        <LocalPacks
          v-if="supportsMods && visitedPacks.includes('shader_pack')"
          v-show="contentType === 'shader_pack'"
          installed-only
          :game-id="game.id"
          kind="shader_pack"
          :locked="locked || modsActivity.busy"
          :revision="contentRevision"
          @busy="packMutationBusy = $event"
        />
      </TabsContent>
    </TabsRoot>
  </div>
</template>
<style scoped>
.details-summary{display:flex;align-items:center;flex-wrap:wrap;gap:var(--ui-space-4);margin-bottom:var(--ui-space-5)}
.details-heading{display:grid;grid-template-columns:auto minmax(0,1fr);column-gap:var(--ui-space-3);align-items:center;flex:1 1 240px;min-width:0}.details-heading>.details-title,.details-heading>.game-version,.details-heading>.details-metadata{grid-column:2}.details-game-mark{grid-column:1;grid-row:1/4;display:grid;place-items:center;width:56px;height:56px;overflow:hidden;border-radius:var(--ui-radius-lg);background:var(--ui-surface-2);--block-top:#b4d889;--block-left:#8ba85e;--block-right:#668244}.details-game-mark svg{width:50px;height:50px;stroke:none}.details-game-mark img{width:100%;height:100%;object-fit:cover;image-rendering:pixelated}.block-top{fill:var(--block-top)}.block-left{fill:var(--block-left)}.block-right{fill:var(--block-right)}.details-game-mark[data-preset=stone]{--block-top:#bfc5c3;--block-left:#969f9e;--block-right:#748080}.details-game-mark[data-preset=dirt]{--block-top:#c4a17c;--block-left:#aa8059;--block-right:#856344}.details-game-mark[data-preset=oak]{--block-top:#d6bd83;--block-left:#b99a61;--block-right:#997d4a}.details-game-mark[data-preset=water]{--block-top:#91c5df;--block-left:#6cabc9;--block-right:#4888ad}.details-game-mark[data-preset=lava]{--block-top:#efb470;--block-left:#dc8f4f;--block-right:#bc6939}.details-game-mark[data-preset=end]{--block-top:#c3b1df;--block-left:#a38abd;--block-right:#82689c}.details-title{display:flex;align-items:center;gap:var(--ui-space-2)}.details-title h1{margin:0;overflow-wrap:anywhere;font-size:var(--ui-text-page);line-height:var(--ui-leading-page);font-weight:600}
.details-icon-button{display:inline-flex;align-items:center;justify-content:center;flex:none;width:var(--ui-control-height);height:var(--ui-control-height);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text-muted)}.details-icon-button:hover{background:var(--ui-surface-hover);color:var(--ui-text)}.details-rename{width:var(--ui-control-compact);height:var(--ui-control-compact);border-color:transparent;background:transparent}
.details-summary .game-version{margin:var(--ui-space-1) 0 0;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.details-metadata{display:flex;flex-wrap:wrap;gap:var(--ui-space-2);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted)}.details-metadata:empty{display:none}.details-metadata .pack-origin{margin:var(--ui-space-1) 0 0;font:inherit;color:inherit}
.details-primary-actions{display:flex;flex:0 1 auto;align-items:center;flex-wrap:wrap;justify-content:flex-end;gap:var(--ui-space-2)}.details-primary-actions .play-button{min-height:var(--ui-launch-height,48px);min-width:112px;padding:0 var(--ui-space-4);display:inline-flex;align-items:center;justify-content:center;gap:var(--ui-space-2);font-size:var(--ui-text-label);font-weight:600}.details-primary-actions .secondary-button{min-height:var(--ui-control-height);display:inline-flex;align-items:center;gap:var(--ui-space-2);padding:0 var(--ui-space-3)}
.details-profile-picker{position:relative;min-width:150px;max-width:240px}.details-profile-picker>.app-icon{position:absolute;z-index:1;left:var(--ui-space-3);top:50%;transform:translateY(-50%);pointer-events:none;color:var(--ui-text-muted)}.details-profile-picker :deep(.select-field-trigger){padding-left:36px}.details-profile-error{flex-basis:100%;margin:0}.launch-session-status{flex-basis:100%;margin:0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.launch-session-status[role=alert]{color:var(--ui-danger)}
.game-page-tabs{display:flex;gap:var(--ui-space-5);overflow-x:auto;white-space:nowrap;margin:0;border-bottom:1px solid var(--ui-border)}.game-page-tabs button{flex:none;min-height:var(--ui-control-height);padding:0 0 var(--ui-space-2);font-size:var(--ui-text-label);line-height:var(--ui-leading-label);border-bottom:2px solid transparent;color:var(--ui-text-muted)}.game-page-tabs button[data-state=active]{color:var(--ui-text);border-bottom-color:var(--ui-focus);font-weight:600}
.details-page :deep(.mods-toolbar),.details-content-toolbar{padding:var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg);background:var(--ui-surface-1)}
.details-page .content-section{margin:0;border:0;padding-top:var(--ui-space-4)}.details-content-toolbar{display:flex;justify-content:space-between;align-items:center;gap:var(--ui-space-3);margin:0 0 var(--ui-space-3)}.details-content-toolbar select{width:auto;min-width:160px;max-width:65%;min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label)}.details-content-title{margin:0;font-size:var(--ui-text-body);font-weight:600}.details-add-content,.details-content-toolbar button{display:inline-flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:0 var(--ui-space-3);font-size:var(--ui-text-label)}
.screenshots-panel,.configs-panel,.runtime-settings{padding-top:var(--ui-space-4)}.runtime-settings{max-width:900px}.rename-form{margin:0 0 var(--ui-space-4)}.runtime-write-controls{border:0;padding:0;margin:0;min-width:0;display:contents}
@media(max-width:900px){.details-heading{flex-basis:100%}.details-primary-actions{width:100%;justify-content:flex-start}.details-profile-picker{flex:1;max-width:280px}.details-primary-actions .play-button{flex:0 0 auto}}
@media(max-width:600px){.details-primary-actions{gap:var(--ui-space-2)}.details-profile-picker{flex:1 1 160px;min-width:0}.details-primary-actions .play-button{flex:1 1 120px}.details-primary-actions .details-mesh{font-size:var(--ui-text-caption)}.game-page-tabs{gap:var(--ui-space-4)}.details-content-toolbar{flex-wrap:wrap}}
</style>
