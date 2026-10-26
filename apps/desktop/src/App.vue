<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { lazyPage } from './lazy-page';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createCloseGuard } from './window-close';
import { useTaskExit } from './use-task-exit';
import { useShellCommands } from './commands/use-shell-commands';
import type { CommandAction } from './commands/registry';
import WindowResizeEdges from './WindowResizeEdges.vue';
import WorkspaceMenus from './desktop-interaction/WorkspaceMenus.vue';
import { workspaceActions } from './desktop-interaction/workspace-actions';
const CommandPalette = lazyPage(() => import('./commands/CommandPalette.vue'));
const TaskExitDialog = lazyPage(() => import('./TaskExitDialog.vue'));
import { afterHomePaint } from './after-home-paint';
import { checkLauncherUpdateAfterHome } from './launcher-updates';
import { useFirstRun } from './use-first-run';
import type { FirstRunStep } from './first-run';
const FirstRunPage = lazyPage(() => import('./FirstRunPage.vue'));
const firstRun = useFirstRun();
const firstRunFlow = ref<'profile' | 'game' | null>(null);
const availableLauncherUpdate = ref('');
const initialSettingsCategory = ref('general');
async function openLauncherUpdate() { initialSettingsCategory.value = 'about'; await navigate('settings'); }
import type { PluginSnapshotRestoreRequest } from './world-snapshots';
import { createPendingLaunchIntentConsumer, type PendingLaunchIntent } from './pending-launch-intent';
import { useRunningTools } from './use-running-tools';
import { applyTheme, readTheme } from './theme';
const CreateGamePage = lazyPage(() => import('./CreateGamePage.vue'));
const ImportPackPage = lazyPage(() => import('./ImportPackPage.vue'));
const CfPackInstallPage = lazyPage(() => import('./CfPackInstallPage.vue'));
import type { CurseForgePackPreview } from './curseforge';
const ImportLauncherPage = lazyPage(() => import('./ImportLauncherPage.vue'));
const GameDirectoryMigrationPage = lazyPage(() => import('./settings/GameDirectoryMigrationPage.vue'));
const ChangeSharePage = lazyPage(() => import('./change-share/ChangeSharePage.vue'));
const GameTransferPage = lazyPage(() => import('./game-transfer/GameTransferPage.vue'));
const MarketPage = lazyPage(() => import('./MarketPage.vue'));
import type { MarketKind } from './market';
import { incomingBlueprint, pendingBlueprint, type PendingBlueprintRequest } from './blueprints';
import type { FileInboxHostRequest, FileInboxResult } from './file-inbox-host';
import { openPluginHandoff } from './plugin-handoffs';
import { recoveryNavigationPage } from './recovery-navigation';
import { usePluginNavigation } from './use-plugin-navigation';
const SettingsPage = lazyPage(() => import('./SettingsPage.vue'));
import LibraryHome from './LibraryHome.vue';
import CommandStatus from './commands/CommandStatus.vue';
import MeshStatusButton from './MeshStatusButton.vue';
import AppNavigation from './AppNavigation.vue';
import { activeSection, extensionDetailPage, backPage, enterPage, goToParent, initialNavigation, pageKey, rootPage, returnToGameDetails, toolPage, type AppPage, type ExtensionCandidate, type MainSection, type ToolLocation } from './navigation';
import { BLUEPRINTS_TOOL_ID } from './tools';
const PluginsPanel = lazyPage(() => import('./PluginsPanel.vue'));
const PluginDetails = lazyPage(() => import('./PluginDetails.vue'));
const ExtensionSettingsPanel = lazyPage(() => import('./ExtensionSettingsPanel.vue'));
import { pluginLocale, type ExtensionContribution, type ResolvedCommand } from './extensions/contributions';
const GameDetailsPage = lazyPage(() => import('./GameDetailsPage.vue'));
const GameRunningPage = lazyPage(() => import('./GameRunningPage.vue'));
const GameReadinessPage = lazyPage(() => import('./GameReadinessPage.vue'));
async function openReadiness() { if (selectedGame.value && await confirmWorkspaceLeave()) showPage({ view: 'readiness', gameId: selectedGame.value.id }); }
const CreatedGamePreferences = lazyPage(() => import('./CreatedGamePreferences.vue'));
const GameActionsPage = lazyPage(() => import('./GameActionsPage.vue'));
const MeshPage = lazyPage(() => import('./MeshPage.vue'));
const MeshGroupPanel = lazyPage(() => import('./mesh-group/MeshGroupPanel.vue'));
const PreparedGameJoin = lazyPage(() => import('./mesh-environment/PreparedGameJoin.vue'));
import { t, locale } from './i18n';
import ProfileSwitcher from './ProfileSwitcher.vue';
const ProfilesPage = lazyPage(() => import('./ProfilesPage.vue'));
const AddProfilePage = lazyPage(() => import('./AddProfilePage.vue'));
const TasksPage = lazyPage(() => import('./TasksPage.vue'));
import { isActiveTask, taskTitleLabel, taskStateLabel, type TaskSnapshot } from './tasks';
import { effectiveProfile, type Game, type GameAction, type PackPreview, type ProfileLibrary } from './games';
const ToolsPage = lazyPage(() => import('./ToolsPage.vue'));
const FileInboxHostReview = lazyPage(() => import('./FileInboxHostReview.vue'));
const inboxReviewRequest = ref<FileInboxHostRequest | null>(null);
const inboxHostReview = ref<{ confirmLeave: () => Promise<boolean> } | null>(null);
const games = ref<Game[]>([]);
const gamesLoading = ref(true);
const gamesError = ref('');
const tasks = ref<TaskSnapshot[]>([]);
const tasksOpen = ref(false);
const preparedInvitation = ref<{gameId: string; invitation: string} | null>(null);
const profileMenuOpen = ref(false);
const toolsPage = ref<{ suspend: () => Promise<void>; resume: () => void; reload: () => void; confirmLeave: () => Promise<boolean> } | null>(null);
let changingTasks = false;
async function suspendToolView() {
  if (view.value === 'tools') await toolsPage.value?.suspend();
}
async function toggleTasks() {
  if (changingTasks) return;
  changingTasks = true;
  try {
    if (!tasksOpen.value) await suspendToolView();
    tasksOpen.value = !tasksOpen.value;
  } catch { error.value = t('shell.error.windowAction'); }
  finally { changingTasks = false; }
}
watch([tasksOpen, profileMenuOpen], ([tasksVisible, profileVisible]) => {
  if (!tasksVisible && !profileVisible && !inboxReviewRequest.value && !shellCommands.paletteOpen.value && !taskExit.open.value && view.value === 'tools') toolsPage.value?.resume();
});
const taskPreviewBusy = ref(false);
const tasksLoading = ref(false);
const tasksError = ref('');
const activeTaskCount = computed(() => tasks.value.filter(isActiveTask).length);
const gameDetailsPage = ref<InstanceType<typeof GameDetailsPage> | null>(null);
const retriedTasks = new Set<string>();
const staleContentGames = new Set<string>();
let unlistenTasks: UnlistenFn | undefined;
let unlistenSchematic: UnlistenFn | undefined;
let unlistenClose: UnlistenFn | undefined;
let unlistenSnapshotRestore: UnlistenFn | undefined;
async function listenSnapshotRestore() {
  if (!native) return;
  const stop = await listen<PluginSnapshotRestoreRequest>('plugin-open-snapshot-restore', async ({ payload }) => {
    if (disposed || navigationBusy.value || !await confirmWorkspaceLeave() || disposed) return;
    const game = games.value.find(item => item.id === payload.game_id && item.status === 'installed');
    if (!game) return;
    tasksOpen.value = false;
    showPage({ view: 'details', gameId: game.id, snapshotRestore: { worldId: payload.world_id, snapshotId: payload.snapshot_id } });
  });
  if (disposed) stop(); else unlistenSnapshotRestore = stop;
}
async function guardWindowClose() {
  if (!native) return;
  const stop = await getCurrentWindow().onCloseRequested(createCloseGuard(taskExit.confirmExit,()=>{error.value=t('shell.error.windowAction');}));
  if (disposed) stop(); else unlistenClose = stop;
}
let pendingSchematicLoading=false;
async function loadPendingSchematic() {
  if (!native || pendingSchematicLoading) return;
  pendingSchematicLoading=true;
  try {
    const value=await invoke<PendingBlueprintRequest | null>('peek_pending_schematic');
    if (!value || disposed || !await confirmWorkspaceLeave() || disposed) return;
    // A newer OS-open request may replace the queue while the leave dialog is open.
    const current=await invoke<PendingBlueprintRequest | null>('peek_pending_schematic');
    if(!current || disposed) return;
    pendingBlueprint.value=current;
    navigation.value = enterPage(enterPage(navigation.value, {view:'tools'}, 'root'), {view:'tools',location:{pluginId:BLUEPRINTS_TOOL_ID,blueprintView:'library'}});
  } catch { error.value=t('shell.error.windowAction'); }
  finally {pendingSchematicLoading=false;}
}
async function listenSchematics() {
  if (!native) return;
  const stop=await listen('schematic-open',()=>{void loadPendingSchematic();});
  if(disposed){stop();return;} unlistenSchematic=stop; await loadPendingSchematic();
}
function updateTask(task: TaskSnapshot) {
  const index = tasks.value.findIndex((item) => item.id === task.id);
  const previous = tasks.value[index];
  if (previous && previous.updated_at > task.updated_at) return;
  if (previous && ['failed', 'cancelled', 'interrupted'].includes(previous.state) && ['queued', 'running'].includes(task.state)) retriedTasks.add(task.id);
  if (retriedTasks.has(task.id) && task.state === 'succeeded') {
    retriedTasks.delete(task.id);
    if (task.game_id && view.value === 'details' && selectedGameId.value === task.game_id && gameDetailsPage.value) {
      staleContentGames.add(task.game_id); void refreshReturnedContent();
    }
  }
  if (index < 0) tasks.value.unshift(task);
  else if (tasks.value[index]!.updated_at <= task.updated_at) tasks.value[index] = task;
}
async function refreshReturnedContent() {
  if (tasksOpen.value || view.value !== 'details' || !staleContentGames.has(selectedGameId.value)) return;
  const id = selectedGameId.value;
  await nextTick();
  if (tasksOpen.value || view.value !== 'details' || selectedGameId.value !== id || !staleContentGames.has(id) || !gameDetailsPage.value) return;
  staleContentGames.delete(id);
  await gameDetailsPage.value.refreshContent();
}
async function loadTasks() {
  if (!native || tasksLoading.value) return;
  tasksLoading.value = true; tasksError.value = '';
  try {
    if (!unlistenTasks) {
      const stop = await listen<TaskSnapshot>('task-updated', (event) => updateTask(event.payload));
      if (disposed) { stop(); return; }
      unlistenTasks = stop;
    }
    const value = await invoke<TaskSnapshot[]>('list_tasks');
    if (!disposed && Array.isArray(value)) for (const task of value) updateTask(task);
  } catch { if (!disposed) tasksError.value = t('shell.error.tasksLoad'); }
  finally { tasksLoading.value = false; }
}
async function continueTaskPack(preview: PackPreview) { if (navigationBusy.value || !await confirmWorkspaceLeave()) return; discoverPacksVisited.value = true; openOnlineImport(preview); tasksOpen.value = false; }
async function continueTaskCfPack(preview: CurseForgePackPreview) { if (navigationBusy.value || !await confirmWorkspaceLeave()) return; discoverPacksVisited.value = true; openCfImport(preview); tasksOpen.value = false; }
const profileLibrary = ref<ProfileLibrary>({ profiles: [], active_id: null });
const profileReady = ref(false);
const profileBusy = ref(false);
const authBusy = ref(false);
const addProfilePage = ref<InstanceType<typeof AddProfilePage> | null>(null);
const profileError = ref('');
const pendingGameId = ref<string | null>(null);
const pendingServerId = ref<string | undefined>();
watch(pendingGameId, value => { if (!value) pendingServerId.value = undefined; });
const starting = ref<string[]>([]);
const gameRevisions = new Map<string, number>();
async function loadProfile() {
  if (!native) { profileReady.value = true; return; }
  profileError.value = '';
  try { profileLibrary.value = await invoke<ProfileLibrary>('list_player_profiles'); profileReady.value = true; }
  catch { profileError.value = t('shell.error.profileLoad'); }
}
async function openProfiles() {
  if (!await confirmWorkspaceLeave()) return;
  profileError.value = '';
  showPage({view:'profiles'});
}
async function openAddProfile() {
  if (!await confirmWorkspaceLeave()) return;
  profileError.value = '';
  showPage({view:'add-profile'});
}
async function leaveProfiles() {
  if (profileBusy.value || authBusy.value) return;
  profileError.value = '';
  if (view.value === 'add-profile') {
    if (addProfilePage.value && !await addProfilePage.value.cancelLogin()) return;
    await loadProfile();
  }
  firstRunFlow.value = null;
  navigation.value=goToParent(navigation.value);
  if (view.value==='details') pendingGameId.value=null;
}
async function finishProfileChoice(returnToGame: boolean) {
  if (returnToGame && view.value === 'first-run' && firstRun.state.value?.step === 'profile') return;
  if (returnToGame && firstRunFlow.value === 'profile') { firstRunFlow.value = null; await setFirstRunStep('game'); showPage({view:'first-run'},'replace'); return; }
  if (returnToGame && pendingGameId.value) {
    const id = pendingGameId.value;
    const serverId = pendingServerId.value;
    const game = games.value.find((game) => game.id === pendingGameId.value);
    if (game?.profile_id) updateGame(await invoke<Game>('set_game_profile', { id: game.id, profileId: profileLibrary.value.active_id }));
    navigation.value=returnToGameDetails(navigation.value,pendingGameId.value);
    pendingGameId.value = null;
    pendingServerId.value = undefined;
    await startGame(id, serverId);
  } else if (returnToGame && view.value === 'add-profile') navigation.value=goToParent(navigation.value);
}
async function acceptMicrosoftLibrary(library: ProfileLibrary) {
  if (profileBusy.value) return;
  profileLibrary.value = library; profileError.value = ''; profileBusy.value = true;
  try { await finishProfileChoice(true); }
  catch { profileError.value = t('shell.error.profileSavedChoose'); showPage({view:'profiles'},'replace'); }
  finally { profileBusy.value = false; }
}
async function changeProfile(command: string, args: Record<string, unknown>, returnToGame = false) {
  if (profileBusy.value || !profileReady.value) return;
  profileError.value = '';
  if (!native) { profileError.value = t('shell.error.profileDesktopOnly'); return; }
  profileBusy.value = true;
  try {
    profileLibrary.value = await invoke<ProfileLibrary>(command, args);
    await finishProfileChoice(returnToGame);
  } catch { profileError.value = t('shell.error.profileSave'); }
  finally { profileBusy.value = false; }
}
function selectProfile(id: string) { void changeProfile('select_player_profile', { id }, true); }
const retrying = ref<string[]>([]);
let unlisten: UnlistenFn | undefined;
let disposed = false;
function updateGame(game: Game) {
  gameRevisions.set(game.id, (gameRevisions.get(game.id) ?? 0) + 1);
  const index = games.value.findIndex((item) => item.id === game.id);
  const wasVerifying = games.value[index]?.launch_state === 'verifying';
  if (index < 0) games.value.unshift(game);
  else games.value[index] = game;
  if (wasVerifying && game.launch_state !== 'verifying') launchIntents.notify();
  if (game.launch_error && starting.value.includes(game.id) && view.value === 'library' && !tasksOpen.value) {
    manageGame(game.id);
  }
}
async function startGame(id: string, serverId?: string) {
  if (tasks.value.some(task => task.game_id === id && task.kind === 'game_repair' && isActiveTask(task))) return;
  let current = games.value.find((game) => game.id === id);
  if (!current || current.content_installing || starting.value.includes(id) || (current.launch_state ?? 'idle') !== 'idle') return;
  if (!profileReady.value || !effectiveProfile(current, profileLibrary.value)) {
    if (!await confirmWorkspaceLeave()) return;
    pendingGameId.value = id;
    pendingServerId.value = serverId;
    if (view.value!=='details' || selectedGameId.value!==id) showPage({view:'details',gameId:id});
    showPage({view:profileLibrary.value.profiles.length || !profileReady.value ? 'profiles' : 'add-profile'});
    return;
  }
  if (!await confirmWorkspaceLeave()) return;
  current = games.value.find(game => game.id === id);
  if (!current || current.content_installing || starting.value.includes(id) || (current.launch_state ?? 'idle') !== 'idle') return;
  starting.value.push(id);
  tasksOpen.value = false;
  showPage({ view: 'running', gameId: id }, view.value === 'details' && selectedGameId.value === id ? 'replace' : 'push');
  const revision = gameRevisions.get(id) ?? 0;
  try {
    if (!native) throw new Error(t('shell.error.gameDesktopOnly'));
    const game = serverId
      ? await invoke<Game>('start_game_server', { id, serverId })
      : await invoke<Game>('start_game', { id });
    if ((gameRevisions.get(id) ?? 0) === revision) updateGame(game);
  } catch (cause) {
    if ((gameRevisions.get(id) ?? 0) === revision) updateGame({ ...current, launch_state: 'idle', launch_error: String(cause), log_path: null });
  } finally { starting.value = starting.value.filter((item) => item !== id); }
}
const createdPreferenceGameId = ref('');
function registerImportedGame(game: Game) {
  // Native progress can arrive before the create command returns.
  if (!games.value.some(item => item.id === game.id)) updateGame(game);
}
function openImportedGame(game: Game) {
  registerImportedGame(game);
  showPage({ view: 'details', gameId: game.id }, 'replace');
}
function addCreatedGame(game: Game) {
  if (firstRunFlow.value === 'game') { firstRunFlow.value = null; if (!games.value.some(item => item.id === game.id)) games.value.unshift(game); void setFirstRunStep('mesh'); showPage({view:'first-run'},'replace'); return; }
  showPage({view:'library'},'root');
  createdPreferenceGameId.value = game.id;
  if (!games.value.some((item) => item.id === game.id)) games.value.unshift(game);
}
function reuseCreatedGameKeys(targetGameId: string) {
  openToolLocation({ pluginId: 'config-doctor', keybindingTargetId: targetGameId });
  createdPreferenceGameId.value = '';
}
async function loadGames() {
  if (!native) { gamesLoading.value = false; return; }
  gamesLoading.value = true;
  gamesError.value = '';
  try {
    if (!unlisten) {
      const stop = await listen<Game>('game-install', (event) => updateGame(event.payload));
      if (disposed) { stop(); return; }
      unlisten = stop;
    }
    const stored = await invoke<Game[]>('list_games');
    if (disposed) return;
    for (const game of stored) if (!games.value.some((item) => item.id === game.id)) games.value.push(game);
  } catch { gamesError.value = t('shell.error.gameLibraryLoad'); }
  finally { gamesLoading.value = false; }
}
async function retryGame(id: string) {
  if (retrying.value.includes(id)) return;
  retrying.value.push(id);
  try {
    const game = await invoke<Game>('retry_game', { id });
    if (games.value.find((item) => item.id === id)?.status === 'failed') updateGame(game);
  }
  catch (cause) { error.value = t('shell.error.gameRetry', { cause: String(cause) }); }
  finally { retrying.value = retrying.value.filter((item) => item !== id); }
}
async function retryGameSessionRecovery(id: string) {
  if (retrying.value.includes(id)) return;
  retrying.value.push(id);
  try { await invoke('retry_game_session_recovery', { id }); }
  catch (cause) { error.value = t('shell.error.gameRetry', { cause: String(cause) }); }
  finally { retrying.value = retrying.value.filter(item => item !== id); }
}
const navigation = ref(initialNavigation());
const page = computed(() => navigation.value.current);
const view = computed(() => page.value.view);
const consentRequired = computed(() => view.value === 'first-run' && !firstRun.consentAccepted());
async function setFirstRunStep(step: FirstRunStep) { await firstRun.save('step', { step }); }
async function acceptFirstRunConsent() { await firstRun.save('acceptConsent'); }
async function resumeFirstRun(reviewTerms = false) {
  if (!await confirmWorkspaceLeave()) return;
  try { await firstRun.load(); if (await firstRun.save('resume')) { if (reviewTerms) await setFirstRunStep('consent'); showPage({view:'first-run'}); } }
  catch (cause) { error.value = String(cause); }
}
function openFirstRunFlow(flow: 'profile' | 'game') {
  if (!firstRun.consentAccepted() || firstRun.busy.value) return;
  firstRunFlow.value = flow; showPage({view: flow === 'profile' ? 'add-profile' : 'create'});
}
async function deferFirstRun() { if (await firstRun.save('defer')) showPage({view:'library'},'root'); }
async function completeFirstRun() { if (await firstRun.save('complete')) showPage({view:'library'},'root'); }
async function chooseFirstRunMesh(meshChoice: boolean) { if (await firstRun.save('meshChoice', { meshChoice })) { meshNetworkRevision.value++; await completeFirstRun(); } }
const detailsVisited = ref(false);
watch(view, value => { if (value === 'details') detailsVisited.value = true; }, { flush: 'sync' });
function showPage(next: AppPage, mode: 'push' | 'replace' | 'root' = 'push') { navigation.value=enterPage(navigation.value,next,mode); }
const meshVisited = ref(false);
const meshBusy = ref(false);
const meshPage=ref<{confirmLeave:()=>Promise<boolean>}|null>(null);
const meshGroupPage=ref<{confirmLeave:()=>Promise<boolean>}|null>(null);
let meshContactRequestId = 0;
async function openMesh() { if (!await confirmWorkspaceLeave()) return; meshVisited.value = true; showPage({view:'mesh',gameId:selectedGameId.value}); }
async function openMeshDestination(mode: 'join' | 'contacts', peerId?: string) {
  if (navigationBusy.value || !await confirmWorkspaceLeave()) return;
  meshVisited.value = true;
  showPage({ view: 'mesh', gameId: selectedGameId.value, initialMode: mode, ...(peerId ? { initialContactId: peerId, contactRequestId: ++meshContactRequestId } : {}) }, 'root');
}
function meshContactOpened(requestId: number) {
  const current = page.value;
  if (current.view !== 'mesh' || current.contactRequestId !== requestId) return;
  showPage({ view: 'mesh', gameId: current.gameId, initialMode: 'contacts' }, 'replace');
}
async function openGameGroup() {
  if (navigationBusy.value || !selectedGame.value || !await confirmWorkspaceLeave()) return;
  showPage({view:'mesh-group',gameId:selectedGameId.value});
}
const pluginDetailsPage=ref<{confirmLeave:()=>Promise<boolean>}|null>(null);
const settingsPage = ref<InstanceType<typeof SettingsPage> | null>(null);
const changeSharePage = ref<{confirmLeave:()=>Promise<boolean>} | null>(null);
const gameTransferPage = ref<{confirmLeave:()=>Promise<boolean>} | null>(null);
async function openDirectoryMigration(taskId?: string) {
  if (navigationBusy.value || !await confirmWorkspaceLeave()) return;
  tasksOpen.value = false;
  showPage({ view: 'move-games', taskId }, view.value === 'move-games' ? 'replace' : 'push');
}
async function openSettingsCategory(category: string) {
  if (view.value === 'settings') { await settingsPage.value?.openCategory(category); return; }
  initialSettingsCategory.value = category;
  await navigate('settings');
}
async function confirmWorkspaceLeave(): Promise<boolean> {
  if (consentRequired.value) return false;
  if (inboxReviewRequest.value) return await inboxHostReview.value?.confirmLeave() ?? false;
  if (view.value === 'tools') return await toolsPage.value?.confirmLeave() ?? true;
  if (view.value === 'mesh') return await meshPage.value?.confirmLeave() ?? true;
  if (view.value === 'mesh-group') return await meshGroupPage.value?.confirmLeave() ?? true;
  if (view.value === 'extension-detail') return await pluginDetailsPage.value?.confirmLeave() ?? true;
  if (view.value === 'settings') return await settingsPage.value?.confirmLeave() ?? true;
  if (view.value === 'details') return await gameDetailsPage.value?.confirmLeave() ?? true;
  if (view.value === 'share-changes') return await changeSharePage.value?.confirmLeave() ?? true;
  if (view.value === 'game-transfer') return await gameTransferPage.value?.confirmLeave() ?? true;
  return true;
}
const toolLocation = computed(() => page.value.view==='tools' ? page.value.location : undefined);
const extensionCategories = ref<{ id: string; label: string; pluginId: string; categoryId: string }[]>([]);
const extensionPanels = new Map<string, { confirmLeave: () => Promise<boolean> }>();
function registerExtensionPanel(id: string, panel: unknown) {
  if (panel) extensionPanels.set(id, panel as { confirmLeave: () => Promise<boolean> }); else extensionPanels.delete(id);
}
async function leaveExtensionSettings() {
  for (const panel of extensionPanels.values()) if (!await panel.confirmLeave()) return false;
  return true;
}
async function loadExtensionCategories() {
  try {
    const items = await invoke<ExtensionContribution[]>('list_extension_contributions', { locale: pluginLocale() });
    extensionCategories.value = items.flatMap(item => item.settings.filter(category => category.fields?.some(field => field.scope === 'global')).map(category => ({ id: `extension-${item.pluginId}-${category.id}`, label: `${item.pluginName} · ${category.label}`, pluginId: item.pluginId, categoryId: category.id })));
  } catch { extensionCategories.value = []; }
}
watch(locale, () => { if (view.value === 'settings') void loadExtensionCategories(); });

