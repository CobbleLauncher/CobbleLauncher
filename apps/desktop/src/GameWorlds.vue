<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { TaskSnapshot } from './tasks';
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import WorldRestore from './WorldRestore.vue';
import AutomaticWorldBackup from './AutomaticWorldBackup.vue';
import WorldDatapacks from './WorldDatapacks.vue';
import WorldSnapshotsPanel from './WorldSnapshotsPanel.vue';
import { locale, t } from './i18n';
import type { LocalWorld } from './games';
import type { WorldBackup, WorldRestoreResult } from './world-backups';
const props = defineProps<{ tasks?: TaskSnapshot[]; gameId: string; locked: boolean; revision?: number }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const worlds = ref<LocalWorld[]>([]);
const recentBackups = ref<Record<string, string>>({});
const query = ref('');
const reading = ref(false);
const readError = ref('');
const opening = ref<string | null>(null);
const exporting = ref<string | null>(null);
const exported = ref<string | null>(null);
const actionError = ref<{ folder: string; message: string } | null>(null);
const restoreOpen = ref(false);
const restoreBusy = ref(false);
const datapackWorld = ref<LocalWorld | null>(null);
const snapshotWorld = ref<LocalWorld | null>(null);
const snapshotBusy = ref(false);
function setSnapshotBusy(value: boolean) { snapshotBusy.value = value; emit('busy', value); }
const backups = ref<WorldBackup[]>([]);
const readingBackups = ref(false);
const backupError = ref('');
const selectedBackup = ref<Pick<WorldBackup, 'backup_id' | 'folder_name' | 'world_name'> | null>(null);
const rollingBack = ref(false);
const rollbackError = ref('');
const restoreResult = ref(false);
const lastRollback = ref<Pick<WorldBackup, 'backup_id' | 'folder_name' | 'world_name'> | null>(null);
const worldBusy = computed(() => !!exporting.value || restoreBusy.value || rollingBack.value || snapshotBusy.value);
const filtered = computed(() => {
  const search = query.value.trim().toLocaleLowerCase();
  return worlds.value.filter((world) => [world.name, world.folder_name].some((value) => value.toLocaleLowerCase().includes(search)));
});
let request = 0;
let backupRequest = 0;
function modeLabel(mode: number | null): string | null {
  const key = mode == null ? undefined : ({ 0: 'worlds.survival', 1: 'worlds.creative', 2: 'worlds.adventure', 3: 'worlds.spectator' } as const)[mode as 0 | 1 | 2 | 3];
  return key ? t(key) : null;
}
function playedLabel(value: number | null): string | null {
  if (value == null) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : new Intl.DateTimeFormat(locale.value, { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }).format(date);
}
async function load() {
  if (worldBusy.value) return;
  const current = ++request;
  reading.value = true; readError.value = '';
  try { const value = await invoke<LocalWorld[]>('list_game_worlds', { id: props.gameId }); if (current === request) worlds.value = value; }
  catch (cause) { if (current === request) readError.value = t('worlds.readError', { error: String(cause) }); }
  finally { if (current === request) reading.value = false; }
}
async function loadBackups() {
  const current = ++backupRequest;
  readingBackups.value = true; backupError.value = '';
  try {
    const value = await invoke<WorldBackup[]>('list_game_world_backups', { id: props.gameId });
    if (current === backupRequest) backups.value = Array.isArray(value) ? value : [];
  } catch (cause) { if (current === backupRequest) backupError.value = t('worlds.backupReadError', { error: String(cause) }); }
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
  catch (cause) { rollbackError.value = t('worlds.rollbackError', { error: String(cause) }); }
  finally { rollingBack.value = false; emit('busy', false); }
}
async function openFolder(world: LocalWorld) {
  if (opening.value) return;
  opening.value = world.folder_name; actionError.value = null;
  try { await invoke('open_game_world_directory', { id: props.gameId, folderName: world.folder_name }); }
  catch (cause) { actionError.value = { folder: world.folder_name, message: t('worlds.openFolderError', { error: String(cause) }) }; }
  finally { opening.value = null; }
}
async function exportWorld(world: LocalWorld) {
  if (props.locked || worldBusy.value || reading.value) return;
  exporting.value = world.folder_name; exported.value = null; actionError.value = null; emit('busy', true);
  try { if (await invoke<boolean>('export_game_world', { id: props.gameId, folderName: world.folder_name })) exported.value = world.folder_name; }
  catch (cause) { actionError.value = { folder: world.folder_name, message: t('worlds.exportError', { error: String(cause) }) }; }
  finally { exporting.value = null; emit('busy', false); }
}
watch(() => props.gameId, () => { datapackWorld.value = null; snapshotWorld.value = null; worlds.value = []; backups.value = []; query.value = ''; exported.value = null; actionError.value = null; restoreOpen.value = false; selectedBackup.value = null; restoreResult.value = false; lastRollback.value = null; rollbackError.value = ''; refresh(); }, { immediate: true });
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
    <WorldDatapacks
      v-else-if="datapackWorld"
      :key="`${gameId}:${datapackWorld.folder_name}`"
      :game-id="gameId"
      :world="datapackWorld"
      :locked="locked"
      @busy="setSnapshotBusy"
      @close="datapackWorld = null"
    />
    <WorldSnapshotsPanel
      v-else-if="snapshotWorld"
      :key="`${gameId}:${snapshotWorld.folder_name}`"
      :tasks="tasks"
      :game-id="gameId"
      :world="snapshotWorld"
      :locked="locked"
      @busy="setSnapshotBusy"
      @close="snapshotWorld = null"
      @restored="restored"
    />
    <template v-else>
      <div class="content-heading">
        <h2>{{ t('worlds.title') }} <span class="count">{{ worlds.length }}</span></h2><div class="world-heading-actions">
          <button
            class="secondary-button"
            :disabled="locked || worldBusy || reading"
            @click="openRestore"
          >
            {{ t('worlds.importBackup') }}
          </button>
          <button
            class="text-button"
            :disabled="reading || readingBackups || worldBusy"
            @click="refresh"
          >
            {{ reading ? t('worlds.loading') : t('worlds.refresh') }}
          </button>
        </div>
      </div>
      <p
        v-if="locked && !worldBusy"
        class="setting-hint"
      >
        {{ t('worlds.locked') }}
      </p>
      <AutomaticWorldBackup
        v-if="worlds.length"
        :game-id="gameId"
        :revision="revision"
        @recent="recentBackups = $event"
      />
      <div class="content-toolbar world-search">
        <input
          v-model="query"
          type="search"
          :aria-label="t('worlds.searchLabel')"
          :placeholder="t('worlds.searchPlaceholder')"
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
          {{ t('worlds.retryRead') }}
        </button>
      </div>
      <p
        v-if="reading && !worlds.length"
        class="content-empty"
        role="status"
      >
        {{ t('worlds.loadingWorlds') }}
      </p>
      <ul
        v-else-if="filtered.length"
        class="world-list"
        :aria-label="t('worlds.title')"
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
                >{{ t('worlds.hardcore') }}</span>
              </h3><div
                v-if="world.minecraft_version || modeLabel(world.game_mode)"
                class="world-metadata"
              >
                <span v-if="world.minecraft_version">Minecraft {{ world.minecraft_version }}</span><span v-if="modeLabel(world.game_mode)">{{ modeLabel(world.game_mode) }}</span>
              </div><p
                v-if="playedLabel(world.last_played)"
                class="mod-meta"
              >
                {{ t('worlds.lastPlayed', { date: playedLabel(world.last_played) }) }}
              </p><p
                v-if="recentBackups[world.folder_name]"
                class="mod-meta"
              >
                {{ t('worlds.autoRecent') }} · {{ playedLabel(new Date(recentBackups[world.folder_name] ?? '').getTime()) }}
              </p><p
                v-if="world.metadata_error"
                class="world-metadata-warning"
              >
                {{ t('worlds.metadataError') }}
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
              :disabled="worldBusy || reading"
              @click="datapackWorld = world"
            >
              {{ t('worlds.datapacks') }}
            </button>
            <button
              class="text-button"
              :disabled="worldBusy || reading"
              @click="snapshotWorld = world"
            >
              {{ t('worldSnapshots.history') }}
            </button>
            <button
              class="text-button"
              :aria-label="t('worlds.openFolder', { name: world.name })"
              :disabled="!!opening"
              @click="openFolder(world)"
            >
              {{ t('worlds.openFolderAction') }}
            </button><button
              class="secondary-button"
              :aria-label="t('worlds.exportBackupFor', { name: world.name })"
              :disabled="locked || worldBusy || reading"
              @click="exportWorld(world)"
            >
              <span
                v-if="exporting === world.folder_name"
                class="busy-spinner"
                aria-hidden="true"
              />{{ exporting === world.folder_name ? t('worlds.exporting') : t('worlds.exportBackup') }}
            </button>
          </div><p
            v-if="exported === world.folder_name"
            class="world-action-result"
            role="status"
          >
            {{ t('worlds.backupExported') }}
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
        {{ query.trim() ? t('worlds.noMatches') : t('worlds.empty') }}
      </p>
      <div
        v-if="restoreResult"
        class="world-restore-result"
      >
        <p
          class="world-action-result"
          role="status"
        >
          {{ t('worlds.restored') }}
        </p><button
          v-if="lastRollback"
          class="text-button"
          :disabled="locked || worldBusy"
          @click="selectBackup(lastRollback)"
        >
          {{ t('worlds.restorePrevious') }}
        </button>
      </div>
      <section
        v-if="selectedBackup"
        class="world-backup-confirm"
        :aria-label="t('worlds.rollbackConfirmLabel')"
      >
        <h3>{{ t('worlds.rollbackTitle', { name: selectedBackup.world_name }) }}</h3><p>{{ t('worlds.rollbackWarning') }}</p><div class="profile-form-actions">
          <button
            class="secondary-button danger-text"
            :disabled="locked || worldBusy"
            @click="rollback"
          >
            {{ rollingBack ? t('worlds.restoring') : t('worlds.confirmRollback') }}
          </button><button
            class="text-button"
            :disabled="worldBusy"
            @click="selectedBackup = null; rollbackError = ''"
          >
            {{ t('worlds.cancel') }}
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
        :aria-label="t('worlds.previousBackups')"
      >
        <h2>{{ t('worlds.previousBackups') }}</h2><p class="setting-hint">
          {{ t('worlds.previousBackupsHint') }}
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
              :aria-label="t('worlds.restoreBackupFor', { name: backup.world_name })"
              :disabled="locked || worldBusy"
              @click="selectBackup(backup)"
            >
              {{ t('worlds.restoreBackup') }}
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
            {{ t('worlds.retryBackups') }}
          </button>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.world-heading-actions{display:flex;align-items:center;gap:17px}.world-heading-actions .secondary-button{min-height:36px;padding:0 13px;font-size:11px}.world-restore-result{display:flex;align-items:center;justify-content:flex-end;gap:17px;margin-top:15px;flex-wrap:wrap}.world-restore-result .world-action-result{margin:0}.world-restore-result .text-button{color:var(--ink)}.world-backup-confirm{margin-top:20px;padding:17px 18px;border:1px solid var(--line);border-radius:12px;background:var(--panel)}.world-backup-confirm h3{font-size:14px;font-weight:600;margin:0;overflow-wrap:anywhere}.world-backup-confirm>p{font-size:12px;line-height:1.8;color:var(--muted);margin-top:9px;overflow-wrap:anywhere}.world-backup-confirm .profile-form-actions{margin-top:13px}.world-backup-confirm .secondary-button{min-height:36px;padding:0 14px;font-size:11px}.world-backups{border-top:1px solid var(--line);margin-top:27px;padding-top:23px}.world-backups>h2{font-size:15px;font-weight:600}.world-backups>.setting-hint{margin-top:7px;font-size:11px}.world-backup-list{list-style:none;padding:0;margin:10px 0 0}.world-backup-list>li{display:flex;align-items:center;gap:15px;padding:14px 0;border-bottom:1px solid var(--line)}.world-backup-list h3{font-size:13px;font-weight:500;margin:0;overflow-wrap:anywhere}.world-backup-list .text-button{flex-shrink:0}@media(max-width:560px){.world-heading-actions{gap:12px}.world-heading-actions .secondary-button{padding:0 11px}.world-backup-confirm{padding:15px}.world-backups{margin-top:24px}}
</style>
