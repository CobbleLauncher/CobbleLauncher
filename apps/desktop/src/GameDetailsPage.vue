<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { isActiveTask, type TaskSnapshot } from './tasks';
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import type { Game, GameAction, LocalPackKind, PlayerProfile, ProfileLibrary } from './games';
import { gameLoaderLabel } from './games';
import type { MarketKind } from './market';
import GameModsPanel from './GameModsPanel.vue';
import GameModHistory from './GameModHistory.vue';
import type { ModsActivity } from './game-mods';
import GameRuntimeSettings from './GameRuntimeSettings.vue';
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import GameAppearancePanel from './GameAppearancePanel.vue';
import AppIcon from './ui/AppIcon.vue';
import LocalPacks from './LocalPacks.vue';
import GameWorlds from './GameWorlds.vue';
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

const props = defineProps<{ tasks?: TaskSnapshot[]; active?: boolean; game: Game; starting: boolean; profile: PlayerProfile | null; library: ProfileLibrary }>();
const emit = defineEmits<{ play: [id: string]; serverPlay: [id: string, serverId: string]; renamed: [game: Game]; profileChanged: [game: Game]; action: [action: GameAction]; contentBusy: [value: boolean]; market: [kind: MarketKind]; extensions: []; mesh: []; login: []; extensionCommand: [command: ResolvedCommand] }>();
const recoveryBusy = ref(false);
const repairingGame = computed(() => props.tasks?.some(task => task.game_id === props.game.id && task.kind === 'game_repair' && isActiveTask(task)) ?? false);
const historyBusy = ref(false);
const supportsMods = computed(() => !!props.game.loader_version);
const moreOpen = ref(false);
const packUpdate = ref<InstanceType<typeof GamePackUpdatePanel> | null>(null);
const packPreviewBusy = ref(false);
const diagnostics = ref<InstanceType<typeof GameDiagnosticsPanel> | null>(null);
function extensionSelected(command: ResolvedCommand) { moreOpen.value = false; emit('extensionCommand', command); }
type GamePageTab = 'content' | 'runtime' | 'worlds' | 'screenshots' | 'configs' | 'servers';
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
async function openMods() { if (await selectPageTab('content')) { contentType.value = supportsMods.value ? 'mod' : 'resource_pack'; modsPanel.value?.showInstalled(); } }
async function openContent(kind: MarketKind) {
  if (kind === 'mod') return openMods();
  if ((kind === 'resource_pack' || kind === 'shader_pack') && await selectPageTab('content')) contentType.value = kind;
}
const worldExportBusy = ref(false);
watch(pageTab, (tab) => { if (tab === 'worlds') worldsVisited.value = true; if (tab === 'screenshots') screenshotsVisited.value = true; if (tab === 'configs') configsVisited.value = true; if (tab === 'runtime') runtimeVisited.value = true; });
const profileBindingBusy = ref(false);
const profileBindingError = ref('');
async function bindProfile(event: Event) {
  if (profileBindingBusy.value) return;
  const profileId = (event.target as HTMLSelectElement).value || null;
  profileBindingBusy.value = true;
  profileBindingError.value = '';
  try { emit('profileChanged', await invoke<Game>('set_game_profile', { id: props.game.id, profileId })); }
  catch { (event.target as HTMLSelectElement).value = props.game.profile_id || ''; profileBindingError.value = t('gameDetails.profileError'); }
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
  editedName.value = props.game.name;
  actionError.value = '';
  actionStatus.value = '';
  editingName.value = true;
  await nextTick();
  nameInput.value?.focus();
  nameInput.value?.select();
}
async function saveName() {
  if (actionBusy.value) return;
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
const locked = computed(() => repairingGame.value || historyBusy.value || recoveryBusy.value || screenshotBusy.value || packPreviewBusy.value || contentInstalling.value || packMutationBusy.value || worldExportBusy.value || configBusy.value || props.starting || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const playDisabled = computed(() => locked.value || modsActivity.value.busy || profileBindingBusy.value);
async function play() { if (playDisabled.value || !await confirmLeave() || playDisabled.value) return; emit('play', props.game.id); }
async function playServer(serverId: string) { if (playDisabled.value || !await confirmLeave() || playDisabled.value) return; emit('serverPlay', props.game.id, serverId); }
async function refreshContent() { contentRevision.value++; await modsPanel.value?.refreshContent(); }
defineExpose({ refreshContent, canLeave, confirmLeave, openMods, openContent, selectPageTab });
watch(() => props.game.id, () => { pageTab.value = 'content'; runtimeVisited.value = false; worldsVisited.value = false; screenshotsVisited.value = false; configsVisited.value = false; configBusy.value = false; contentType.value = supportsMods.value ? 'mod' : 'resource_pack'; visitedPacks.value = supportsMods.value ? [] : ['resource_pack']; modsActivity.value = {busy: false, installing: false, updating: false, recognizing: false}; }, { immediate: true });
function profileKindLabel(profile: PlayerProfile) { return t(profile.kind === 'microsoft' ? 'profiles.kindMicrosoft' : 'profiles.kindOffline'); }
const playLabel = computed(() => {
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
</script>

<template>
  <div class="details-page page-content">
    <header class="details-summary">
      <div class="details-heading">
        <div class="details-title">
          <h1>{{ game.name }}</h1><button
            class="details-icon-button details-rename"
            :aria-label="t('gameDetails.rename')"
            :title="t('gameDetails.rename')"
            :disabled="actionBusy || editingName || contentBusy"
            @click="editName"
          >
            <AppIcon name="pencil" />
          </button>
        </div>
        <p class="game-version">
          Minecraft {{ game.version }} · {{ game.loader_version ? gameLoaderLabel(game) : t('gameDetails.vanilla') }}
        </p>
        <div class="details-metadata">
          <button
            class="details-profile"
            :disabled="contentBusy"
            @click="selectPageTab('runtime')"
          >
            <AppIcon
              name="user"
              :size="14"
            />{{ profile ? profile.name : t('gameDetails.chooseProfile') }}<AppIcon
              name="chevron-down"
              :size="13"
            />
          </button><span
            v-if="game.pack_name"
            class="pack-origin"
          >{{ t('gameDetails.modpack', {name: game.pack_name}) }}<span v-if="game.pack_version"> · {{ game.pack_version }}</span></span>
        </div>
      </div>
      <div class="details-primary-actions">
        <button
          class="primary-button play-button"
          :disabled="playDisabled"
          @click="play"
        >
          <AppIcon name="play" />{{ playLabel }}
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
            :disabled="actionBusy || editingName || contentBusy"
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
    </header>
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
    <div
      class="content-tabs game-page-tabs"
      role="tablist"
      :aria-label="t('gameDetails.tabs')"
    >
      <button
        v-for="tab in pageTabs"
        :id="`game-${tab.id}-tab`"
        :key="tab.id"
        role="tab"
        :aria-selected="pageTab === tab.id"
        :aria-controls="`game-${tab.id}-panel`"
        :disabled="contentBusy"
        @click="selectPageTab(tab.id)"
      >
        {{ tab.label }}
      </button>
    </div>
    <section
      v-if="pageTab === 'servers'"
      id="game-servers-panel"
      role="tabpanel"
      aria-labelledby="game-servers-tab"
    >
      <GameServersPanel
        :game-id="game.id"
        :locked="playDisabled"
        :running="game.launch_state === 'running'"
        @play="playServer"
      />
    </section>
    <section
      v-if="screenshotsVisited"
      v-show="pageTab === 'screenshots'"
      id="game-screenshots-panel"
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
    </section>
    <section
      v-if="supportsMods && configsVisited"
      v-show="pageTab === 'configs'"
      id="game-configs-panel"
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
    </section>
    <section
      v-if="runtimeVisited"
      v-show="pageTab === 'runtime'"
      id="game-runtime-panel"
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
          <select
            id="game-player-profile"
            :value="game.profile_id || ''"
            :disabled="profileBindingBusy"
            @change="bindProfile"
          >
            <option value="">
              {{ t('gameDetails.followProfile') }}
            </option><option
              v-if="game.profile_id && !library.profiles.some((item) => item.id === game.profile_id)"
              :value="game.profile_id"
              disabled
            >
              {{ t('gameDetails.removedProfile') }}
            </option><option
              v-for="item in library.profiles"
              :key="item.id"
              :value="item.id"
            >
              {{ t('gameDetails.pinProfile', { name: item.name, kind: profileKindLabel(item) }) }}
            </option>
          </select>
        </SettingsRow>
      </SettingsSection>
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
    </section>
    <section
      v-if="worldsVisited"
      v-show="pageTab === 'worlds'"
      id="game-worlds-panel"
      role="tabpanel"
      aria-labelledby="game-worlds-tab"
    >
      <GameWorlds
        :tasks="tasks"
        :game-id="game.id"
        :locked="locked || modsActivity.busy"
        :revision="contentRevision"
        @busy="worldExportBusy = $event"
      />
    </section>
    <section
      v-show="pageTab === 'content'"
      id="game-content-panel"
      role="tabpanel"
      aria-labelledby="game-content-tab"
      class="content-section"
    >
      <div class="details-content-toolbar">
        <label
          class="sr-only"
          for="game-content-type"
        >{{ t('gameDetails.contentTypes') }}</label><select
          v-if="supportsMods"
          id="game-content-type"
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
          <AppIcon name="plus" />{{ t(contentType === 'mod' ? 'gameDetails.addMods' : contentType === 'resource_pack' ? 'gameDetails.addResourcePacks' : 'gameDetails.addShaders') }}
        </button>
      </div>
      <GameModHistory
        v-if="supportsMods && contentType === 'mod'"
        :game="game"
        :locked="locked || modsActivity.busy"
        :revision="contentRevision"
        @busy="historyBusy = $event"
        @updated="refreshContent"
      />
      <GameModsPanel
        v-if="supportsMods"
        v-show="contentType === 'mod'"
        ref="modsPanel"
        :game="game"
        :locked="locked || packMutationBusy || worldExportBusy"
        :revision="contentRevision"
        @activity="modsActivity = $event"
      />
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
    </section>
  </div>
</template>
<style scoped>
.details-summary{display:flex;align-items:flex-start;justify-content:space-between;gap:24px;margin-bottom:24px}.details-heading{min-width:0;flex:1}.details-title{display:flex;align-items:center;gap:9px}.details-title h1{margin:0;font-size:28px;line-height:1.25;letter-spacing:-.6px;overflow-wrap:anywhere}.details-icon-button{display:inline-flex;align-items:center;justify-content:center;flex:none;width:36px;min-height:36px;border:1px solid var(--line);border-radius:9px;background:var(--panel);color:var(--muted);cursor:pointer}.details-icon-button:hover{background:var(--hover);color:var(--ink)}.details-rename{width:30px;min-height:30px;border-color:transparent}.details-metadata{display:flex;align-items:center;flex-wrap:wrap;gap:10px;margin-top:11px}.details-profile{display:inline-flex;align-items:center;gap:7px;min-height:30px;padding:5px 9px;border:1px solid var(--line);border-radius:8px;background:var(--panel);font:inherit;font-size:14px;color:var(--muted);cursor:pointer}.details-summary .game-version{margin:8px 0 0}.details-summary .pack-origin{margin:0;font-size:14px;color:var(--muted)}.details-primary-actions{display:flex;align-items:center;flex-wrap:wrap;justify-content:flex-end;gap:9px;padding-top:3px}.details-primary-actions .play-button{min-height:42px;min-width:112px;padding:10px 18px;display:inline-flex;align-items:center;justify-content:center;gap:8px}.details-primary-actions .secondary-button{min-height:40px;display:inline-flex;align-items:center;gap:8px;padding:9px 13px}.details-content-toolbar{display:flex;justify-content:space-between;align-items:center;gap:12px;margin:20px 0 8px}.details-content-toolbar select{width:auto;min-width:160px;max-width:65%;min-height:40px;padding:8px 12px;border:1px solid var(--line);border-radius:9px;background:var(--panel);color:var(--ink);font:inherit;font-size:13px}.details-content-title{margin:0;font-size:15px;font-weight:600}.details-content-toolbar button{display:inline-flex;align-items:center;gap:7px;min-height:40px}.details-action-menu{min-width:235px;max-width:calc(100vw - 24px);padding:6px;border:1px solid var(--line);border-radius:11px;background:var(--panel);color:var(--ink);box-shadow:0 14px 40px #0002;z-index:100}.details-action-menu :deep([role=menuitem]){display:flex;align-items:center;gap:10px;min-height:38px;padding:9px 11px;border-radius:6px;font-size:13px;outline:0;cursor:default}.details-action-menu :deep([role=menuitem][data-highlighted]){background:var(--hover)}.details-action-menu :deep([role=menuitem][data-disabled]){opacity:.45}.details-action-menu :deep([role=menuitem] svg){flex:none}.details-action-menu :deep([role=separator]){height:1px;margin:5px 4px;background:var(--line)}.game-page-tabs{gap:22px;overflow-x:auto;white-space:nowrap;margin:24px 0 0}.game-page-tabs button{flex-shrink:0}.screenshots-panel{padding-top:20px}.configs-panel{padding-top:6px}.runtime-settings{padding-top:22px}.rename-form{margin:0 0 20px}
@media(max-width:720px){.details-summary{flex-direction:column;gap:16px}.details-primary-actions{width:100%;justify-content:flex-start}.details-primary-actions .play-button{flex:1}.details-title h1{font-size:25px}.details-content-toolbar{flex-wrap:wrap}.details-content-toolbar select{flex:1;max-width:100%;min-width:140px}.details-content-toolbar button{flex-shrink:0}}
</style>
