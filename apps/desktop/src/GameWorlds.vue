<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { LocalWorld } from './games';
const props = defineProps<{ gameId: string; locked: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const worlds = ref<LocalWorld[]>([]);
const query = ref('');
const reading = ref(false);
const readError = ref('');
const opening = ref<string | null>(null);
const exporting = ref<string | null>(null);
const exported = ref<string | null>(null);
const actionError = ref<{ folder: string; message: string } | null>(null);
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return worlds.value.filter((world) => [world.name, world.folder_name].some((value) => value.toLocaleLowerCase().includes(search)));
});
let request = 0;
function modeLabel(mode: number | null): string | null { return mode == null ? null : ({ 0: '生存', 1: '创造', 2: '冒险', 3: '旁观' } as Record<number, string>)[mode] ?? null; }
function playedLabel(value: number | null): string | null {
  if (value == null) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }).format(date);
}
async function load() {
  if (exporting.value) return;
  const current = ++request;
  reading.value = true; readError.value = '';
  try { const value = await invoke<LocalWorld[]>('list_game_worlds', { id: props.gameId }); if (current === request) worlds.value = value; }
  catch (cause) { if (current === request) readError.value = `无法读取世界：${String(cause)}`; }
  finally { if (current === request) reading.value = false; }
}
async function openFolder(world: LocalWorld) {
  if (opening.value) return;
  opening.value = world.folder_name; actionError.value = null;
  try { await invoke('open_game_world_directory', { id: props.gameId, folderName: world.folder_name }); }
  catch (cause) { actionError.value = { folder: world.folder_name, message: `无法打开世界文件夹：${String(cause)}` }; }
  finally { opening.value = null; }
}
async function exportWorld(world: LocalWorld) {
  if (props.locked || exporting.value || reading.value) return;
  exporting.value = world.folder_name; exported.value = null; actionError.value = null; emit('busy', true);
  try { if (await invoke<boolean>('export_game_world', { id: props.gameId, folderName: world.folder_name })) exported.value = world.folder_name; }
  catch (cause) { actionError.value = { folder: world.folder_name, message: `无法导出世界备份：${String(cause)}` }; }
  finally { exporting.value = null; emit('busy', false); }
}
watch(() => props.gameId, () => { worlds.value = []; query.value = ''; exported.value = null; actionError.value = null; void load(); }, { immediate: true });
onUnmounted(() => { ++request; });
</script>
<template>
  <div class="game-worlds">
    <div class="content-heading">
      <h2>本地世界 <span class="count">{{ worlds.length }}</span></h2><button
        class="text-button"
        :disabled="reading || !!exporting"
        @click="load"
      >
        {{ reading ? '读取中…' : '刷新' }}
      </button>
    </div>
    <p
      v-if="locked && !exporting"
      class="setting-hint"
    >
      关闭游戏并等待当前任务完成后，即可导出备份。
    </p>
    <div class="content-toolbar world-search">
      <input
        v-model="query"
        type="search"
        aria-label="搜索世界"
        placeholder="搜索世界名称或文件夹"
        :disabled="!!exporting"
      >
    </div>
    <div
      v-if="readError"
      class="inline-error"
      role="alert"
    >
      <p>{{ readError }}</p><button
        class="text-button"
        :disabled="reading"
        @click="load"
      >
        重试读取
      </button>
    </div>
    <p
      v-if="reading && !worlds.length"
      class="content-empty"
      role="status"
    >
      正在读取世界…
    </p>
    <ul
      v-else-if="filtered.length"
      class="world-list"
      aria-label="本地世界"
    >
      <li
        v-for="world in filtered"
        :key="world.folder_name"
        class="world-row"
      >
        <div class="world-row-main">
          <span
            class="world-icon"
            aria-hidden="true"
          ><svg viewBox="0 0 32 32"><path d="m5 10 11-6 11 6-11 6zM5 10v12l11 6 11-6V10M16 16v12M5 15l11 6 11-6" /></svg></span><div class="mod-info">
            <h3>
              {{ world.name }}<span
                v-if="world.hardcore"
                class="mod-state"
              >极限</span>
            </h3><div
              v-if="world.minecraft_version || modeLabel(world.game_mode)"
              class="world-metadata"
            >
              <span v-if="world.minecraft_version">Minecraft {{ world.minecraft_version }}</span><span v-if="modeLabel(world.game_mode)">{{ modeLabel(world.game_mode) }}</span>
            </div><p
              v-if="playedLabel(world.last_played)"
              class="mod-meta"
            >
              上次游玩 {{ playedLabel(world.last_played) }}
            </p><p
              v-if="world.metadata_error"
              class="world-metadata-warning"
            >
              无法读取世界信息
            </p><p
              v-if="world.folder_name !== world.name"
              class="mod-filename"
            >
              {{ world.folder_name }}
            </p>
          </div>
        </div><div class="world-actions">
          <button
            class="text-button"
            :aria-label="`打开 ${world.name} 文件夹`"
            :disabled="!!opening"
            @click="openFolder(world)"
          >
            打开文件夹
          </button><button
            class="secondary-button"
            :aria-label="`导出 ${world.name} 的备份`"
            :disabled="locked || !!exporting || reading"
            @click="exportWorld(world)"
          >
            <span
              v-if="exporting === world.folder_name"
              class="busy-spinner"
              aria-hidden="true"
            />{{ exporting === world.folder_name ? '导出中…' : '导出备份' }}
          </button>
        </div><p
          v-if="exported === world.folder_name"
          class="world-action-result"
          role="status"
        >
          备份已导出
        </p><p
          v-if="actionError?.folder === world.folder_name"
          class="settings-error"
          role="alert"
        >
          {{ actionError.message }}
        </p>
      </li>
    </ul>
    <p
      v-else-if="!readError"
      class="content-empty"
    >
      {{ query.trim() ? '没有找到匹配的世界。' : '还没有本地世界。在游戏中创建世界后，会显示在这里。' }}
    </p>
  </div>
</template>
