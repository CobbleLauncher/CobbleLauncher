<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import WorldRestore from './WorldRestore.vue';
import type { LocalWorld } from './games';
import type { WorldBackup, WorldRestoreResult } from './world-backups';
const props = defineProps<{ gameId: string; locked: boolean; revision?: number }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const worlds = ref<LocalWorld[]>([]);
const query = ref('');
const reading = ref(false);
const readError = ref('');
const opening = ref<string | null>(null);
const exporting = ref<string | null>(null);
const exported = ref<string | null>(null);
const actionError = ref<{ folder: string; message: string } | null>(null);
const restoreOpen = ref(false);
const restoreBusy = ref(false);
const backups = ref<WorldBackup[]>([]);
const readingBackups = ref(false);
const backupError = ref('');
const selectedBackup = ref<Pick<WorldBackup, 'backup_id' | 'folder_name' | 'world_name'> | null>(null);
const rollingBack = ref(false);
const rollbackError = ref('');
const restoreResult = ref(false);
const lastRollback = ref<Pick<WorldBackup, 'backup_id' | 'folder_name' | 'world_name'> | null>(null);
const worldBusy = computed(() => !!exporting.value || restoreBusy.value || rollingBack.value);
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return worlds.value.filter((world) => [world.name, world.folder_name].some((value) => value.toLocaleLowerCase().includes(search)));
});
let request = 0;
let backupRequest = 0;
function modeLabel(mode: number | null): string | null { return mode == null ? null : ({ 0: '生存', 1: '创造', 2: '冒险', 3: '旁观' } as Record<number, string>)[mode] ?? null; }
function playedLabel(value: number | null): string | null {
  if (value == null) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }).format(date);
}
async function load() {
  if (worldBusy.value) return;
  const current = ++request;
  reading.value = true; readError.value = '';
  try { const value = await invoke<LocalWorld[]>('list_game_worlds', { id: props.gameId }); if (current === request) worlds.value = value; }
  catch (cause) { if (current === request) readError.value = `无法读取世界：${String(cause)}`; }
  finally { if (current === request) reading.value = false; }
}
async function loadBackups() {
  const current = ++backupRequest;
  readingBackups.value = true; backupError.value = '';
  try {
    const value = await invoke<WorldBackup[]>('list_game_world_backups', { id: props.gameId });
    if (current === backupRequest) backups.value = Array.isArray(value) ? value : [];
  } catch (cause) { if (current === backupRequest) backupError.value = `无法读取世界备份：${String(cause)}`; }
  finally { if (current === backupRequest) readingBackups.value = false; }
}
function refresh() { void load(); void loadBackups(); }
function openRestore() {
  if (props.locked || worldBusy.value || reading.value) return;
  restoreResult.value = false; lastRollback.value = null; selectedBackup.value = null; rollbackError.value = ''; restoreOpen.value = true;
}
function setRestoreBusy(value: boolean) { restoreBusy.value = value; emit('busy', value); }
function restored(value: WorldRestoreResult, folder: string) {
  const name = worlds.value.find((world) => world.folder_name === folder)?.name ?? value.worlds.find((world) => world.folder_name === folder)?.name ?? folder;
  worlds.value = value.worlds; restoreResult.value = true; restoreOpen.value = false; selectedBackup.value = null; exported.value = null; actionError.value = null;
  lastRollback.value = value.rollback_id ? { backup_id: value.rollback_id, folder_name: folder, world_name: name } : null;
  void loadBackups();
}
function selectBackup(backup: Pick<WorldBackup, 'backup_id' | 'folder_name' | 'world_name'>) {
  if (props.locked || worldBusy.value) return;
  selectedBackup.value = backup; rollbackError.value = '';
}
async function rollback() {
  if (!selectedBackup.value || props.locked || worldBusy.value) return;
  const backup = selectedBackup.value;
  rollingBack.value = true; rollbackError.value = ''; restoreResult.value = false; emit('busy', true);
  try { restored(await invoke<WorldRestoreResult>('rollback_game_world_restore', { id: props.gameId, rollbackId: backup.backup_id }), backup.folder_name); }
  catch (cause) { rollbackError.value = `恢复备份未完成：${String(cause)}`; }
  finally { rollingBack.value = false; emit('busy', false); }
}
async function openFolder(world: LocalWorld) {
  if (opening.value) return;
  opening.value = world.folder_name; actionError.value = null;
  try { await invoke('open_game_world_directory', { id: props.gameId, folderName: world.folder_name }); }
  catch (cause) { actionError.value = { folder: world.folder_name, message: `无法打开世界文件夹：${String(cause)}` }; }
  finally { opening.value = null; }
}
async function exportWorld(world: LocalWorld) {
  if (props.locked || worldBusy.value || reading.value) return;
  exporting.value = world.folder_name; exported.value = null; actionError.value = null; emit('busy', true);
  try { if (await invoke<boolean>('export_game_world', { id: props.gameId, folderName: world.folder_name })) exported.value = world.folder_name; }
  catch (cause) { actionError.value = { folder: world.folder_name, message: `无法导出世界备份：${String(cause)}` }; }
  finally { exporting.value = null; emit('busy', false); }
}
watch(() => props.gameId, () => { worlds.value = []; backups.value = []; query.value = ''; exported.value = null; actionError.value = null; restoreOpen.value = false; selectedBackup.value = null; restoreResult.value = false; lastRollback.value = null; rollbackError.value = ''; refresh(); }, { immediate: true });
watch(() => props.revision, () => { restoreOpen.value = false; selectedBackup.value = null; restoreResult.value = false; lastRollback.value = null; rollbackError.value = ''; actionError.value = null; exported.value = null; refresh(); });
onUnmounted(() => { ++request; ++backupRequest; });
</script>
<template>
  <div class="game-worlds">
    <WorldRestore
      v-if="restoreOpen"
      :game-id="gameId"
      :worlds="worlds"
      :locked="locked"
      @busy="setRestoreBusy"
      @cancel="restoreOpen = false"
      @restored="restored"
    />
    <template v-else>
      <div class="content-heading">
        <h2>本地世界 <span class="count">{{ worlds.length }}</span></h2><div class="world-heading-actions">
          <button
            class="secondary-button"
            :disabled="locked || worldBusy || reading"
            @click="openRestore"
          >
            导入世界备份
          </button>
          <button
            class="text-button"
            :disabled="reading || readingBackups || worldBusy"
            @click="refresh"
          >
            {{ reading ? '读取中…' : '刷新' }}
          </button>
        </div>
      </div>
      <p
        v-if="locked && !worldBusy"
        class="setting-hint"
      >
        关闭游戏并等待当前任务完成后，即可导出或恢复备份。
      </p>
      <div class="content-toolbar world-search">
        <input
          v-model="query"
          type="search"
          aria-label="搜索世界"
          placeholder="搜索世界名称或文件夹"
          :disabled="worldBusy"
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
              :disabled="locked || worldBusy || reading"
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
      <div
        v-if="restoreResult"
        class="world-restore-result"
      >
        <p
          class="world-action-result"
          role="status"
        >
          世界已恢复
        </p><button
          v-if="lastRollback"
          class="text-button"
          :disabled="locked || worldBusy"
          @click="selectBackup(lastRollback)"
        >
          恢复替换前的世界
        </button>
      </div>
      <section
        v-if="selectedBackup"
        class="world-backup-confirm"
        aria-label="恢复备份确认"
      >
        <h3>恢复 {{ selectedBackup.world_name }} 的备份</h3><p>将替换这个世界的当前内容。当前世界会先另存为备份。</p><div class="profile-form-actions">
          <button
            class="secondary-button danger-text"
            :disabled="locked || worldBusy"
            @click="rollback"
          >
            {{ rollingBack ? '恢复中…' : '确认恢复备份' }}
          </button><button
            class="text-button"
            :disabled="worldBusy"
            @click="selectedBackup = null; rollbackError = ''"
          >
            取消
          </button>
        </div><p
          v-if="rollbackError"
          class="settings-error"
          role="alert"
        >
          {{ rollbackError }}
        </p>
      </section>
      <section
        v-if="backups.length || backupError"
        class="world-backups"
        aria-label="替换前的备份"
      >
        <h2>替换前的备份</h2><p class="setting-hint">
          替换世界时保留，可随时恢复。
        </p><ul
          v-if="backups.length"
          class="world-backup-list"
        >
          <li
            v-for="backup in backups"
            :key="backup.backup_id"
          >
            <div class="mod-info">
              <h3>{{ backup.world_name }}</h3><p
                v-if="playedLabel(backup.created_at)"
                class="mod-meta"
              >
                {{ playedLabel(backup.created_at) }}
              </p>
            </div><button
              class="text-button"
              :aria-label="`恢复备份 ${backup.world_name}`"
              :disabled="locked || worldBusy"
              @click="selectBackup(backup)"
            >
              恢复备份
            </button>
          </li>
        </ul><div
          v-if="backupError"
          class="inline-error"
          role="alert"
        >
          <p>{{ backupError }}</p><button
            class="text-button"
            :disabled="readingBackups || worldBusy"
            @click="loadBackups"
          >
            重试读取备份
          </button>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.world-heading-actions{display:flex;align-items:center;gap:17px}.world-heading-actions .secondary-button{min-height:36px;padding:0 13px;font-size:11px}.world-restore-result{display:flex;align-items:center;justify-content:flex-end;gap:17px;margin-top:15px;flex-wrap:wrap}.world-restore-result .world-action-result{margin:0}.world-restore-result .text-button{color:var(--ink)}.world-backup-confirm{margin-top:20px;padding:17px 18px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}.world-backup-confirm h3{font-size:14px;font-weight:600;margin:0;overflow-wrap:anywhere}.world-backup-confirm>p{font-size:12px;line-height:1.8;color:var(--muted);margin-top:9px;overflow-wrap:anywhere}.world-backup-confirm .profile-form-actions{margin-top:13px}.world-backup-confirm .secondary-button{min-height:36px;padding:0 14px;font-size:11px}.world-backups{border-top:1px solid var(--line);margin-top:27px;padding-top:23px}.world-backups>h2{font-size:15px;font-weight:600}.world-backups>.setting-hint{margin-top:7px;font-size:11px}.world-backup-list{list-style:none;padding:0;margin:10px 0 0}.world-backup-list>li{display:flex;align-items:center;gap:15px;padding:14px 0;border-bottom:1px solid var(--line)}.world-backup-list h3{font-size:13px;font-weight:500;margin:0;overflow-wrap:anywhere}.world-backup-list .text-button{flex-shrink:0}@media(max-width:560px){.world-heading-actions{gap:12px}.world-heading-actions .secondary-button{padding:0 11px}.world-backup-confirm{padding:15px}.world-backups{margin-top:24px}}
</style>
