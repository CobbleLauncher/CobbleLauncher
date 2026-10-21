<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui';
import type { Game, GameAction, LocalMod, PlayerProfile, ProfileLibrary } from './games';
import { gameLoaderLabel } from './games';
import DiscoverMods from './DiscoverMods.vue';
import ModUpdates from './ModUpdates.vue';
import GameRuntimeSettings from './GameRuntimeSettings.vue';

const props = defineProps<{ game: Game; starting: boolean; profile: PlayerProfile | null; library: ProfileLibrary }>();
const emit = defineEmits<{ play: [id: string]; renamed: [game: Game]; profileChanged: [game: Game]; action: [action: GameAction]; contentBusy: [value: boolean] }>();
const pageTab = ref<'content' | 'runtime'>('content');
const profileBindingBusy = ref(false);
const profileBindingError = ref('');
async function bindProfile(event: Event) {
  if (profileBindingBusy.value) return;
  const profileId = (event.target as HTMLSelectElement).value || null;
  profileBindingBusy.value = true;
  profileBindingError.value = '';
  try { emit('profileChanged', await invoke<Game>('set_game_profile', { id: props.game.id, profileId })); }
  catch { (event.target as HTMLSelectElement).value = props.game.profile_id || ''; profileBindingError.value = '未能保存游戏档案，请重试'; }
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
  if (!name || name.length > 80) { actionError.value = '游戏名称需为 1–80 个字符'; return; }
  actionBusy.value = true;
  try {
    const game = await invoke<Game>('rename_game', { id: props.game.id, name });
    if (!disposed) { emit('renamed', game); editingName.value = false; actionStatus.value = '名称已保存'; }
  } catch (cause) { if (!disposed) actionError.value = `无法更改名称：${String(cause)}`; }
  finally { actionBusy.value = false; }
}
async function openLocation(command: 'open_game_directory' | 'open_game_log') {
  if (actionBusy.value) return;
  actionError.value = '';
  actionStatus.value = '';
  actionBusy.value = true;
  try { await invoke(command, { id: props.game.id }); }
  catch (cause) { if (!disposed) actionError.value = `无法打开${command === 'open_game_log' ? '日志' : '文件夹'}：${String(cause)}`; }
  finally { actionBusy.value = false; }
}
const mods = ref<LocalMod[]>([]);
const tab = ref<'installed' | 'discover' | 'updates'>('installed');
const updatingMods = ref(false);
const installingMod = ref(false);
const contentInstalling = computed(() => installingMod.value || updatingMods.value || !!props.game.content_installing);
async function installed() { await refresh(); tab.value = 'installed'; }
const query = ref('');
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const deleting = ref<string | null>(null);
const selectedMods = ref<string[]>([]);
const batchDeleting = ref(false);
const contentBusy = computed(() => busy.value || installingMod.value || updatingMods.value);
watch(() => busy.value || updatingMods.value, (value) => emit('contentBusy', value));
watch(mods, (value) => { selectedMods.value = selectedMods.value.filter((file) => value.some((mod) => mod.file_name === file)); });
const allVisibleSelected = computed(() => filtered.value.length > 0 && filtered.value.every((mod) => selectedMods.value.includes(mod.file_name)));
function selectMod(file: string, checked: boolean) { selectedMods.value = checked ? [...new Set([...selectedMods.value, file])] : selectedMods.value.filter((value) => value !== file); batchDeleting.value = false; }
function selectVisible(checked: boolean) {
  const visible = new Set(filtered.value.map((mod) => mod.file_name));
  selectedMods.value = checked ? [...new Set([...selectedMods.value, ...visible])] : selectedMods.value.filter((file) => !visible.has(file));
  batchDeleting.value = false;
}
async function batch(command: 'set_game_mod_enabled' | 'remove_game_mod', enabled = false) {
  if (locked.value || busy.value || loading.value || !selectedMods.value.length) return;
  const targets = mods.value.filter((mod) => selectedMods.value.includes(mod.file_name));
  busy.value = true; batchDeleting.value = false; deleting.value = null; error.value = '';
  let currentName = '';
  try {
    for (const mod of targets) {
      currentName = mod.name;
      await invoke(command, { id: props.game.id, fileName: mod.file_name, ...(command === 'set_game_mod_enabled' ? { enabled } : {}) });
      selectedMods.value = selectedMods.value.filter((file) => file !== mod.file_name);
    }
  } catch (cause) { error.value = `未能处理 ${currentName}：${String(cause)}`; }
  finally { await refresh(true); busy.value = false; }
}
let request = 0;
const locked = computed(() => contentInstalling.value || props.starting || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return mods.value.filter((mod) => [mod.name, mod.file_name, mod.version, mod.loader, ...mod.authors].some((value) => value?.toLocaleLowerCase().includes(search)));
});
async function refresh(preserveError = false) {
  const current = ++request;
  loading.value = true;
  if (!preserveError) error.value = '';
  try {
    const result = await invoke<LocalMod[]>('list_game_mods', { id: props.game.id });
    if (current === request) mods.value = result;
  } catch (cause) { if (current === request && (!preserveError || !error.value)) error.value = `无法读取内容：${String(cause)}`; }
  finally { if (current === request) loading.value = false; }
}
async function mutate(command: string, args: Record<string, unknown> = {}) {
  if (locked.value || busy.value || loading.value) return;
  const current = ++request;
  busy.value = true;
  error.value = '';
  try {
    const result = await invoke<LocalMod[] | null>(command, { id: props.game.id, ...args });
    if (current === request && result) mods.value = result;
  } catch (cause) { if (current === request) error.value = `无法更新内容：${String(cause)}`; }
  finally { if (current === request) busy.value = false; }
}
function remove(mod: LocalMod) {
  if (locked.value || busy.value || loading.value) return;
  deleting.value = null;
  void mutate('remove_game_mod', { fileName: mod.file_name });
}
watch(locked, (value) => { if (value) { deleting.value = null; batchDeleting.value = false; } });
watch(() => props.game.id, () => { mods.value = []; selectedMods.value = []; batchDeleting.value = false; busy.value = false; query.value = ''; void refresh(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++request; });
</script>

<template>
  <div class="details-page page-content">
    <div class="details-summary">
      <div>
        <p class="game-version">
          Minecraft {{ game.version }} · {{ gameLoaderLabel(game) }}
        </p>
        <button
          class="text-button launch-identity"
          :disabled="contentBusy"
          @click="pageTab = 'runtime'"
        >
          {{ profile ? `使用 ${profile.name} · 离线` : '请选择玩家档案' }} <span aria-hidden="true">⌄</span>
        </button>
        <p
          v-if="game.pack_name"
          class="pack-origin"
        >
          整合包 · {{ game.pack_name }}<span v-if="game.pack_version"> · {{ game.pack_version }}</span>
        </p>
      </div>
      <button
        class="primary-button play-button"
        :disabled="locked || busy || profileBindingBusy"
        @click="$emit('play', game.id)"
      >
        {{ updatingMods ? '更新 Mod 中' : contentInstalling ? '安装 Mod 中' : game.launch_state === 'running' ? '运行中' : locked ? '启动中…' : profile ? `以 ${profile.name} 开始` : '选择玩家档案' }}
      </button>
    </div>
    <div class="game-management-actions">
      <button
        class="text-button"
        :disabled="actionBusy || editingName || contentBusy"
        @click="editName"
      >
        更改名称
      </button>
      <button
        class="text-button"
        :disabled="actionBusy"
        @click="openLocation('open_game_directory')"
      >
        打开游戏文件夹
      </button>
      <button
        class="text-button"
        :disabled="actionBusy || !game.log_path"
        @click="openLocation('open_game_log')"
      >
        打开日志
      </button>
      <DropdownMenuRoot>
        <DropdownMenuTrigger
          class="text-button game-more-trigger"
          aria-label="更多游戏操作"
          :disabled="actionBusy || editingName || contentBusy"
        >
          更多 <span aria-hidden="true">⌄</span>
        </DropdownMenuTrigger>
        <DropdownMenuPortal>
          <DropdownMenuContent
            class="profile-menu game-actions-menu"
            align="end"
            :side-offset="6"
            :collision-padding="12"
          >
            <DropdownMenuItem
              class="profile-menu-item"
              :disabled="locked || busy"
              @select="$emit('action', 'copy')"
            >
              复制游戏
            </DropdownMenuItem>
            <DropdownMenuItem
              class="profile-menu-item"
              :disabled="locked || busy"
              @select="$emit('action', 'export')"
            >
              导出整合包
            </DropdownMenuItem>
            <DropdownMenuSeparator class="profile-menu-separator" />
            <DropdownMenuItem
              class="profile-menu-item danger-text"
              :disabled="locked || busy"
              @select="$emit('action', 'remove')"
            >
              移除游戏
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenuPortal>
      </DropdownMenuRoot>
    </div>
    <form
      v-if="editingName"
      class="rename-form"
      @submit.prevent="saveName"
      @keydown.esc="!actionBusy && (editingName = false)"
    >
      <label for="edited-game-name">游戏名称</label>
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
          {{ actionBusy ? '保存中…' : '保存名称' }}
        </button>
        <button
          class="text-button"
          type="button"
          :disabled="actionBusy"
          @click="editingName = false"
        >
          取消
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
    <p
      v-if="game.launch_error"
      class="game-error"
      role="alert"
    >
      启动失败：{{ game.launch_error }}
    </p>
    <div
      class="content-tabs game-page-tabs"
      role="tablist"
      aria-label="游戏管理"
    >
      <button
        id="game-content-tab"
        role="tab"
        :aria-selected="pageTab === 'content'"
        aria-controls="game-content-panel"
        :disabled="contentBusy"
        @click="pageTab = 'content'"
      >
        内容
      </button>
      <button
        id="game-runtime-tab"
        role="tab"
        :aria-selected="pageTab === 'runtime'"
        aria-controls="game-runtime-panel"
        :disabled="contentBusy"
        @click="pageTab = 'runtime'"
      >
        运行设置
      </button>
    </div>
    <section
      v-if="pageTab === 'runtime'"
      id="game-runtime-panel"
      role="tabpanel"
      aria-labelledby="game-runtime-tab"
      class="runtime-settings"
    >
      <h2>玩家档案</h2><p class="setting-hint">
        默认跟随顶部选择，也可以为这个游戏固定身份。
      </p>
      <div class="form-field">
        <label for="game-player-profile">使用的档案</label><select
          id="game-player-profile"
          :value="game.profile_id || ''"
          :disabled="profileBindingBusy"
          @change="bindProfile"
        >
          <option value="">
            跟随当前档案
          </option><option
            v-if="game.profile_id && !library.profiles.some((item) => item.id === game.profile_id)"
            :value="game.profile_id"
            disabled
          >
            档案已移除，请重新选择
          </option><option
            v-for="item in library.profiles"
            :key="item.id"
            :value="item.id"
          >
            固定 {{ item.name }}
          </option>
        </select>
      </div>
      <p class="setting-hint">
        {{ profile ? `实际使用 ${profile.name} · 离线` : '请选择玩家档案' }}
      </p>
      <p
        v-if="profileBindingError"
        role="alert"
        class="settings-error"
      >
        {{ profileBindingError }}
      </p>
      <GameRuntimeSettings
        :game="game"
        @updated="$emit('profileChanged', $event)"
      />
    </section>
    <section
      v-show="pageTab === 'content'"
      id="game-content-panel"
      role="tabpanel"
      aria-labelledby="game-content-tab"
      class="content-section"
    >
      <div
        class="content-tabs"
        role="tablist"
        aria-label="Mod 内容"
      >
        <button
          id="installed-tab"
          role="tab"
          :aria-selected="tab === 'installed'"
          :disabled="contentInstalling || busy"
          aria-controls="installed-content"
          @click="tab = 'installed'"
        >
          已安装 <span class="count">{{ mods.length }}</span>
        </button>
        <button
          id="discover-tab"
          role="tab"
          :aria-selected="tab === 'discover'"
          :disabled="contentInstalling || busy"
          aria-controls="discover-content"
          @click="tab = 'discover'"
        >
          发现 Mod
        </button>
        <button
          id="updates-tab"
          role="tab"
          :aria-selected="tab === 'updates'"
          :disabled="contentInstalling || busy"
          aria-controls="updates-content"
          @click="tab = 'updates'"
        >
          更新
        </button>
      </div>
      <DiscoverMods
        v-if="tab === 'discover'"
        id="discover-content"
        role="tabpanel"
        aria-labelledby="discover-tab"
        :game="game"
        :locked="locked || busy"
        @installing="installingMod = $event"
        @installed="installed"
      />
      <ModUpdates
        v-show="tab === 'updates'"
        id="updates-content"
        role="tabpanel"
        aria-labelledby="updates-tab"
        :game="game"
        :locked="locked || busy || loading"
        @busy="updatingMods = $event"
        @updated="refresh()"
      />
      <div
        v-show="tab === 'installed'"
        id="installed-content"
        role="tabpanel"
        aria-labelledby="installed-tab"
      >
        <div class="content-heading">
          <h2 id="content-heading">
            本地 Mod <span class="count">{{ mods.length }}</span>
          </h2>
          <button
            class="secondary-button"
            :disabled="locked || busy || loading"
            @click="mutate('import_game_mods')"
          >
            {{ busy ? '处理中…' : '导入 Mod' }}
          </button>
        </div>
        <p
          v-if="locked && !contentInstalling"
          class="setting-hint"
          role="status"
        >
          游戏启动或运行期间无法修改内容，请关闭游戏后再试。
        </p>
        <p
          v-else-if="!game.loader_version"
          class="setting-hint"
        >
          原版不会加载 Mod。需要使用 Fabric 等模组加载器。
        </p>
        <div class="content-toolbar">
          <input
            v-model="query"
            type="search"
            aria-label="搜索本地 Mod"
            placeholder="搜索名称、版本或作者"
            :disabled="loading"
          >
          <button
            class="text-button"
            :disabled="busy || loading"
            @click="refresh()"
          >
            {{ loading ? '读取中…' : '刷新' }}
          </button>
        </div>
        <div
          v-if="mods.length"
          class="mod-selection-toolbar installed-selection"
        >
          <label class="mod-selection-label"><input
            type="checkbox"
            aria-label="全选当前列表"
            :checked="allVisibleSelected"
            :indeterminate="filtered.some((mod) => selectedMods.includes(mod.file_name)) && !allVisibleSelected"
            :disabled="locked || busy || loading || !filtered.length"
            @change="selectVisible(($event.target as HTMLInputElement).checked)"
          ><span>{{ selectedMods.length ? `已选择 ${selectedMods.length} 个` : '选择 Mod' }}</span></label>
          <div
            v-if="selectedMods.length"
            class="mod-batch-actions"
          >
            <button
              class="text-button"
              :disabled="locked || busy || loading"
              @click="batch('set_game_mod_enabled', true)"
            >
              启用所选
            </button><button
              class="text-button"
              :disabled="locked || busy || loading"
              @click="batch('set_game_mod_enabled', false)"
            >
              停用所选
            </button><button
              class="text-button danger-text"
              :disabled="locked || busy || loading"
              @click="batchDeleting = true; deleting = null"
            >
              移除所选
            </button>
          </div>
        </div>
        <div
          v-if="batchDeleting"
          class="mod-batch-confirm"
        >
          <p>移除选中的 {{ selectedMods.length }} 个 Mod？游戏和存档会保留。</p><div class="profile-form-actions">
            <button
              class="secondary-button danger-text"
              :disabled="locked || busy || loading"
              @click="batch('remove_game_mod')"
            >
              确认移除所选
            </button><button
              class="text-button"
              :disabled="busy"
              @click="batchDeleting = false"
            >
              取消
            </button>
          </div>
        </div>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }}
        </p>
        <p
          v-if="loading"
          class="content-empty"
          role="status"
        >
          正在读取本地内容…
        </p>
        <ul
          v-else-if="filtered.length"
          class="mod-list"
          aria-label="本地 Mod"
        >
          <li
            v-for="mod in filtered"
            :key="mod.file_name"
            class="mod-row"
          >
            <input
              class="mod-selection-checkbox"
              type="checkbox"
              :aria-label="`选择 ${mod.name}`"
              :checked="selectedMods.includes(mod.file_name)"
              :disabled="locked || busy || loading"
              @change="selectMod(mod.file_name, ($event.target as HTMLInputElement).checked)"
            >
            <div
              class="mod-info"
              :class="{ 'mod-disabled': !mod.enabled }"
            >
              <h3>
                {{ mod.name }}<span
                  v-if="!mod.enabled"
                  class="mod-state"
                >已禁用</span>
                <span
                  v-if="mod.pack_owned || game.pack_name"
                  class="mod-state"
                >{{ mod.pack_owned ? '整合包自带' : '自行添加' }}</span>
              </h3>
              <p class="mod-meta">
                {{ mod.version || '版本未知' }} · {{ mod.loader || '加载器未知' }}<span v-if="mod.authors.length"> · {{ mod.authors.join('、') }}</span>
              </p>
              <p
                v-if="mod.description"
                class="mod-description"
              >
                {{ mod.description }}
              </p>
              <p class="mod-filename">
                {{ mod.file_name }}
              </p>
            </div>
            <div class="mod-actions">
              <button
                class="mod-toggle"
                role="switch"
                :aria-label="`启用 ${mod.name}`"
                :aria-checked="mod.enabled"
                :disabled="locked || busy || loading"
                @click="mutate('set_game_mod_enabled', { fileName: mod.file_name, enabled: !mod.enabled })"
              >
                <span />
              </button>
              <template v-if="deleting === mod.file_name">
                <button
                  class="text-button"
                  :disabled="locked || busy || loading"
                  @click="remove(mod)"
                >
                  确认删除
                </button>
                <button
                  class="text-button"
                  :disabled="busy"
                  @click="deleting = null"
                >
                  取消
                </button>
              </template>
              <button
                v-else
                class="text-button"
                :aria-label="`删除 ${mod.name}`"
                :disabled="locked || busy || loading"
                @click="deleting = mod.file_name"
              >
                删除
              </button>
            </div>
          </li>
        </ul>
        <p
          v-else
          class="content-empty"
        >
          {{ query.trim() ? '没有找到匹配的 Mod。' : '还没有 Mod，导入本地 .jar 文件开始。' }}
        </p>
      </div>
    </section>
  </div>
</template>