const selectedSection = computed(() => activeSection(page.value,navigation.value.parents));
const navigationBusy = computed(() => consentRequired.value || firstRun.busy.value || !!inboxReviewRequest.value || taskPreviewBusy.value || profileBusy.value || authBusy.value || gameActionBusy.value || packPreviewBusy.value);
async function navigate(section: MainSection) {
  if (navigationBusy.value) return;
  if (!await confirmWorkspaceLeave()) return;
  firstRunFlow.value = null;
  tasksOpen.value = false;
  pendingGameId.value=null;
  if (section === 'settings') await loadExtensionCategories();
  if (section === 'discover-packs') { discoverPacksVisited.value=true; }
  if (section === 'mesh') { meshVisited.value=true; }
  showPage(rootPage(section),'root');
}
async function openExtensions(candidate?: ExtensionCandidate) {
  if (navigationBusy.value || !await confirmWorkspaceLeave()) return;
  tasksOpen.value=false;
  showPage({view:'extensions',candidate});
}
async function openPluginDetail(pluginId:string) {
  if(navigationBusy.value || !await confirmWorkspaceLeave()) return;
  navigation.value=extensionDetailPage(navigation.value,pluginId);
}
function pluginDetailTitle(title:string) {
  if(page.value.view==='extension-detail') showPage({...page.value,title},'replace');
}
function pluginRemoved() { navigation.value=goToParent(navigation.value); }
async function openTool(pluginId: string) { if (!await confirmWorkspaceLeave()) return; openToolLocation({pluginId}); }
function openToolLocation(location:ToolLocation) { navigation.value=toolPage(navigation.value,location); }
async function openExtensionCommand(command: ResolvedCommand) { if (!await confirmWorkspaceLeave()) return; openToolLocation({pluginId:command.workflow==='viewer'?BLUEPRINTS_TOOL_ID:command.pluginId,command,instanceId:command.instanceId,blueprintView:command.workflow==='viewer'?'library':undefined,title:command.label}); }
const discoverPacksVisited = ref(false);
const packPreviewBusy = ref(false);
const initialPackPreview = ref<PackPreview | null>(null);
const initialCfPackPreview = ref<CurseForgePackPreview | null>(null);
const marketKind = computed(() => navigation.value.market.kind);
const marketRevision = ref(0);
const marketGameId = computed(() => navigation.value.market.gameId);
async function openGameMarket(kind: MarketKind) { if (!await confirmWorkspaceLeave()) return; ++marketRevision.value; discoverPacksVisited.value = true; showPage({view:'discover-packs',kind,gameId:selectedGameId.value}); }
function marketInstalled(id: string) { staleContentGames.add(id); }
async function openBlueprintMarket(target: { gameId: string; query: string }) {
  if (!await confirmWorkspaceLeave()) return;
  ++marketRevision.value;
  discoverPacksVisited.value = true;
  showPage({ view: 'discover-packs', kind: 'mod', gameId: target.gameId, query: target.query });
}
async function locateMarketContent(id: string, kind: MarketKind) {
  await locateTaskGame(id);
  await nextTick();
  if (page.value.view === 'details' && page.value.gameId === id) await gameDetailsPage.value?.openContent(kind);
}
function openLocalImport() { initialPackPreview.value = null; showPage({view:'import'}); }
function finishInboxReview() {
  inboxReviewRequest.value = null;
  if (view.value === 'tools' && !tasksOpen.value && !profileMenuOpen.value) toolsPage.value?.resume();
}
async function acceptInboxResult(result: FileInboxResult) {
  if (result.kind === 'report') return;
  if (result.kind === 'mrpack') { initialPackPreview.value = result.preview; showPage({ view: 'import' }); }
  else if (result.kind === 'curseforge-pack') { initialCfPackPreview.value = result.preview; showPage({ view: 'cf-import' }); }
  else if (result.kind === 'viewer') {
    incomingBlueprint.value = result.viewer;
    const current = page.value;
    openToolLocation({ pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'viewer', instanceId: current.view === 'tools' ? current.location?.instanceId : undefined });
  } else {
    manageGame(result.gameId, true);
    await nextTick();
    await gameDetailsPage.value?.openContent(result.contentKind === 'resource-pack' ? 'resource_pack' : result.contentKind === 'shader-pack' ? 'shader_pack' : 'mod');
    await gameDetailsPage.value?.refreshContent();
  }
}
function ensureMarketParent() { if(view.value!=='discover-packs') showPage(navigation.value.market,'root'); }
function openCfImport(preview: CurseForgePackPreview | null = null) { initialCfPackPreview.value = preview; if(preview) ensureMarketParent(); showPage({view:'cf-import'},view.value==='import'?'replace':'push'); }
function openOnlineImport(preview: PackPreview) { initialPackPreview.value = preview; ensureMarketParent(); showPage({view:'import'}); }
const gameAction = computed<GameAction>(()=>page.value.view==='game-action'?page.value.action:'copy');
const gameActionBusy = ref(false);
const contentBusy = ref(false);

