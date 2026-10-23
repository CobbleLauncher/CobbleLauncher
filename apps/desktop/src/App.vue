<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { lazyPage } from './lazy-page';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createCloseGuard } from './window-close';
import { createPendingLaunchIntentConsumer, type PendingLaunchIntent } from './pending-launch-intent';
import { applyTheme, readTheme } from './theme';
const CreateGamePage = lazyPage(() => import('./CreateGamePage.vue'));
const ImportPackPage = lazyPage(() => import('./ImportPackPage.vue'));
const CfPackInstallPage = lazyPage(() => import('./CfPackInstallPage.vue'));
import type { CurseForgePackPreview } from './curseforge';
const ImportLauncherPage = lazyPage(() => import('./ImportLauncherPage.vue'));
const MarketPage = lazyPage(() => import('./MarketPage.vue'));
import type { MarketKind } from './market';
import { pendingBlueprint, type PendingBlueprintRequest } from './blueprints';
const SettingsPage = lazyPage(() => import('./SettingsPage.vue'));
import LibraryHome from './LibraryHome.vue';
import MeshStatusButton from './MeshStatusButton.vue';
import AppNavigation from './AppNavigation.vue';
import { activeSection, extensionDetailPage, backPage, enterPage, goToParent, initialNavigation, pageKey, rootPage, returnToGameDetails, toolPage, type AppPage, type ExtensionCandidate, type MainSection, type ToolLocation } from './navigation';
import { BLUEPRINTS_TOOL_ID } from './tools';
const PluginsPanel = lazyPage(() => import('./PluginsPanel.vue'));
const PluginDetails = lazyPage(() => import('./PluginDetails.vue'));
const ExtensionSettingsPanel = lazyPage(() => import('./ExtensionSettingsPanel.vue'));
import { pluginLocale, type ExtensionContribution, type ResolvedCommand } from './extensions/contributions';
const GameDetailsPage = lazyPage(() => import('./GameDetailsPage.vue'));
const CreatedGamePreferences = lazyPage(() => import('./CreatedGamePreferences.vue'));
const GameActionsPage = lazyPage(() => import('./GameActionsPage.vue'));
const MeshPage = lazyPage(() => import('./MeshPage.vue'));
import { t, locale } from './i18n';
import ProfileSwitcher from './ProfileSwitcher.vue';
const ProfilesPage = lazyPage(() => import('./ProfilesPage.vue'));
const AddProfilePage = lazyPage(() => import('./AddProfilePage.vue'));
const TasksPage = lazyPage(() => import('./TasksPage.vue'));
import { isActiveTask, type TaskSnapshot } from './tasks';
import { effectiveProfile, type Game, type GameAction, type PackPreview, type ProfileLibrary } from './games';
const ToolsPage = lazyPage(() => import('./ToolsPage.vue'));
const games = ref<Game[]>([]);
const gamesLoading = ref(true);
const gamesError = ref('');
const tasks = ref<TaskSnapshot[]>([]);
const tasksOpen = ref(false);
const profileMenuOpen = ref(false);
const toolsPage = ref<{ suspend: () => Promise<void>; resume: () => void; confirmLeave: () => Promise<boolean> } | null>(null);
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
  if (!tasksVisible && !profileVisible && view.value === 'tools') toolsPage.value?.resume();
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
async function guardWindowClose() {
  if (!native) return;
  const stop = await getCurrentWindow().onCloseRequested(createCloseGuard(confirmWorkspaceLeave,()=>{error.value=t('shell.error.windowAction');}));
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
  if (previous && ['failed', 'cancelled', 'interrupted'].includes(previous.state) && task.state === 'running') retriedTasks.add(task.id);
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
  navigation.value=goToParent(navigation.value);
  if (view.value==='details') pendingGameId.value=null;
}
async function finishProfileChoice(returnToGame: boolean) {
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
  if (index < 0) games.value.unshift(game);
  else games.value[index] = game;
  if (game.launch_error && starting.value.includes(game.id) && view.value === 'library' && !tasksOpen.value) {
    manageGame(game.id);
  }
}
async function startGame(id: string, serverId?: string) {
  if (tasks.value.some(task => task.game_id === id && task.kind === 'game_repair' && isActiveTask(task))) return;
  const current = games.value.find((game) => game.id === id);
  if (!current || current.content_installing || starting.value.includes(id) || current.launch_state === 'preparing' || current.launch_state === 'running') return;
  if (!profileReady.value || !effectiveProfile(current, profileLibrary.value)) {
    if (!await confirmWorkspaceLeave()) return;
    pendingGameId.value = id;
    pendingServerId.value = serverId;
    if (view.value!=='details' || selectedGameId.value!==id) showPage({view:'details',gameId:id});
    showPage({view:profileLibrary.value.profiles.length || !profileReady.value ? 'profiles' : 'add-profile'});
    return;
  }
  starting.value.push(id);
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
function addCreatedGame(game: Game) {
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
const navigation = ref(initialNavigation());
const page = computed(() => navigation.value.current);
const view = computed(() => page.value.view);
const detailsVisited = ref(false);
watch(view, value => { if (value === 'details') detailsVisited.value = true; }, { flush: 'sync' });
function showPage(next: AppPage, mode: 'push' | 'replace' | 'root' = 'push') { navigation.value=enterPage(navigation.value,next,mode); }
const meshVisited = ref(false);
const meshBusy = ref(false);
const meshPage=ref<{confirmLeave:()=>Promise<boolean>}|null>(null);
async function openMesh() { if (!await confirmWorkspaceLeave()) return; meshVisited.value = true; showPage({view:'mesh',gameId:selectedGameId.value}); }
const pluginDetailsPage=ref<{confirmLeave:()=>Promise<boolean>}|null>(null);
const settingsPage = ref<InstanceType<typeof SettingsPage> | null>(null);
async function confirmWorkspaceLeave(): Promise<boolean> {
  if (view.value === 'tools') return await toolsPage.value?.confirmLeave() ?? true;
  if (view.value === 'mesh') return await meshPage.value?.confirmLeave() ?? true;
  if (view.value === 'extension-detail') return await pluginDetailsPage.value?.confirmLeave() ?? true;
  if (view.value === 'settings') return await settingsPage.value?.confirmLeave() ?? true;
  if (view.value === 'details') return await gameDetailsPage.value?.confirmLeave() ?? true;
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
const navigationBusy = computed(() => taskPreviewBusy.value || profileBusy.value || authBusy.value || gameActionBusy.value || contentBusy.value || packPreviewBusy.value || launcherImportBusy.value || meshBusy.value);
async function navigate(section: MainSection) {
  if (navigationBusy.value) return;
  if (!await confirmWorkspaceLeave()) return;
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
const launcherImportBusy = ref(false);
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
function ensureMarketParent() { if(view.value!=='discover-packs') showPage(navigation.value.market,'root'); }
function openCfImport(preview: CurseForgePackPreview | null = null) { initialCfPackPreview.value = preview; if(preview) ensureMarketParent(); showPage({view:'cf-import'},view.value==='import'?'replace':'push'); }
function openOnlineImport(preview: PackPreview) { initialPackPreview.value = preview; ensureMarketParent(); showPage({view:'import'}); }
const gameAction = computed<GameAction>(()=>page.value.view==='game-action'?page.value.action:'copy');
const gameActionBusy = ref(false);
const contentBusy = ref(false);
const selectedGameId = computed(()=> 'gameId' in page.value && page.value.gameId ? page.value.gameId : navigation.value.lastGameId);
const selectedGame = computed(() => games.value.find((game) => game.id === selectedGameId.value));
function manageGame(id: string) {
  if (games.value.find((game) => game.id === id)?.status !== 'installed') return;
  showPage({view:'details',gameId:id});
}
async function locateTaskGame(id: string) {
  if (!games.value.some(game => game.id === id && game.status === 'installed')) return;
  if (!await confirmWorkspaceLeave()) return;
  tasksOpen.value = false;
  manageGame(id);
}
async function openGameAction(action: GameAction) {
  if (!await confirmWorkspaceLeave()) return;
  const game = selectedGame.value;
  if (!game || game.status !== 'installed' || game.content_installing || starting.value.includes(game.id) || game.launch_state === 'preparing' || game.launch_state === 'running') return;
  showPage({view:'game-action',gameId:game.id,action});
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
  if (profileBusy.value || authBusy.value || gameActionBusy.value || contentBusy.value || packPreviewBusy.value || launcherImportBusy.value || meshBusy.value) return;
  if (view.value === 'profiles' || view.value === 'add-profile') await leaveProfiles();
  else navigation.value=goToParent(navigation.value);
}
const parentPage = computed(()=>backPage(navigation.value));
function pageTitle(value:AppPage):string {
  switch(value.view) {
    case 'library':return t('shell.back.libraryLabel');
    case 'details':return games.value.find(game=>game.id===value.gameId)?.name ?? t('shell.back.gameDetailsLabel');
    case 'tools':return value.location?.title ?? value.location?.command?.label ?? (value.location?.blueprintView==='online'?t('blueprintOnline.online'):value.location?.pluginId===BLUEPRINTS_TOOL_ID?t('tools.blueprintsName'):t('navigation.tools'));
    case 'extension-detail':return value.title ?? t('navigation.extensions');
    case 'extensions':return t('navigation.extensions');
    case 'discover-packs':return t('shell.page.market');
    case 'game-action':return ({copy:t('shell.action.copy'),export:t('shell.action.export'),remove:t('shell.action.remove')})[value.action];
    case 'mesh':return t('mesh.title');
    case 'create':return t('shell.page.create');
    case 'import':case 'cf-import':return t('shell.page.importPack');
    case 'import-launcher':return t('shell.page.importLauncher');
    case 'profiles':return t('shell.page.profiles');
    case 'add-profile':return t('shell.page.addProfile');
    case 'settings':return t('shell.page.settings');
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
let unlistenLaunchIntent: UnlistenFn | undefined;
const launchIntents = createPendingLaunchIntentConsumer({
  peek: () => invoke<PendingLaunchIntent | null>('peek_pending_launch'),
  acknowledge: queueId => invoke<boolean>('acknowledge_pending_launch', { queueId }),
  hasGame: id => games.value.some(game => game.id === id),
  confirmWorkspaceLeave,
  openGame: id => { tasksOpen.value = false; manageGame(id); },
  startGame: async id => { tasksOpen.value = false; manageGame(id); await startGame(id); },
  missingGame: () => { error.value = t('shell.error.quickLaunchUnavailable'); },
  failed: cause => { error.value = t(String(cause).includes('QUICK_LAUNCH_GAME_UNAVAILABLE') ? 'shell.error.quickLaunchUnavailable' : 'shell.error.quickLaunch'); },
});
watch([gamesLoading, profilesSettled], ([loading, settled]) => {
  if (native && !loading && settled && !gamesError.value) launchIntents.markReady();
});
async function listenLaunchIntents() {
  if (!native) return;
  const stop = await listen('launch-intent', () => launchIntents.notify());
  if (disposed) stop(); else { unlistenLaunchIntent = stop; launchIntents.notify(); }
}
async function syncWindow() {
  if (native) { try { maximized.value = await getCurrentWindow().isMaximized(); } catch { /* Actions surface errors. */ } }
}
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!native) return;
  try { error.value = ''; await getCurrentWindow()[action](); await syncWindow(); }
  catch { error.value = t('shell.error.windowAction'); }
}
onMounted(() => { applyTheme(theme); void syncWindow(); void guardWindowClose().catch(() => {}); void loadGames(); void loadProfile().finally(() => { profilesSettled.value = true; }); void loadTasks(); void listenSchematics().catch(()=>{}); void listenLaunchIntents().catch(() => { error.value = t('shell.error.quickLaunch'); }); window.addEventListener('resize', syncWindow); });
onUnmounted(() => { disposed = true; launchIntents.dispose(); unlistenLaunchIntent?.(); unlisten?.(); unlistenTasks?.(); unlistenSchematic?.(); unlistenClose?.(); window.removeEventListener('resize', syncWindow); });
</script>
<template>
  <div class="launcher">
    <header class="titlebar">
      <div
        class="drag-area"
        data-tauri-drag-region
      >
        <span
          class="brand-mark"
          aria-hidden="true"
        ><i /><i /><i /><i /></span><span class="brand">cobble<span class="brand-dot">.</span></span>
      </div>
      <MeshStatusButton
        :active="selectedSection === 'mesh'"
        :before-change="confirmWorkspaceLeave"
        @open="navigate('mesh')"
        @changed="meshNetworkRevision++"
      />
      <button
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
    </header>
    <AppNavigation
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
      <div
        v-if="tasksOpen || view !== 'library'"
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
          v-if="tasksOpen || view !== 'details'"
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
          @play="startGame"
          @manage="manageGame"
          @retry="retryGame"
          @reload="loadGames"
          @create="showPage({ view: 'create' })"
          @import-pack="openLocalImport"
          @import-launcher="showPage({ view: 'import-launcher' })"
        />
        <GameDetailsPage
          v-if="detailsVisited && selectedGame"
          v-show="view === 'details'"
          :key="selectedGame.id"
          ref="gameDetailsPage"
          :active="view === 'details' && !tasksOpen"
          :tasks="tasks"
          :game="selectedGame"
          :starting="starting.includes(selectedGame.id)"
          :profile="effectiveProfile(selectedGame, profileLibrary)"
          :library="profileLibrary"
          @play="startGame"
          @renamed="updateGame"
          @server-play="startGame"
          @profile-changed="updateGame"
          @action="openGameAction"
          @content-busy="contentBusy = $event"
          @market="openGameMarket"
          @extensions="openExtensions"
          @mesh="openMesh"
          @login="openProfiles"
          @extension-command="openExtensionCommand"
        />
        <template v-if="view !== 'library'">
          <CreateGamePage
            v-if="view === 'create'"
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
            @created="addCreatedGame"
            @busy="launcherImportBusy = $event"
          />
          <GameActionsPage
            v-else-if="view === 'game-action' && selectedGame"
            :game="selectedGame"
            :action="gameAction"
            :starting="starting.includes(selectedGame.id)"
            @busy="gameActionBusy = $event"
            @copied="addCreatedGame"
            @removed="removeFromLibrary"
            @cancel="goBack"
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
            :suspended="tasksOpen || profileMenuOpen"
            :location="toolLocation"
            :games="games"
            :initial-game-id="toolLocation?.instanceId ?? toolLocation?.command?.instanceId ?? ''"
            @navigate="openToolLocation"
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
            :extra-categories="extensionCategories"
            :before-category-leave="leaveExtensionSettings"
            @profiles="openProfiles"
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
          @busy="meshBusy = $event"
          @launch="startGame"
        />
        <MarketPage
          v-if="discoverPacksVisited"
          v-show="view === 'discover-packs'"
          :key="marketRevision"
          :games="games"
          :initial-kind="marketKind"
          :initial-game-id="marketGameId"
          :initial-query="navigation.market.query"
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
        />
      </section>
    </main>
  </div>
</template>

