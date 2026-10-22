<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { applyTheme, readTheme } from './theme';
import CreateGamePage from './CreateGamePage.vue';
import ImportPackPage from './ImportPackPage.vue';
import ImportLauncherPage from './ImportLauncherPage.vue';
import DiscoverPacksPage from './DiscoverPacksPage.vue';
import SettingsPage from './SettingsPage.vue';
import GameCard from './GameCard.vue';
import GameDetailsPage from './GameDetailsPage.vue';
import GameActionsPage from './GameActionsPage.vue';
import ProfileSwitcher from './ProfileSwitcher.vue';
import ProfilesPage from './ProfilesPage.vue';
import AddProfilePage from './AddProfilePage.vue';
import { effectiveProfile, type Game, type GameAction, type PackPreview, type ProfileLibrary } from './games';
const games = ref<Game[]>([]);
const profileLibrary = ref<ProfileLibrary>({ profiles: [], active_id: null });
const profileReady = ref(false);
const profileBusy = ref(false);
const authBusy = ref(false);
const addProfilePage = ref<InstanceType<typeof AddProfilePage> | null>(null);
const profileError = ref('');
const pendingGameId = ref<string | null>(null);
const profileOrigin = ref<'library' | 'details'>('library');
const addOrigin = ref<'library' | 'details' | 'profiles'>('library');
const starting = ref<string[]>([]);
const gameRevisions = new Map<string, number>();
async function loadProfile() {
  if (!native) { profileReady.value = true; return; }
  profileError.value = '';
  try { profileLibrary.value = await invoke<ProfileLibrary>('list_player_profiles'); profileReady.value = true; }
  catch { profileError.value = '无法读取玩家档案，请重试'; }
}
function openProfiles() {
  profileOrigin.value = view.value === 'details' ? 'details' : 'library';
  profileError.value = '';
  view.value = 'profiles';
}
function openAddProfile() {
  addOrigin.value = view.value === 'profiles' ? 'profiles' : view.value === 'details' ? 'details' : 'library';
  profileError.value = '';
  view.value = 'add-profile';
}
async function leaveProfiles() {
  if (profileBusy.value || authBusy.value) return;
  profileError.value = '';
  if (view.value === 'add-profile') {
    if (addProfilePage.value && !await addProfilePage.value.cancelLogin()) return;
    await loadProfile();
  }
  if (pendingGameId.value) { selectedGameId.value = pendingGameId.value; pendingGameId.value = null; view.value = 'details'; }
  else view.value = view.value === 'add-profile' ? addOrigin.value : profileOrigin.value;
}
async function finishProfileChoice(returnToGame: boolean) {
  if (returnToGame && pendingGameId.value) {
    const game = games.value.find((game) => game.id === pendingGameId.value);
    if (game?.profile_id) updateGame(await invoke<Game>('set_game_profile', { id: game.id, profileId: profileLibrary.value.active_id }));
    selectedGameId.value = pendingGameId.value;
    pendingGameId.value = null;
    view.value = 'details';
  } else if (returnToGame && view.value === 'add-profile') view.value = addOrigin.value;
}
async function acceptMicrosoftLibrary(library: ProfileLibrary) {
  if (profileBusy.value) return;
  profileLibrary.value = library; profileError.value = ''; profileBusy.value = true;
  try { await finishProfileChoice(true); }
  catch { profileError.value = '档案已保存，请重新选择游戏使用的档案。'; view.value = 'profiles'; }
  finally { profileBusy.value = false; }
}
async function changeProfile(command: string, args: Record<string, unknown>, returnToGame = false) {
  if (profileBusy.value || !profileReady.value) return;
  profileError.value = '';
  if (!native) { profileError.value = '请在桌面应用中保存玩家档案'; return; }
  profileBusy.value = true;
  try {
    profileLibrary.value = await invoke<ProfileLibrary>(command, args);
    await finishProfileChoice(returnToGame);
  } catch { profileError.value = '未能保存档案，请重试'; }
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
}
async function startGame(id: string) {
  const current = games.value.find((game) => game.id === id);
  if (!current || current.content_installing || starting.value.includes(id) || current.launch_state === 'preparing' || current.launch_state === 'running') return;
  if (!profileReady.value || !effectiveProfile(current, profileLibrary.value)) {
    pendingGameId.value = id; selectedGameId.value = id; profileOrigin.value = 'details';
    if (profileLibrary.value.profiles.length || !profileReady.value) view.value = 'profiles';
    else { addOrigin.value = 'details'; view.value = 'add-profile'; }
    return;
  }
  starting.value.push(id);
  const revision = gameRevisions.get(id) ?? 0;
  try {
    if (!native) throw new Error('请在桌面应用中启动游戏');
    const game = await invoke<Game>('start_game', { id });
    if ((gameRevisions.get(id) ?? 0) === revision) updateGame(game);
  } catch (cause) {
    if ((gameRevisions.get(id) ?? 0) === revision) updateGame({ ...current, launch_state: 'idle', launch_error: String(cause) });
  } finally { starting.value = starting.value.filter((item) => item !== id); }
}
function addCreatedGame(game: Game) {
  view.value = 'library';
  if (!games.value.some((item) => item.id === game.id)) games.value.unshift(game);
}
async function loadGames() {
  if (!native) return;
  try {
    const stop = await listen<Game>('game-install', (event) => updateGame(event.payload));
    if (disposed) { stop(); return; }
    unlisten = stop;
    const stored = await invoke<Game[]>('list_games');
    for (const game of stored) if (!games.value.some((item) => item.id === game.id)) games.value.push(game);
  } catch { error.value = '无法读取游戏库，请重新打开应用'; }
}
async function retryGame(id: string) {
  if (retrying.value.includes(id)) return;
  retrying.value.push(id);
  try {
    const game = await invoke<Game>('retry_game', { id });
    if (games.value.find((item) => item.id === id)?.status === 'failed') updateGame(game);
  }
  catch (cause) { error.value = `无法重试安装：${String(cause)}`; }
  finally { retrying.value = retrying.value.filter((item) => item !== id); }
}
const view = ref<'library' | 'create' | 'import' | 'import-launcher' | 'settings' | 'details' | 'profiles' | 'add-profile' | 'game-action' | 'discover-packs'>('library');
const launcherImportBusy = ref(false);
const discoverPacksVisited = ref(false);
const packPreviewBusy = ref(false);
const importOrigin = ref<'library' | 'discover-packs'>('library');
const initialPackPreview = ref<PackPreview | null>(null);
function openDiscoverPacks() { discoverPacksVisited.value = true; view.value = 'discover-packs'; }
function openLocalImport() { initialPackPreview.value = null; importOrigin.value = 'library'; view.value = 'import'; }
function openOnlineImport(preview: PackPreview) { initialPackPreview.value = preview; importOrigin.value = 'discover-packs'; view.value = 'import'; }
const gameAction = ref<GameAction>('copy');
const gameActionBusy = ref(false);
const contentBusy = ref(false);
const gameActionTitle = computed(() => ({ copy: '复制游戏', export: '导出整合包', remove: '移除游戏' })[gameAction.value]);
const selectedGameId = ref('');
const selectedGame = computed(() => games.value.find((game) => game.id === selectedGameId.value));
function manageGame(id: string) {
  if (games.value.find((game) => game.id === id)?.status !== 'installed') return;
  selectedGameId.value = id;
  view.value = 'details';
}
function openGameAction(action: GameAction) {
  const game = selectedGame.value;
  if (!game || game.status !== 'installed' || game.content_installing || starting.value.includes(game.id) || game.launch_state === 'preparing' || game.launch_state === 'running') return;
  gameAction.value = action;
  view.value = 'game-action';
}
function removeFromLibrary(id: string) {
  games.value = games.value.filter((game) => game.id !== id);
  gameRevisions.delete(id);
  selectedGameId.value = '';
  if (pendingGameId.value === id) pendingGameId.value = null;
  view.value = 'library';
}
function goBack() {
  if (profileBusy.value || authBusy.value || gameActionBusy.value || contentBusy.value || packPreviewBusy.value || launcherImportBusy.value) return;
  if (view.value === 'import') view.value = importOrigin.value;
  else if (view.value === 'game-action') view.value = 'details';
  else if (view.value === 'profiles' || view.value === 'add-profile') leaveProfiles();
  else view.value = 'library';
}
watch(view, async () => {
  await nextTick();
  document.querySelector<HTMLElement>('main h1')?.focus();
  window.scrollTo(0, 0);
});
const theme = readTheme();
const native = isTauri();
const maximized = ref(false);
const error = ref('');
async function syncWindow() {
  if (native) { try { maximized.value = await getCurrentWindow().isMaximized(); } catch { /* Actions surface errors. */ } }
}
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!native) return;
  try { error.value = ''; await getCurrentWindow()[action](); await syncWindow(); }
  catch { error.value = '窗口操作失败，请重试'; }
}
onMounted(() => { applyTheme(theme); void syncWindow(); void loadGames(); void loadProfile(); window.addEventListener('resize', syncWindow); });
onUnmounted(() => { disposed = true; unlisten?.(); window.removeEventListener('resize', syncWindow); });
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
      <div
        v-if="native"
        class="window-controls"
      >
        <button
          aria-label="最小化"
          title="最小化"
          @click="windowAction('minimize')"
        >
          <svg viewBox="0 0 16 16"><path d="M3 8h10" /></svg>
        </button>
        <button
          :aria-label="maximized ? '还原' : '最大化'"
          :title="maximized ? '还原' : '最大化'"
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
          aria-label="关闭窗口"
          title="关闭窗口"
          @click="windowAction('close')"
        >
          <svg viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg>
        </button>
      </div>
    </header>
    <main>
      <template v-if="view === 'library'">
        <div class="library-header">
          <div>
            <p class="overline">
              MINECRAFT JAVA
            </p><h1 tabindex="-1">
              游戏库<span class="count">{{ games.length }}</span>
            </h1>
          </div>
          <div class="library-actions">
            <ProfileSwitcher
              :library="profileLibrary"
              :busy="profileBusy"
              :ready="profileReady"
              @select="selectProfile"
              @add="openAddProfile"
              @manage="openProfiles"
            />
            <button
              class="settings-button"
              aria-label="打开设置"
              title="设置"
              @click="view = 'settings'"
            >
              <svg viewBox="0 0 24 24"><path d="M4 7h16M4 17h16" /><circle
                cx="9"
                cy="7"
                r="3"
              /><circle
                cx="15"
                cy="17"
                r="3"
              /></svg>
            </button>
          </div>
        </div>
        <section
          v-if="!games.length"
          class="empty-library"
          aria-labelledby="empty-title"
        >
          <div
            class="island-scene"
            aria-hidden="true"
          >
            <svg
              viewBox="0 0 360 250"
              fill="none"
            ><ellipse
               class="island-shadow"
               cx="180"
               cy="214"
               rx="102"
               ry="16"
             /><path
               class="orbit"
               d="M32 153 180 70l148 83-148 82z"
             />
              <path
                class="stone-left"
                d="m75 143 105 60v-63L75 80z"
              /><path
                class="stone-right"
                d="m180 203 105-60V80l-105 60z"
              /><path
                class="grass-top"
                d="m75 80 105-60 105 60-105 60z"
              /><path
                class="grass-left"
                d="m75 80 105 60v18L75 98z"
              /><path
                class="grass-right"
                d="m180 140 105-60v18l-105 60z"
              />
              <path
                class="stone-detail"
                d="m90 125 24 14v17l-24-14zm42 37 28 16v16l-28-16zm75-3 24-14v17l-24 14zm40-39 23-13v17l-23 13z"
              />
              <path
                class="trunk-left"
                d="m173 77 9 5v34l-9-5z"
              /><path
                class="trunk-right"
                d="m182 82 9-5v34l-9 5z"
              />
              <path
                class="leaf-left"
                d="m142 38 40 23v42l-40-23z"
              /><path
                class="leaf-right"
                d="m182 61 40-23v42l-40 23z"
              /><path
                class="leaf-top"
                d="m142 38 40-23 40 23-40 23z"
              />
              <path
                class="flower"
                d="m116 87 7-4 7 4-7 4zm119 8 7-4 7 4-7 4z"
              /><path
                class="grass-stroke"
                d="M123 91v9m119-1v9"
              />
              <path
                class="spark"
                d="M70 40v12m-6-6h12M284 42v8m-4-4h8"
              />
            </svg>
          </div>
          <h2 id="empty-title">
            还没有游戏
          </h2><p class="empty-description">
            创建新游戏，或导入已有的游戏与整合包。
          </p>
          <div class="start-actions">
            <button
              class="primary-button"
              @click="view = 'create'"
            >
              <span aria-hidden="true">＋</span>创建游戏
            </button><button
              class="secondary-button"
              @click="openLocalImport"
            >
              <svg viewBox="0 0 20 20"><path d="M10 3v9m-4-4 4 4 4-4M4 13v4h12v-4" /></svg>导入整合包
            </button>
            <button
              class="secondary-button"
              @click="openDiscoverPacks"
            >
              发现整合包
            </button>
            <button
              class="secondary-button"
              @click="view = 'import-launcher'"
            >
              导入已有游戏
            </button>
          </div>
        </section>
        <section
          v-else
          class="game-library"
          aria-label="已创建的游戏"
        >
          <GameCard
            v-for="game in games"
            :key="game.id"
            :game="game"
            :retrying="retrying.includes(game.id)"
            :profile="effectiveProfile(game, profileLibrary)"
            :starting="starting.includes(game.id)"
            @retry="retryGame"
            @play="startGame"
            @manage="manageGame"
          />
          <button
            class="add-game-button"
            @click="view = 'create'"
          >
            <span aria-hidden="true">＋</span> 创建游戏
          </button>
          <button
            class="add-game-button"
            @click="openLocalImport"
          >
            <span aria-hidden="true">↓</span> 导入整合包
          </button>
          <button
            class="add-game-button"
            @click="openDiscoverPacks"
          >
            发现整合包
          </button>
          <button
            class="add-game-button"
            @click="view = 'import-launcher'"
          >
            导入已有游戏
          </button>
        </section>
      </template>
      <template v-else>
        <div class="page-header">
          <button
            class="back-button"
            :aria-label="view === 'import' && importOrigin === 'discover-packs' ? '返回发现整合包' : view === 'game-action' ? '返回游戏详情' : view === 'profiles' || view === 'add-profile' ? '返回' : '返回游戏库'"
            :disabled="profileBusy || authBusy || gameActionBusy || contentBusy || packPreviewBusy || launcherImportBusy"
            @click="goBack"
          >
            <svg viewBox="0 0 20 20"><path d="m11 4-6 6 6 6M5 10h11" /></svg>{{ view === 'import' && importOrigin === 'discover-packs' ? '发现整合包' : view === 'game-action' ? '游戏详情' : view === 'profiles' || view === 'add-profile' ? '返回' : '游戏库' }}
          </button>
          <h1 tabindex="-1">
            {{ view === 'create' ? '创建游戏' : view === 'import' ? '导入整合包' : view === 'import-launcher' ? '导入已有游戏' : view === 'discover-packs' ? '发现整合包' : view === 'details' ? selectedGame?.name : view === 'game-action' ? gameActionTitle : view === 'profiles' ? '玩家档案' : view === 'add-profile' ? '添加玩家档案' : '设置' }}
          </h1>
        </div>
        <CreateGamePage
          v-if="view === 'create'"
          @created="addCreatedGame"
        />
        <ImportPackPage
          v-else-if="view === 'import'"
          :initial-preview="initialPackPreview"
          @created="addCreatedGame"
        />
        <ImportLauncherPage
          v-else-if="view === 'import-launcher'"
          @created="addCreatedGame"
          @busy="launcherImportBusy = $event"
        />
        <GameDetailsPage
          v-else-if="view === 'details' && selectedGame"
          :game="selectedGame"
          :starting="starting.includes(selectedGame.id)"
          :profile="effectiveProfile(selectedGame, profileLibrary)"
          :library="profileLibrary"
          @play="startGame"
          @renamed="updateGame"
          @profile-changed="updateGame"
          @action="openGameAction"
          @content-busy="contentBusy = $event"
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
        <SettingsPage
          v-else-if="view === 'settings'"
          @saved="view = 'library'"
        />
      </template>
      <DiscoverPacksPage
        v-if="discoverPacksVisited"
        v-show="view === 'discover-packs'"
        @busy="packPreviewBusy = $event"
        @selected-pack="openOnlineImport"
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
    </main>
  </div>
</template>