const selectedGameId = computed(()=> 'gameId' in page.value && page.value.gameId ? page.value.gameId : navigation.value.lastGameId);
const selectedGame = computed(() => games.value.find((game) => game.id === selectedGameId.value));
const runningTools = useRunningTools({ gameId: selectedGameId, launchState: () => selectedGame.value?.launch_state, active: () => view.value === 'running' && !tasksOpen.value });
async function openRunningTool(command: ResolvedCommand) {
  const resolved = await runningTools.resolve(command);
  if (resolved && view.value === 'running' && selectedGameId.value === resolved.instanceId) await openExtensionCommand(resolved);
}
async function openSeedCompass() {
  if (view.value !== 'running' || runningTools.loading.value || runningTools.resolving.value) return;
  const command = runningTools.commands.value.find(item => item.pluginId === 'seed-compass' && item.commandId === 'seed-compass.open');
  if (command) await openRunningTool(command);
  else await openExtensions({sourceId:'bundled',pluginId:'seed-compass'});
}
function manageGame(id: string, forceSettings = false) {
  const game = games.value.find(item => item.id === id);
  if (game?.status !== 'installed') return;
  const live = starting.value.includes(id) || ['preparing', 'running', 'verifying', 'recovery_failed'].includes(game.launch_state ?? 'idle');
  showPage({view: !forceSettings && live ? 'running' : 'details',gameId:id});
}
async function locateTaskGame(id: string) {
  if (!games.value.some(game => game.id === id && game.status === 'installed')) return;
  if (!await confirmWorkspaceLeave()) return;
  tasksOpen.value = false;
  manageGame(id, true);
}
async function openRuntimeSettings(id: string) {
  if (!await confirmWorkspaceLeave()) return;
  showPage({view: 'details', gameId: id, recoveryTarget: {kind: 'runtimeSettings'}});
}
async function showBackgroundCopy() {
  gameActionBusy.value = false;
  tasksOpen.value = true;
  await loadTasks();
}
async function showPreparedEnvironment() {
  tasksOpen.value = true;
  await Promise.all([loadGames(), loadTasks()]);
}
async function joinPreparedEnvironment(value: {gameId: string; invitation: string}) {
  const game = games.value.find(item => item.id === value.gameId);
  if (!game || game.status !== 'installed' || game.content_installing || game.error || !await confirmWorkspaceLeave()) return;
  tasksOpen.value = false;
  showPage({view: 'mesh-group', gameId: value.gameId, invitation: value.invitation});
}
async function openGameAction(action: GameAction) {
  if (!await confirmWorkspaceLeave()) return;
  const game = selectedGame.value;
  if (!game || game.status !== 'installed' || game.content_installing || starting.value.includes(game.id) || (game.launch_state ?? 'idle') !== 'idle') return;
  showPage({view:'game-action',gameId:game.id,action});
}
async function openGameExchange(mode: 'export' | 'import' | 'transfer') {
  if (!await confirmWorkspaceLeave()) return;
  const game = selectedGame.value;
  if (!game || game.status !== 'installed' || game.content_installing || starting.value.includes(game.id) || (game.launch_state ?? 'idle') !== 'idle') return;
  showPage(mode === 'transfer' ? {view:'game-transfer',gameId:game.id} : {view:'share-changes',gameId:game.id,mode});
}
async function receiveGameTransfer() {
  if (navigationBusy.value || !await confirmWorkspaceLeave()) return;
  showPage({view:'game-transfer',gameId:''});
}
async function openTransferTask(taskId: string) {
  if (!await confirmWorkspaceLeave()) return;
  tasksOpen.value = false;
  showPage({view:'game-transfer',gameId:'',taskId});
}
async function changesApplied(id: string) {
  staleContentGames.add(id);
  await loadGames();
  navigation.value = returnToGameDetails(navigation.value, id);
  await refreshReturnedContent();
}
async function openTransferredGame(id: string) {
  await loadGames();
  if (games.value.some(game => game.id === id)) showPage({view:'details',gameId:id});
}
function removeFromLibrary(id: string) {
  games.value = games.value.filter((game) => game.id !== id);
  gameRevisions.delete(id);
  if(navigation.value.lastGameId===id) navigation.value.lastGameId='';
  if (pendingGameId.value === id) pendingGameId.value = null;
  showPage({view:'library'},'root');
}
async function goBack() {
  if(tasksOpen.value) {if(!taskPreviewBusy.value) tasksOpen.value=false;return;}
  if (!await confirmWorkspaceLeave()) return;
  if (navigationBusy.value) return;
  if (view.value === 'profiles' || view.value === 'add-profile') await leaveProfiles();
  else { firstRunFlow.value = null; navigation.value=goToParent(navigation.value); }
}
const parentPage = computed(()=>backPage(navigation.value));
function pageTitle(value:AppPage):string {
  switch(value.view) {
    case 'readiness':return t('readiness.title');
    case 'first-run':return locale.value === 'zh-CN' ? '首次使用' : 'Getting started';
    case 'library':return t('shell.back.libraryLabel');
    case 'details':case 'running':return games.value.find(game=>game.id===value.gameId)?.name ?? t('shell.back.gameDetailsLabel');
    case 'tools':return value.location?.title ?? value.location?.command?.label ?? (value.location?.blueprintView==='online'?t('blueprintOnline.online'):value.location?.pluginId===BLUEPRINTS_TOOL_ID?t('tools.blueprintsName'):t('navigation.tools'));
    case 'extension-detail':return value.title ?? t('navigation.extensions');
    case 'extensions':return t('navigation.extensions');
    case 'discover-packs':return t('shell.page.market');
    case 'game-action':return ({copy:t('shell.action.copy'),export:t('shell.action.export'),remove:t('shell.action.remove')})[value.action];
    case 'share-changes':return t(value.mode === 'export' ? 'gameDetails.shareChanges' : 'gameDetails.applyChanges');
    case 'game-transfer':return t(value.gameId ? 'gameDetails.transferGame' : 'home.receiveGame');
    case 'mesh':return t('mesh.title');
    case 'mesh-group':return t('runningGame.share');
    case 'create':return t('shell.page.create');
    case 'import':case 'cf-import':return t('shell.page.importPack');
    case 'import-launcher':return t('shell.page.importLauncher');
    case 'profiles':return t('shell.page.profiles');
    case 'add-profile':return t('shell.page.addProfile');
    case 'settings':return t('shell.page.settings');
    case 'move-games':return locale.value === 'zh-CN' ? '搬迁游戏' : 'Move games';
  }
}
const sharedHeading=ref<HTMLElement|null>(null);
const workspacePositions = new Map<string, number>();
let positionRevision = 0;
watch(() => tasksOpen.value ? 'tasks' : pageKey(page.value), async (key, previous) => {
  const revision = ++positionRevision;
  if (previous) {
    workspacePositions.delete(previous);
    workspacePositions.set(previous, window.scrollY);
    if (workspacePositions.size > 32) workspacePositions.delete(workspacePositions.keys().next().value!);
  }
  await nextTick();
  if (revision !== positionRevision || disposed) return;
  void refreshReturnedContent();
  const heading=sharedHeading.value ?? [...document.querySelectorAll<HTMLElement>('.app-workspace h1')].find(element=>element.getClientRects().length>0);
  heading?.setAttribute('tabindex','-1');heading?.focus({ preventScroll: true });
  window.scrollTo(0, workspacePositions.get(key) ?? 0);
});
const theme = readTheme();
const native = isTauri();
const maximized = ref(false);
const error = ref('');
const profilesSettled = ref(false);
const meshNetworkRevision = ref(0);
let homeWorkScheduled = false;
let cancelHomeWork: (() => void) | undefined;
let unlistenLaunchIntent: UnlistenFn | undefined;
const launchIntents = createPendingLaunchIntentConsumer({
  peek: () => invoke<PendingLaunchIntent | null>('peek_pending_launch'),
  acknowledge: queueId => invoke<boolean>('acknowledge_pending_launch', { queueId }),
  hasGame: id => games.value.some(game => game.id === id),
  isVerifying: id => games.value.some(game => game.id === id && game.launch_state === 'verifying'),
  confirmWorkspaceLeave,
  openGame: id => { tasksOpen.value = false; manageGame(id); },
  startGame: async id => { tasksOpen.value = false; manageGame(id); await startGame(id); },
  missingGame: () => { error.value = t('shell.error.quickLaunchUnavailable'); },
  failed: cause => { error.value = t(String(cause).includes('QUICK_LAUNCH_GAME_UNAVAILABLE') ? 'shell.error.quickLaunchUnavailable' : 'shell.error.quickLaunch'); },
});
watch([gamesLoading, profilesSettled], ([loading, settled]) => {
  if (!native || loading || !settled || gamesError.value) return;
  launchIntents.markReady();
  if (!homeWorkScheduled) {
    homeWorkScheduled = true;
    void nextTick().then(() => {
      if (disposed) return;
      cancelHomeWork = afterHomePaint(() => {
        void invoke('report_home_ready', { rendererReadyMs: performance.now() }).catch(() => {});
        void invoke('mesh_network_start').catch(() => {});
        void checkLauncherUpdateAfterHome().then(status => { if (!disposed && status?.available_version) availableLauncherUpdate.value = status.available_version; }).catch(() => {});
        void firstRun.load().then(state => { if (!disposed && view.value === 'library' && !state.deferred && !state.completed) showPage({view:'first-run'}); }).catch(() => {});
      });
    });
  }
});
async function listenLaunchIntents() {
  if (!native) return;
  const stop = await listen('launch-intent', () => launchIntents.notify());
  if (disposed) stop(); else { unlistenLaunchIntent = stop; launchIntents.notify(); }
}
async function syncWindow() {
  if (native) { try { maximized.value = await getCurrentWindow().isMaximized(); } catch { /* Actions surface errors. */ } }
}
function networkRestored() {
  if (native && homeWorkScheduled) void invoke('mesh_network_resume_if_needed').catch(() => {});
}
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!native) return;
  try { error.value = ''; await getCurrentWindow()[action](); await syncWindow(); }
  catch { error.value = t('shell.error.windowAction'); }
}
const taskExit = useTaskExit(native, async () => consentRequired.value || await confirmWorkspaceLeave());
const librarySelection = ref('');
const commandGameId = computed(() => view.value === 'library' ? librarySelection.value : selectedGameId.value);
function focusCurrentSearch() {
  const fields = [...document.querySelectorAll<HTMLInputElement>('[data-list-search], main input[type="search"], .launcher input[type="search"]')];
  const field = fields.find(item => item.getClientRects().length > 0 && !item.disabled);
  field?.focus();
  field?.select();
}
const desktopCommands = computed<CommandAction[]>(() => {
  const zh = locale.value === 'zh-CN';
  return [
    { id: 'game.create', label: zh ? '添加游戏' : 'Add game', icon: 'plus', enabled: !navigationBusy.value, run: async () => { if (await confirmWorkspaceLeave()) { tasksOpen.value = false; showPage({ view: 'create' }); } } },
    { id: 'game.import', label: zh ? '导入整合包' : 'Import modpack', icon: 'folder', enabled: !navigationBusy.value, run: async () => { if (await confirmWorkspaceLeave()) { tasksOpen.value = false; openLocalImport(); } } },
    { id: 'settings.home-layout', label: zh ? '首页布局' : 'Home layout', icon: 'monitor', run: () => openSettingsCategory('home-layout') },
    { id: 'settings.appearance', label: zh ? '外观' : 'Appearance', icon: 'palette', run: () => openSettingsCategory('general') },
    { id: 'palette.open', label: zh ? '快捷操作' : 'Quick actions', icon: 'search', run: () => shellCommands.openPalette() },
    { id: 'game.content', label: zh ? '查找内容' : 'Find content', icon: 'compass', enabled: !!selectedGame.value && !navigationBusy.value, run: () => openGameMarket(selectedGame.value?.loader_version ? 'mod' : 'resource_pack') },
    { id: 'game.readiness', label: zh ? '离线准备' : 'Offline readiness', icon: 'download', enabled: !!selectedGame.value && !navigationBusy.value, run: openReadiness },
  ];
});
const shellCommands = useShellCommands({
  games, page, selectedGameId: commandGameId, busy: navigationBusy,
  ready: computed(() => native && !gamesLoading.value && profilesSettled.value),
  navigate, openSettings: () => navigate('settings'), back: goBack,
  searchGames: focusCurrentSearch, launchGame: startGame,
  onGameUpdated: updateGame,
  extraActions: desktopCommands,
  openGame: async id => { if (await confirmWorkspaceLeave()) manageGame(id); },
  openExtension: openExtensionCommand,
  onError: message => { error.value = message; },
});
const desktopMenuActions = computed(() => workspaceActions(tasksOpen.value ? 'tasks' : view.value, shellCommands.commands.value));
const titlebarTask = computed(() => tasks.value.find(isActiveTask));
const titlebarStatus = computed(() => {
  const task = titlebarTask.value;
  if (task) return `${taskTitleLabel(task)} · ${taskStateLabel(task)}`;
  if (selectedGame.value && ['details', 'running'].includes(view.value)) return selectedGame.value.name;
  const labels: Record<string, string> = locale.value === 'zh-CN'
    ? { library: '游戏库', 'discover-packs': '市场', tools: '工具', extensions: '扩展', settings: '设置', mesh: '联机', profiles: '玩家档案' }
    : { library: 'Games', 'discover-packs': 'Market', tools: 'Tools', extensions: 'Extensions', settings: 'Settings', mesh: 'Multiplayer', profiles: 'Players' };
  return labels[selectedSection.value] ?? 'Cobble';
});
usePluginNavigation({
  native, loading: gamesLoading, busy: navigationBusy,
  hasGame: id => games.value.some(game => game.id === id && game.status === 'installed'),
  refreshGames: loadGames, confirmLeave: confirmWorkspaceLeave,
  hideTasks: () => { tasksOpen.value = false; },
  error: key => { error.value = t(key); },
  openInbox: () => {
    const current = navigation.value.current;
    if (current.view === 'tools' && current.location?.pluginId === 'file-inbox') toolsPage.value?.reload();
    else openToolLocation({ pluginId: 'file-inbox', instanceId: 'gameId' in current ? current.gameId || undefined : current.view === 'tools' ? current.location?.instanceId : undefined });
  },
  openInboxReview: async request => { await suspendToolView(); inboxReviewRequest.value = request; },
  openInboxMarket: link => {
    ++marketRevision.value;
    discoverPacksVisited.value = true;
    showPage({view:'discover-packs',kind:link.kind,gameId:link.gameId ?? '',query:link.project,link});
  },
  openGame: id => manageGame(id, true),
  openGeneratedBlueprint: blueprintId => {
    const current = navigation.value.current;
    openToolLocation({ pluginId: BLUEPRINTS_TOOL_ID, blueprintView: 'library', blueprintId, instanceId: current.view === 'tools' ? current.location?.instanceId : undefined });
  },
  openRecovery: request => showPage(recoveryNavigationPage(request)),
  openHandoff: request => {
    const next = openPluginHandoff(navigation.value, request, id => games.value.some(game => game.id === id && game.status === 'installed'));
    if (next) navigation.value = next;
  },
});
onMounted(() => { applyTheme(theme); void syncWindow(); void guardWindowClose().catch(() => {}); void loadGames(); void loadProfile().finally(() => { profilesSettled.value = true; }); void loadTasks(); void listenSchematics().catch(()=>{}); void listenSnapshotRestore().catch(() => { error.value = t('shell.error.windowAction'); }); void listenLaunchIntents().catch(() => { error.value = t('shell.error.quickLaunch'); }); window.addEventListener('resize', syncWindow); window.addEventListener('online', networkRestored); });
onUnmounted(() => { disposed = true; cancelHomeWork?.(); launchIntents.dispose(); unlistenLaunchIntent?.(); unlistenSnapshotRestore?.(); unlisten?.(); unlistenTasks?.(); unlistenSchematic?.(); unlistenClose?.(); window.removeEventListener('resize', syncWindow); window.removeEventListener('online', networkRestored); });
</script>
<template>
  <div class="launcher">
    <WindowResizeEdges @error="error = t('shell.error.windowAction')" />
    <WorkspaceMenus
      :actions="desktopMenuActions"
      :context-key="`${pageKey(page)}:${tasksOpen}`"
      :disabled="navigationBusy || shellCommands.paletteOpen.value"
      @error="error = $event"
    />
    <CommandPalette
      v-if="shellCommands.paletteOpen.value"
      :open="shellCommands.paletteOpen.value"
      :commands="shellCommands.commands.value"
      @close="shellCommands.closePalette"
      @error="error = $event"
    />
    <TaskExitDialog
      v-if="taskExit.open.value"
      :open="taskExit.open.value"
      :busy="taskExit.busy.value"
      :error="taskExit.error.value"
      @wait="taskExit.keepWaiting"
      @cancel="taskExit.cancelAndExit"
    />
    <header
      class="titlebar"
      data-tauri-drag-region
    >
      <div
        class="drag-area"
        data-tauri-drag-region
      >
        <span
          class="brand-mark"
          aria-hidden="true"
        ><i /><i /><i /><i /></span><span class="brand">cobble<span class="brand-dot">.</span></span>
      </div>
      <CommandStatus
        v-if="view !== 'first-run'"
        :status="titlebarStatus"
        :busy="!!titlebarTask"
        :shortcut="shellCommands.shortcuts.value['palette.open']"
        :open="shellCommands.paletteOpen.value"
        @activate="shellCommands.openPalette"
      />
      <div class="titlebar-actions">
        <MeshStatusButton
          v-if="view !== 'first-run'"
          :active="selectedSection === 'mesh'"
          :before-change="confirmWorkspaceLeave"
          @open="navigate('mesh')"
          @changed="meshNetworkRevision++"
          @contact="openMeshDestination('contacts', $event)"
          @join="openMeshDestination('join')"
          @contacts="openMeshDestination('contacts')"
        />
        <button
          v-if="view !== 'first-run'"
          class="tasks-button"
          :disabled="taskPreviewBusy"
          :aria-label="t('shell.openTasks')"
          :aria-expanded="tasksOpen"
          @click="toggleTasks"
        >
          <svg
            viewBox="0 0 20 20"
            fill="none"
            aria-hidden="true"
          ><path d="M10 3v9m-3-3 3 3 3-3M4 13v4h12v-4" /></svg><span>{{ t('shell.task') }}</span><span
            v-if="activeTaskCount"
            class="task-count"
          >{{ activeTaskCount }}</span>
        </button>
        <div
          v-if="native"
          class="window-controls"
        >
          <button
            :aria-label="t('shell.window.minimize')"
            :title="t('shell.window.minimize')"
            @click="windowAction('minimize')"
          >
            <svg viewBox="0 0 16 16"><path d="M3 8h10" /></svg>
          </button>
          <button
            :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
            :title="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
            @click="windowAction('toggleMaximize')"
          >
            <svg viewBox="0 0 16 16"><path
              v-if="maximized"
              d="M5 5V3h8v8h-2M3 5h8v8H3z"
            /><path
              v-else
              d="M3 3h10v10H3z"
            /></svg>
          </button>
          <button
            class="window-close"
            :aria-label="t('shell.window.close')"
            :title="t('shell.window.close')"
            @click="windowAction('close')"
          >
            <svg viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg>
          </button>
        </div>
      </div>
    </header>
    <AppNavigation
      v-if="view !== 'first-run'"
      :active="selectedSection"
      :disabled="navigationBusy"
      @navigate="navigate"
      @settings="navigate('settings')"
    >
      <ProfileSwitcher
        :library="profileLibrary"
        :busy="profileBusy"
        :ready="profileReady"
        :before-open="suspendToolView"
        @open-change="profileMenuOpen = $event"
        @select="selectProfile"
        @add="openAddProfile"
        @manage="openProfiles"
      />
    </AppNavigation>
    <main class="app-workspace">
      <p
        v-if="availableLauncherUpdate"
        class="launcher-update-notice"
        role="status"
      >
        <button
          class="secondary-button"
          @click="openLauncherUpdate"
        >
          {{ locale === 'zh-CN' ? `Cobble ${availableLauncherUpdate} 可用 · 查看更新` : `Cobble ${availableLauncherUpdate} available · View update` }}
        </button>
      </p>
      <div
        v-if="tasksOpen || (view !== 'library' && view !== 'first-run')"
        class="page-header"
      >
        <button
          v-if="tasksOpen || parentPage"
          class="back-button"
          :aria-label="tasksOpen ? t('shell.back.workspace') : t('shell.back.to', { page: pageTitle(parentPage!) })"
          :disabled="tasksOpen ? taskPreviewBusy : navigationBusy"
          @click="goBack"
        >
          <svg viewBox="0 0 20 20"><path d="m11 4-6 6 6 6M5 10h11" /></svg>{{ tasksOpen ? t('shell.back.workspace') : pageTitle(parentPage!) }}
        </button>
        <h1
          v-if="tasksOpen || (view !== 'details' && view !== 'running')"
          ref="sharedHeading"
          tabindex="-1"
        >
          {{ tasksOpen ? t('shell.task') : pageTitle(page) }}
        </h1>
      </div>
      <div v-show="!tasksOpen">
        <CreatedGamePreferences
          v-if="view === 'library' && createdPreferenceGameId && games.some(game => game.id === createdPreferenceGameId && game.status === 'installed')"
          :key="createdPreferenceGameId"
          :game-id="createdPreferenceGameId"
          @reuse="reuseCreatedGameKeys"
          @dismiss="createdPreferenceGameId = ''"
        />
        <LibraryHome
          v-show="view === 'library'"
          :games="games"
          :loading="gamesLoading"
          :error="gamesError"
          :profile-library="profileLibrary"
          :starting="starting"
          :retrying="retrying"
          :quick-actions="shellCommands.commands.value"
          :quick-actions-loading="shellCommands.extensionsLoading.value"
          @select="librarySelection = $event"
          @saved-market="navigate('discover-packs')"
          @play="startGame"
          @manage="manageGame"
          @retry="retryGame"
          @retry-session-recovery="retryGameSessionRecovery"
          @reload="loadGames"
          @create="showPage({ view: 'create' })"
          @import-pack="openLocalImport"
          @import-launcher="showPage({ view: 'import-launcher' })"
          @receive-game="receiveGameTransfer"
        />
        <GameRunningPage
          v-if="view === 'running' && selectedGame"
          :key="selectedGame.id"
          :game="selectedGame"
          :starting="starting.includes(selectedGame.id)"
          :profile="effectiveProfile(selectedGame, profileLibrary)"
          :tasks="tasks"
          :commands="runningTools.commands.value"
          :tools-loading="runningTools.loading.value"
          :tools-busy="runningTools.resolving.value"
          :tools-error="runningTools.error.value"
          @refresh-tools="runningTools.refresh"
          @manage="manageGame(selectedGame.id, true)"
          @share="openGameGroup"
          @seed="openSeedCompass"
          @extension-command="openRunningTool"
          @play="startGame"
          @retry-session-recovery="retryGameSessionRecovery"
          @login="openProfiles"
          @extensions="openExtensions"
          @runtime="openRuntimeSettings(selectedGame.id)"
          @repaired="updateGame"
        />
        <MeshGroupPanel
          v-if="view === 'mesh-group' && selectedGame"
          :key="selectedGame.id"
          ref="meshGroupPage"
          :game-id="selectedGame.id"
          :game-name="selectedGame.name"
          :running="selectedGame.launch_state === 'running'"
          :active="!tasksOpen"
          :initial-invitation="page.view === 'mesh-group' ? page.invitation : undefined"
          @busy="meshBusy = $event"
          @close="goBack"
          @launch="startGame"
          @created="showPreparedEnvironment"
          @prepared="preparedInvitation = $event"
        />
        <GameDetailsPage
          v-if="detailsVisited && selectedGame"
          v-show="view === 'details'"
          :key="selectedGame.id"
          ref="gameDetailsPage"
          :active="view === 'details' && !tasksOpen"
          :tasks="tasks"
          :game="selectedGame"
          :snapshot-restore="page.view === 'details' ? page.snapshotRestore : undefined"
          :recovery-target="page.view === 'details' ? page.recoveryTarget : undefined"
          :starting="starting.includes(selectedGame.id)"
          :profile="effectiveProfile(selectedGame, profileLibrary)"
          :library="profileLibrary"
          @play="startGame"
          @renamed="updateGame"
          @server-play="startGame"
          @retry-session-recovery="retryGameSessionRecovery"
          @profile-changed="updateGame"
          @action="openGameAction"
          @exchange="openGameExchange"
          @content-busy="contentBusy = $event"
          @readiness="openReadiness"
          @market="openGameMarket"
          @open-game="locateTaskGame"
          @extensions="openExtensions"
          @mesh="openMesh"
          @login="openProfiles"
          @extension-command="openExtensionCommand"
        />
        <GameReadinessPage
          v-if="view === 'readiness' && selectedGame"
          :key="selectedGame.id"
          :game="selectedGame"
          @task="updateTask"
          @tasks="tasksOpen = true"
          @account="openProfiles"
          @runtime="openRuntimeSettings(selectedGame.id)"
        />
        <template v-if="view !== 'library'">
          <FirstRunPage
            v-if="view === 'first-run' && firstRun.state.value"
            :step="firstRun.state.value.step"
            :consent-accepted="firstRun.consentAccepted()"
            :mesh-choice="firstRun.state.value.meshChoice"
            :busy="firstRun.busy.value"
            :error="firstRun.error.value"
            :library="profileLibrary"
            :profile-busy="profileBusy"
            :profile-ready="profileReady"
            :profile-error="profileError"
            @step="setFirstRunStep"
            @accept-consent="acceptFirstRunConsent"
            @mesh-choice="chooseFirstRunMesh"
            @add-profile="openFirstRunFlow('profile')"
            @create-game="openFirstRunFlow('game')"
            @defer="deferFirstRun"
            @complete="completeFirstRun"
            @add-offline="(name, note) => changeProfile('add_offline_profile', { name, note }, true)"
            @select-profile="selectProfile"
            @library-updated="acceptMicrosoftLibrary"
            @auth-busy="authBusy = $event"
          />
          <CreateGamePage
            v-else-if="view === 'create'"
            @created="addCreatedGame"
          />
          <ImportPackPage
            v-else-if="view === 'import'"
            :key="initialPackPreview?.preview_id ?? 'local'"
            :initial-preview="initialPackPreview"
            @curseforge="openCfImport()"
            @created="addCreatedGame"
          />
          <CfPackInstallPage
            v-else-if="view === 'cf-import'"
            :key="initialCfPackPreview?.preview_id ?? 'local-cf'"
            :initial-preview="initialCfPackPreview"
            @created="addCreatedGame"
            @busy="packPreviewBusy = $event"
          />
          <ImportLauncherPage
            v-else-if="view === 'import-launcher'"
            :games="games"
            :tasks="tasks"
            @started="registerImportedGame"
            @open="openImportedGame"
          />
          <GameDirectoryMigrationPage
            v-else-if="page.view === 'move-games'"
            :initial-task-id="page.taskId"
            @back="goBack"
          />
          <GameActionsPage
            v-else-if="view === 'game-action' && selectedGame"
            :game="selectedGame"
            :action="gameAction"
            :starting="starting.includes(selectedGame.id)"
            @busy="gameActionBusy = $event"
            @copy-started="showBackgroundCopy"
            @removed="removeFromLibrary"
            @cancel="goBack"
          />
          <ChangeSharePage
            v-else-if="page.view === 'share-changes' && selectedGame"
            :key="pageKey(page)"
            ref="changeSharePage"
            :game="selectedGame"
            :mode="page.mode"
            @back="goBack"
            @applied="changesApplied"
          />
          <GameTransferPage
            v-else-if="page.view === 'game-transfer'"
            :key="pageKey(page)"
            ref="gameTransferPage"
            :game-id="page.gameId || undefined"
            :initial-task-id="page.taskId"
            :initial-offer-id="page.offerId"
            @back="goBack"
            @open-mesh="navigate('mesh')"
            @open-game="openTransferredGame"
            @task="loadTasks"
            @discarded="loadGames"
          />
          <ProfilesPage
            v-else-if="view === 'profiles'"
            :library="profileLibrary"
            :busy="profileBusy"
            :error="profileError"
            :ready="profileReady"
            :choosing="!!pendingGameId"
            @add="openAddProfile"
            @select="selectProfile"
            @remove="(id) => changeProfile('remove_player_profile', { id })"
            @note="(id, note) => changeProfile('update_profile_note', { id, note })"
            @retry="loadProfile"
          />
          <AddProfilePage
            v-else-if="view === 'add-profile'"
            ref="addProfilePage"
            :library="profileLibrary"
            :busy="profileBusy"
            :error="profileError"
            :ready="profileReady"
            @add="(name, note) => changeProfile('add_offline_profile', { name, note }, true)"
            @select="selectProfile"
            @cancel="leaveProfiles"
            @library-updated="acceptMicrosoftLibrary"
            @auth-busy="authBusy = $event"
          />
          <ToolsPage
            v-else-if="view === 'tools'"
            ref="toolsPage"
            :suspended="tasksOpen || profileMenuOpen || !!inboxReviewRequest || shellCommands.paletteOpen.value || taskExit.open.value"
            :location="toolLocation"
            :games="games"
            :initial-game-id="toolLocation?.instanceId ?? toolLocation?.command?.instanceId ?? ''"
            @navigate="openToolLocation"
            @extension-details="openPluginDetail"
            @market="openBlueprintMarket"
            @command="openExtensionCommand"
            @extensions="openExtensions"
          />
          <PluginsPanel
            v-else-if="view === 'extensions'"
            :candidate="page.view === 'extensions' ? page.candidate : undefined"
            @open-tool="openTool"
            @open-detail="openPluginDetail"
          />
          <PluginDetails
            v-else-if="page.view === 'extension-detail'"
            :key="page.pluginId"
            ref="pluginDetailsPage"
            :plugin-id="page.pluginId"
            @title="pluginDetailTitle"
            @open-tool="openTool"
            @command="openExtensionCommand"
            @open-detail="openPluginDetail"
            @browse-catalog="openExtensions"
            @removed="pluginRemoved"
          />
          <SettingsPage
            v-else-if="view === 'settings'"
            ref="settingsPage"
            :initial-category="initialSettingsCategory"
            :extra-categories="extensionCategories"
            :commands="shellCommands.commands.value"
            :games="games"
            :before-category-leave="leaveExtensionSettings"
            @profiles="openProfiles"
            @first-run="resumeFirstRun()"
            @review-terms="resumeFirstRun(true)"
            @migrate-games="openDirectoryMigration()"
          >
            <template
              v-for="category in extensionCategories"
              :key="category.id"
              #[`category-${category.id}`]
            >
              <ExtensionSettingsPanel
                :ref="panel => registerExtensionPanel(category.id, panel)"
                :plugin-id="category.pluginId"
                :category-id="category.categoryId"
              />
            </template>
          </SettingsPage>
        </template>
        <MeshPage
          v-if="meshVisited"
          v-show="view === 'mesh'"
          ref="meshPage"
          :games="games"
          :initial-game-id="page.view === 'mesh' ? page.gameId : selectedGameId"
          :active="view === 'mesh'"
          :network-revision="meshNetworkRevision"
          :initial-mode="page.view === 'mesh' ? page.initialMode : undefined"
          :initial-contact-id="page.view === 'mesh' ? page.initialContactId : undefined"
          :contact-request-id="page.view === 'mesh' ? page.contactRequestId : undefined"
          @busy="meshBusy = $event"
          @launch="startGame"
          @contact-opened="meshContactOpened"
        />
        <MarketPage
          v-if="discoverPacksVisited"
          v-show="view === 'discover-packs'"
          :key="marketRevision"
          :games="games"
          :initial-kind="marketKind"
          :initial-game-id="marketGameId"
          :initial-query="navigation.market.query"
          :initial-link="navigation.market.link"
          @installed="marketInstalled"
          @locate="locateMarketContent"
          @busy="packPreviewBusy = $event"
          @selected-pack="openOnlineImport"
          @selected-cf-pack="openCfImport"
        />
        <p
          v-if="profileError && view === 'library'"
          role="alert"
          class="window-error"
        >
          {{ profileError }}<button
            v-if="!profileReady"
            class="text-button"
            @click="loadProfile"
          >
            重试
          </button>
        </p>
        <p
          v-if="error"
          role="alert"
          class="window-error"
        >
          {{ error }}
        </p>
      </div>
      <section
        v-show="tasksOpen"
        class="tasks-workspace"
      >
        <PreparedGameJoin
          v-if="preparedInvitation"
          :game="games.find(game => game.id === preparedInvitation!.gameId) ?? null"
          :invitation="preparedInvitation.invitation"
          :disabled="navigationBusy"
          @join="joinPreparedEnvironment"
        />
        <TasksPage
          :tasks="tasks"
          :continue-disabled="navigationBusy"
          :loading="tasksLoading"
          :error="tasksError"
          @updated="updateTask"
          @locate="locateTaskGame"
          @reload="loadTasks"
          @selected-pack="continueTaskPack"
          @selected-cf-pack="continueTaskCfPack"
          @continuing="taskPreviewBusy = $event"
          @open-migration="openDirectoryMigration"
          @open-transfer="openTransferTask"
        />
      </section>
    </main>
    <FileInboxHostReview
      v-if="inboxReviewRequest"
      ref="inboxHostReview"
      :request="inboxReviewRequest"
      @result="acceptInboxResult($event).catch(() => { error = t('shell.error.fileInboxFailed'); })"
      @finished="finishInboxReview"
    />
  </div>
</template>

<style scoped>
.titlebar{display:grid;grid-template-columns:minmax(0,1fr) minmax(160px,360px) minmax(0,1fr);gap:12px}
.titlebar-actions{grid-column:3;display:flex;align-items:center;justify-self:end;min-width:0;gap:2px}
.titlebar .drag-area{min-width:0;align-self:stretch}
@media(max-width:960px){.titlebar{display:flex;gap:10px}.titlebar .drag-area{flex:0 0 auto;padding-left:14px}.titlebar .brand{display:none}.titlebar .command-status{flex:1}.titlebar-actions{flex-shrink:0;margin-left:auto}.titlebar .tasks-button{padding:0 8px}.titlebar .tasks-button>span:not(.task-count){display:none}.titlebar .window-controls button{width:34px}}
</style>

