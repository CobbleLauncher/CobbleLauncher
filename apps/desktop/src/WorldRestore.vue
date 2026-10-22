<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import type { LocalWorld } from './games';
import type { WorldBackupPreview, WorldRestoreResult } from './world-backups';
const props = defineProps<{ gameId: string; worlds: LocalWorld[]; locked: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; cancel: []; restored: [result: WorldRestoreResult, folder: string] }>();
const preview = ref<WorldBackupPreview | null>(null);
const mode = ref('new');
const folderName = ref('');
const targetFolder = ref('');
const target = computed(() => props.worlds.find((world) => world.folder_name === targetFolder.value));
const confirmed = ref(false);
const picking = ref(false);
const restoring = ref(false);
const busy = computed(() => picking.value || restoring.value);
const error = ref('');
let disposed = false;
function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const unit = bytes < 1024 ** 2 ? 'KiB' : bytes < 1024 ** 3 ? 'MiB' : 'GiB';
  const divisor = unit === 'KiB' ? 1024 : unit === 'MiB' ? 1024 ** 2 : 1024 ** 3;
  return `${new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 1 }).format(bytes / divisor)} ${unit}`;
}
async function choose() {
  if (busy.value || props.locked) return;
  picking.value = true; error.value = ''; emit('busy', true);
  try {
    const value = await invoke<WorldBackupPreview | null>('preview_world_backup', { id: props.gameId });
    if (disposed) return;
    if (!value) { if (!preview.value) emit('cancel'); return; }
    preview.value = value; folderName.value = value.suggested_folder; mode.value = 'new'; targetFolder.value = ''; confirmed.value = false;
  } catch (cause) { if (!disposed) error.value = `无法读取世界备份：${String(cause)}`; }
  finally { picking.value = false; emit('busy', false); }
}
async function restore() {
  if (!preview.value || busy.value || props.locked) return;
  error.value = '';
  const folder = mode.value === 'replace' ? target.value?.folder_name : folderName.value.trim();
  if (!folder) { error.value = mode.value === 'replace' ? '请选择目标世界' : '请填写世界文件夹名称'; return; }
  if (mode.value === 'replace' && !confirmed.value) return;
  if (mode.value === 'new' && (/[/\\<>:"|?*]/.test(folder) || [...folder].some((character) => character.charCodeAt(0) < 32) || folder === '.' || folder === '..' || folder.endsWith('.'))) { error.value = '请输入有效的文件夹名称，不含路径或特殊字符'; return; }
  if (mode.value === 'new' && props.worlds.some((world) => world.folder_name.toLocaleLowerCase() === folder.toLocaleLowerCase())) { error.value = '此世界文件夹已存在，请使用其他名称或选择替换已有世界'; return; }
  restoring.value = true; emit('busy', true);
  try {
    const value = await invoke<WorldRestoreResult>('restore_game_world', { id: props.gameId, previewId: preview.value.preview_id, folderName: folder, replaceExisting: mode.value === 'replace' });
    if (!disposed) emit('restored', value, folder);
  } catch (cause) { if (!disposed) error.value = `恢复世界未完成：${String(cause)}`; }
  finally { restoring.value = false; emit('busy', false); }
}
watch([mode, targetFolder], () => { confirmed.value = false; error.value = ''; });
onMounted(() => { void choose(); });
onUnmounted(() => { disposed = true; });
</script>
<template>
  <section class="world-restore">
    <button
      class="text-button"
      :disabled="busy"
      @click="emit('cancel')"
    >
      ← 返回世界列表
    </button>
    <div class="world-restore-heading">
      <h2>恢复世界</h2>
    </div>
    <button
      class="pack-picker"
      :class="{ 'has-pack': preview }"
      :disabled="busy || locked"
      @click="choose"
    >
      <svg
        viewBox="0 0 40 40"
        fill="none"
        aria-hidden="true"
      ><path d="m8 12 12-6 12 6-12 6zM8 12v16l12 6 12-6V12M20 18v16M15 9l12 6" /></svg><span><strong>{{ picking ? '正在读取…' : preview ? '更换备份' : '选择世界备份 ZIP' }}</strong><span class="setting-hint">世界文件夹的 ZIP 备份</span></span><span
        class="picker-arrow"
        aria-hidden="true"
      >↗</span>
    </button>
    <form
      v-if="preview"
      class="create-form pack-form"
      @submit.prevent="restore"
    >
      <section
        class="pack-summary"
        aria-label="备份信息"
      >
        <div class="pack-title">
          <h3>{{ preview.name }}</h3>
        </div><div
          v-if="preview.minecraft_version"
          class="compatibility-tags"
        >
          <span>Minecraft {{ preview.minecraft_version }}</span>
        </div><p class="setting-hint">
          {{ new Intl.NumberFormat('zh-CN').format(preview.file_count) }} 个文件 · {{ sizeLabel(preview.total_bytes) }}
        </p>
        <p
          v-if="preview.metadata_error"
          class="world-metadata-warning"
        >
          无法读取世界信息，请确认备份来源
        </p>
      </section>
      <div class="form-field">
        <label id="world-restore-mode-label">恢复方式</label><RadioGroupRoot
          v-model="mode"
          class="world-restore-modes"
          aria-labelledby="world-restore-mode-label"
          :disabled="busy || locked"
        >
          <RadioGroupItem
            value="new"
            class="world-restore-mode"
          >
            恢复为新世界
          </RadioGroupItem><RadioGroupItem
            value="replace"
            class="world-restore-mode"
            :disabled="!worlds.length"
          >
            替换已有世界
          </RadioGroupItem>
        </RadioGroupRoot>
      </div>
      <div
        v-if="mode === 'new'"
        class="form-field"
      >
        <label for="restore-world-folder">世界文件夹</label><input
          id="restore-world-folder"
          v-model="folderName"
          maxlength="128"
          autocomplete="off"
          :disabled="busy || locked"
        ><p class="setting-hint">
          在存档目录中创建新文件夹，不覆盖已有世界。
        </p>
      </div>
      <template v-else>
        <div class="form-field">
          <label for="restore-world-target">目标世界</label><select
            id="restore-world-target"
            v-model="targetFolder"
            :disabled="busy || locked"
          >
            <option
              value=""
              disabled
            >
              请选择目标世界
            </option><option
              v-for="world in worlds"
              :key="world.folder_name"
              :value="world.folder_name"
            >
              {{ world.name }}{{ world.name !== world.folder_name ? ` · ${world.folder_name}` : '' }}
            </option>
          </select>
        </div>
        <div
          v-if="target"
          class="world-replace-confirm"
        >
          <p>将用备份替换 {{ target.name }}。替换前的世界会单独保留，可在世界列表中恢复。</p><label class="pack-optional"><input
            v-model="confirmed"
            type="checkbox"
            :disabled="busy || locked"
          ><span>我确认替换 {{ target.name }}</span></label>
        </div>
      </template>
      <button
        class="done-button"
        :class="{ 'danger-text': mode === 'replace' }"
        type="submit"
        :disabled="busy || locked || (mode === 'replace' && (!target || !confirmed))"
      >
        {{ restoring ? '恢复中…' : mode === 'replace' ? '替换并保留备份' : '恢复世界' }}
      </button>
    </form>
    <p
      v-if="error"
      class="settings-error pack-error"
      role="alert"
    >
      {{ error }}
    </p>
  </section>
</template>

<style scoped>
.world-restore-heading{margin:13px 0 23px}.world-restore-heading h2{font-size:20px;font-weight:600}.world-restore .pack-title h3{font-size:19px;line-height:1.5;margin:0;font-weight:600;overflow-wrap:anywhere}.world-restore .pack-summary .compatibility-tags{margin-top:12px}.world-restore-modes{display:flex;gap:9px}.world-restore-mode{flex:1;min-width:0;min-height:43px;border:1px solid var(--line);border-radius:9px;background:transparent;color:var(--muted);padding:10px 12px;font-size:12px}.world-restore-mode[data-state=checked]{border-color:#87a65b;background:var(--hover);color:var(--ink)}.world-restore-mode:disabled{opacity:.5}.world-restore select{width:100%;min-width:0}.world-replace-confirm{padding:15px 17px;background:var(--panel);border:1px solid var(--line);border-radius:11px}.world-replace-confirm>p{font-size:12px;color:var(--muted);line-height:1.8;overflow-wrap:anywhere}.world-replace-confirm .pack-optional{padding-bottom:0}.world-restore .done-button.danger-text{background:var(--panel);border:1px solid currentColor}@media(max-width:560px){.world-restore .pack-picker{padding:19px 17px;gap:13px}.world-restore .pack-picker>svg{width:32px;height:32px}.world-restore-modes{gap:8px}.world-restore-mode{padding:10px 8px;font-size:11px}.world-replace-confirm{padding:13px 14px}}
</style>
