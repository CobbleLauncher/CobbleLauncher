<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, ManagedGameMod, ModRecognitionResult, ModUpdateCandidate, ModUpdateCheck, ModUpdatePreview, ModUpdateResult, ModVersion } from './games';
import ModVersionPicker from './ModVersionPicker.vue';
const props = defineProps<{ game: Game; locked: boolean }>();
const emit = defineEmits<{ busy: [value: boolean]; recognizing: [value: boolean]; updated: [] }>();
const updates = ref<ModUpdateCandidate[]>([]);
const selected = ref<string[]>([]);
const checked = ref(false);
const checking = ref(false);
const managedCount = ref(0);
const checkError = ref('');
const preparing = ref(false);
const applying = ref(false);
const preview = ref<ModUpdatePreview | null>(null);
const planError = ref('');
const lastRequested = ref<string[]>([]);
const result = ref<ModUpdateResult | null>(null);
const recognizing = ref(false);
const recognitionResult = ref<ModRecognitionResult | null>(null);
const recognitionError = ref('');
const versionMode = ref(false);
const managed = ref<ManagedGameMod[]>([]);
const loadingManaged = ref(false);
const managedError = ref('');
const selectedManaged = ref<ManagedGameMod | null>(null);
const managedVersion = ref<ModVersion | null>(null);
const preferredVersionId = ref<string | undefined>();
const planKind = ref<'update' | 'version'>('update');
const versionChanged = ref(false);
let managedRequest = 0;
let request = 0;
const allSelected = computed(() => updates.value.length > 0 && updates.value.every((update) => selected.value.includes(update.project_id)));
const working = computed(() => checking.value || preparing.value || applying.value || recognizing.value);
function toggleAll(checked: boolean) { selected.value = checked ? updates.value.map((update) => update.project_id) : []; }
function toggle(id: string, checked: boolean) { selected.value = checked ? [...new Set([...selected.value, id])] : selected.value.filter((value) => value !== id); }
async function loadManaged() {
  const current = ++managedRequest;
  loadingManaged.value = true; managedError.value = '';
  try {
    const value = await invoke<ManagedGameMod[]>('list_managed_game_mods', { id: props.game.id });
    if (current === managedRequest) managed.value = value;
  } catch (cause) { if (current === managedRequest) managedError.value = `无法读取 Mod 版本记录：${String(cause)}`; }
  finally { if (current === managedRequest) loadingManaged.value = false; }
}
function openVersionManager() {
  if (working.value) return;
  versionMode.value = true; versionChanged.value = false; planError.value = ''; selectedManaged.value = null; managedVersion.value = null;
  void loadManaged();
}
function manageVersion(mod: ManagedGameMod) {
  if (working.value) return;
  preferredVersionId.value = undefined; selectedManaged.value = mod; managedVersion.value = null; planError.value = ''; versionChanged.value = false;
}
async function viewUpdateVersion(update: ModUpdateCandidate) {
  if (working.value) return;
  versionMode.value = true; selectedManaged.value = null; managedVersion.value = null; planError.value = ''; versionChanged.value = false;
  const current = managedRequest + 1;
  await loadManaged();
  if (current !== managedRequest || !versionMode.value || managedError.value) return;
  const mod = managed.value.find((item) => item.project_id === update.project_id);
  if (!mod) { managedError.value = '未找到此 Mod 的版本记录，可重新识别本地 Mod 后重试。'; return; }
  preferredVersionId.value = update.version_id; selectedManaged.value = { ...mod, name: mod.name ?? update.name };
}
function chooseVersion(value: ModVersion | null) {
  ++request; managedVersion.value = value; preparing.value = false; planError.value = '';
  if (planKind.value === 'version') preview.value = null;
}
function leaveVersionManager() {
  if (applying.value) return;
  ++request; preparing.value = false; planError.value = '';
  if (selectedManaged.value) { selectedManaged.value = null; managedVersion.value = null; }
  else { ++managedRequest; loadingManaged.value = false; versionMode.value = false; }
}
async function prepareVersion() {
  if (!selectedManaged.value || !managedVersion.value || working.value || props.locked || selectedManaged.value.version_id === managedVersion.value.version_id) return;
  const current = ++request;
  planKind.value = 'version'; preparing.value = true; planError.value = ''; versionChanged.value = false;
  try {
    const value = await invoke<ModUpdatePreview>('preview_game_mod_version_changes', { id: props.game.id, selections: [{ project_id: selectedManaged.value.project_id, version_id: managedVersion.value.version_id }] });
    if (current === request) preview.value = value;
  } catch (cause) { if (current === request) planError.value = `无法准备版本变更：${String(cause)}`; }
  finally { if (current === request) preparing.value = false; }
}
async function recognize() {
  if (working.value || props.locked) return;
  const current = ++request;
  recognizing.value = true; recognitionError.value = ''; recognitionResult.value = null;
  emit('recognizing', true); emit('busy', true);
  try {
    const value = await invoke<ModRecognitionResult>('recognize_game_mods', { id: props.game.id });
    if (current !== request) return;
    recognitionResult.value = value;
    updates.value = []; selected.value = []; preview.value = null; checked.value = false;
    managedCount.value = 0; lastRequested.value = []; checkError.value = ''; planError.value = ''; result.value = null;
  } catch { if (current === request) recognitionError.value = '未能识别本地 Mod，请重试。'; }
  finally { recognizing.value = false; emit('recognizing', false); emit('busy', false); }
}
async function check() {
  if (working.value || props.locked) return;
  const current = ++request;
  checking.value = true; checkError.value = ''; planError.value = ''; result.value = null; recognitionResult.value = null;
  try {
    const value = await invoke<ModUpdateCheck>('check_game_mod_updates', { id: props.game.id });
    if (current !== request) return;
    updates.value = value.updates; managedCount.value = value.managed_count; checked.value = true;
    selected.value = selected.value.filter((id) => value.updates.some((update) => update.project_id === id));
  } catch { if (current === request) checkError.value = '暂时无法检查更新，请重试。'; }
  finally { if (current === request) checking.value = false; }
}
async function prepare(ids: string[]) {
  if (!ids.length || working.value || props.locked) return;
  const current = ++request;
  planKind.value = 'update'; lastRequested.value = [...ids]; preparing.value = true; planError.value = ''; result.value = null;
  try {
    const value = await invoke<ModUpdatePreview>('preview_game_mod_updates', { id: props.game.id, projectIds: ids });
    if (current === request) preview.value = value;
  } catch (cause) { if (current === request) planError.value = `无法准备更新：${String(cause)}`; }
  finally { if (current === request) preparing.value = false; }
}
async function apply() {
  if (!preview.value || applying.value || props.locked) return;
  applying.value = true; emit('busy', true); planError.value = '';
  try {
    result.value = await invoke<ModUpdateResult>('apply_game_mod_updates', { id: props.game.id, planId: preview.value.plan_id });
    const completed = new Set(preview.value.changes.map((change) => change.project_id));
    updates.value = updates.value.filter((update) => !completed.has(update.project_id));
    selected.value = selected.value.filter((id) => !completed.has(id));
    managedCount.value += result.value.added;
    preview.value = null;
    if (planKind.value === 'version') {
      versionChanged.value = true; result.value = null; selectedManaged.value = null; managedVersion.value = null;
      updates.value = []; selected.value = []; checked.value = false; recognitionResult.value = null;
      await loadManaged();
    }
    emit('updated');
  } catch (cause) { planError.value = `${planKind.value === 'version' ? '版本更换' : '更新'}未完成：${String(cause)}`; }
  finally { applying.value = false; emit('busy', false); }
}
function back() { if (!applying.value) { preview.value = null; planError.value = ''; } }
watch(() => props.game.id, () => { ++request; ++managedRequest; versionMode.value = false; managed.value = []; loadingManaged.value = false; managedError.value = ''; selectedManaged.value = null; managedVersion.value = null; versionChanged.value = false; updates.value = []; selected.value = []; checked.value = false; checking.value = false; preparing.value = false; preview.value = null; checkError.value = ''; planError.value = ''; result.value = null; recognitionResult.value = null; recognitionError.value = ''; });
onUnmounted(() => { ++request; ++managedRequest; });
</script>
<template>
  <div class="mod-updates">
    <template v-if="preview">
      <button
        class="text-button"
        :disabled="applying"
        @click="back"
      >
        {{ planKind === 'version' ? '← 返回版本选择' : '← 返回更新列表' }}
      </button>
      <div class="update-plan-heading">
        <h2>{{ planKind === 'version' ? '更换版本' : '本次更新' }} <span class="count">{{ preview.changes.length }}</span></h2><p class="setting-hint">
          确认版本变更和必需依赖后再安装。
        </p>
      </div>
      <ul
        class="mod-list"
        aria-label="更新内容"
      >
        <li
          v-for="change in preview.changes"
          :key="change.project_id"
          class="mod-row"
        >
          <div class="mod-info">
            <h3>
              {{ change.name }}<span
                v-if="!change.from_version"
                class="mod-state"
              >新增依赖</span><span
                v-if="!change.enabled"
                class="mod-state"
              >保持停用</span>
            </h3><p class="mod-meta">
              {{ change.from_version ? `${change.from_version} → ${change.to_version}` : change.to_version }}
            </p>
          </div>
        </li>
      </ul>
      <div class="plan-actions">
        <button
          class="primary-button"
          :disabled="applying || locked || !preview.changes.length"
          @click="apply"
        >
          <span
            v-if="applying"
            class="busy-spinner"
            aria-hidden="true"
          />{{ planKind === 'version' ? applying ? '更换中…' : planError ? '重试更换' : '确认更换' : applying ? '更新中…' : planError ? '重试更新' : '确认更新' }}
        </button>
      </div>
    </template>
    <template v-else-if="!versionMode">
      <div class="content-heading">
        <div>
          <h2>Mod 更新</h2><p class="setting-hint">
            检查已识别的 Modrinth Mod。
          </p>
        </div><button
          class="secondary-button"
          :disabled="working || locked"
          @click="check"
        >
          {{ checking ? '检查中…' : checked ? '重新检查' : '检查更新' }}
        </button>
      </div>
      <div class="mod-recognition-tools">
        <button
          class="text-button"
          :disabled="working"
          @click="openVersionManager"
        >
          版本管理
        </button>
        <button
          class="text-button"
          :disabled="working || locked"
          @click="recognize"
        >
          {{ recognizing ? '识别中…' : recognitionError ? '重试识别' : '识别本地 Mod' }}
        </button>
        <p class="mod-meta">
          用于本地导入和整合包中的 Mod。
        </p>
      </div>
      <p
        v-if="recognitionResult"
        class="update-result"
        role="status"
      >
        <template v-if="!recognitionResult.recognized && !recognitionResult.unmatched">
          没有待识别的 Mod。
        </template><template v-else>
          已识别 {{ recognitionResult.recognized }} 个 Mod<span v-if="recognitionResult.unmatched">，{{ recognitionResult.unmatched }} 个未匹配，需自行确认版本</span>。
        </template>
      </p>
      <p
        v-if="recognitionError"
        class="settings-error"
        role="alert"
      >
        {{ recognitionError }}
      </p>
      <p
        v-if="checkError"
        class="settings-error"
        role="alert"
      >
        {{ checkError }}
      </p>
      <p
        v-if="result"
        class="update-result"
        role="status"
      >
        已更新 {{ result.updated }} 个 Mod<span v-if="result.added">，新增 {{ result.added }} 个必需依赖</span>。
      </p>
      <template v-if="updates.length">
        <div class="mod-selection-toolbar">
          <label class="mod-selection-label"><input
            type="checkbox"
            aria-label="全选更新"
            :checked="allSelected"
            :indeterminate="selected.length > 0 && !allSelected"
            :disabled="working || locked"
            @change="toggleAll(($event.target as HTMLInputElement).checked)"
          ><span>全选更新</span></label><button
            class="secondary-button"
            aria-label="更新所选"
            :disabled="!selected.length || working || locked"
            @click="prepare(selected)"
          >
            {{ preparing ? '准备中…' : `更新所选 (${selected.length})` }}
          </button>
        </div>
        <ul
          class="mod-list update-list"
          aria-label="可用更新"
        >
          <li
            v-for="update in updates"
            :key="update.project_id"
            class="mod-row"
          >
            <input
              class="mod-selection-checkbox"
              type="checkbox"
              :aria-label="`选择更新 ${update.name}`"
              :checked="selected.includes(update.project_id)"
              :disabled="working || locked"
              @change="toggle(update.project_id, ($event.target as HTMLInputElement).checked)"
            ><div class="mod-info">
              <h3>
                {{ update.name }}<span
                  v-if="update.pack_owned"
                  class="mod-state"
                >整合包自带</span><span
                  v-if="!update.enabled"
                  class="mod-state"
                >已停用</span>
              </h3><p class="mod-meta">
                {{ update.current_version }} <span
                  class="update-arrow"
                  aria-hidden="true"
                >→</span> {{ update.version_number }}
              </p>
            </div><div class="update-version-actions">
              <button
                class="text-button"
                :aria-label="`版本与日志 ${update.name}`"
                :disabled="working"
                @click="viewUpdateVersion(update)"
              >
                版本与日志
              </button><button
                class="secondary-button"
                :aria-label="`更新 ${update.name}`"
                :disabled="working || locked"
                @click="prepare([update.project_id])"
              >
                更新
              </button>
            </div>
          </li>
        </ul>
      </template>
      <p
        v-else-if="!checking"
        class="content-empty"
      >
        {{ !checked ? '手动检查适用于此游戏版本和加载器的更新。' : managedCount ? '已检查的 Modrinth Mod 暂无可用更新。' : '没有可检查更新的 Modrinth Mod。' }}<span
          v-if="checked"
          class="unmanaged-note"
        >未识别的 Mod 需自行确认版本。</span>
      </p>
      <p
        v-if="preparing"
        class="setting-hint"
        role="status"
      >
        正在准备更新和必需依赖…
      </p>
    </template>
    <section
      v-show="versionMode && !preview"
      class="managed-mod-versions"
    >
      <button
        class="text-button"
        :disabled="applying"
        @click="leaveVersionManager"
      >
        {{ selectedManaged ? '← 返回版本列表' : '← 返回更新列表' }}
      </button>
      <template v-if="selectedManaged">
        <div class="managed-version-heading">
          <h2>{{ selectedManaged.name ?? selectedManaged.filename }}</h2><p
            v-if="selectedManaged.name && selectedManaged.name !== selectedManaged.filename"
            class="mod-filename"
          >
            {{ selectedManaged.filename }}
          </p><p
            v-if="!selectedManaged.enabled"
            class="mod-meta"
          >
            此 Mod 已停用，更换版本后保持停用。
          </p>
        </div>
        <ModVersionPicker
          :key="selectedManaged.project_id"
          :game-id="game.id"
          :project-id="selectedManaged.project_id"
          :current-version-id="selectedManaged.version_id"
          :preferred-version-id="preferredVersionId"
          :disabled="applying"
          @selected="chooseVersion"
        />
        <p
          v-if="managedVersion"
          class="version-change-summary"
        >
          {{ managedVersion.version_id === selectedManaged.version_id ? '当前已安装此版本。' : `更换到 ${managedVersion.version_number}` }}
        </p>
        <button
          class="secondary-button mod-version-preview-button"
          :disabled="working || locked || !managedVersion || managedVersion.version_id === selectedManaged.version_id"
          @click="prepareVersion"
        >
          {{ preparing ? '准备中…' : planError ? '重试预览版本变更' : '预览版本变更' }}
        </button>
        <p
          v-if="locked"
          class="setting-hint"
        >
          关闭游戏后即可更换版本。
        </p>
      </template>
      <template v-else>
        <div class="managed-version-heading">
          <h2>Mod 版本管理</h2><p class="setting-hint">
            选择已识别的 Mod，查看或更换版本。
          </p>
        </div>
        <p
          v-if="versionChanged"
          class="update-result"
          role="status"
        >
          版本已更换。
        </p>
        <p
          v-if="loadingManaged"
          class="content-empty"
          role="status"
        >
          正在读取 Mod 版本记录…
        </p>
        <div
          v-if="managedError"
          class="inline-error"
          role="alert"
        >
          <p>{{ managedError }}</p><button
            class="text-button"
            @click="loadManaged"
          >
            重试读取版本记录
          </button>
        </div>
        <ul
          v-if="managed.length"
          class="mod-list managed-mod-list"
          aria-label="可管理版本的 Mod"
        >
          <li
            v-for="mod in managed"
            :key="mod.project_id"
            class="mod-row"
          >
            <div class="mod-info">
              <h3>{{ mod.name ?? mod.filename }}</h3><p
                v-if="mod.name && mod.name !== mod.filename"
                class="mod-filename"
              >
                {{ mod.filename }}
              </p><p
                v-if="!mod.enabled"
                class="mod-meta"
              >
                已停用
              </p>
            </div><button
              class="secondary-button"
              :aria-label="`管理版本 ${mod.name ?? mod.filename}`"
              :disabled="loadingManaged || working"
              @click="manageVersion(mod)"
            >
              管理版本
            </button>
          </li>
        </ul>
        <p
          v-else-if="!loadingManaged && !managedError"
          class="content-empty"
        >
          没有可管理版本的 Modrinth Mod，可先识别本地 Mod。
        </p>
      </template>
    </section>
    <div
      v-if="planError"
      class="inline-error"
      role="alert"
    >
      <p>{{ planError }}</p><button
        v-if="!preview && !versionMode"
        class="text-button"
        :disabled="working || locked"
        @click="prepare(lastRequested)"
      >
        重试准备
      </button>
    </div>
  </div>
</template>
