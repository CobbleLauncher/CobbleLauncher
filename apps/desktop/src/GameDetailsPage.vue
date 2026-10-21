<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, LocalMod } from './games';
import DiscoverMods from './DiscoverMods.vue';

const props = defineProps<{ game: Game; starting: boolean; hasProfile: boolean }>();
const emit = defineEmits<{ play: [id: string]; renamed: [game: Game] }>();
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
const tab = ref<'installed' | 'discover'>('installed');
const installingMod = ref(false);
const contentInstalling = computed(() => installingMod.value || !!props.game.content_installing);
async function installed() { await refresh(); tab.value = 'installed'; }
const query = ref('');
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const deleting = ref<string | null>(null);
let request = 0;
const locked = computed(() => contentInstalling.value || props.starting || props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return mods.value.filter((mod) => [mod.name, mod.file_name, mod.version, mod.loader, ...mod.authors].some((value) => value?.toLocaleLowerCase().includes(search)));
});
async function refresh() {
  const current = ++request;
  loading.value = true;
  error.value = '';
  try {
    const result = await invoke<LocalMod[]>('list_game_mods', { id: props.game.id });
    if (current === request) mods.value = result;
  } catch (cause) { if (current === request) error.value = `无法读取内容：${String(cause)}`; }
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
watch(locked, (value) => { if (value) deleting.value = null; });
watch(() => props.game.id, () => { mods.value = []; busy.value = false; query.value = ''; void refresh(); }, { immediate: true });
onUnmounted(() => { disposed = true; ++request; });
</script>

<template>
  <div class="details-page page-content">
    <div class="details-summary">
      <div>
        <p class="game-version">
          Minecraft {{ game.version }} · {{ game.loader_version ? `Fabric ${game.loader_version}` : '原版' }}
        </p>
        <p
          v-if="game.pack_name"
          class="pack-origin"
        >
          整合包 · {{ game.pack_name }}<span v-if="game.pack_version"> · {{ game.pack_version }}</span>
        </p>
      </div>
      <button
        class="primary-button play-button"
        :disabled="locked || busy"
        @click="$emit('play', game.id)"
      >
        {{ contentInstalling ? '安装 Mod 中' : game.launch_state === 'running' ? '运行中' : locked ? '启动中…' : hasProfile ? '开始游戏' : '设置离线档案' }}
      </button>
    </div>
    <div class="game-management-actions">
      <button
        class="text-button"
        :disabled="actionBusy || editingName"
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
    <section
      class="content-section"
      aria-labelledby="content-heading"
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
          :disabled="contentInstalling"
          aria-controls="installed-content"
          @click="tab = 'installed'"
        >
          已安装 <span class="count">{{ mods.length }}</span>
        </button>
        <button
          id="discover-tab"
          role="tab"
          :aria-selected="tab === 'discover'"
          :disabled="contentInstalling"
          aria-controls="discover-content"
          @click="tab = 'discover'"
        >
          发现 Mod
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
            @click="refresh"
          >
            {{ loading ? '读取中…' : '刷新' }}
          </button>
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
