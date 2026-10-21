<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, ModRecognitionResult, ModUpdateCandidate, ModUpdateCheck, ModUpdatePreview, ModUpdateResult } from './games';
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
let request = 0;
const allSelected = computed(() => updates.value.length > 0 && updates.value.every((update) => selected.value.includes(update.project_id)));
const working = computed(() => checking.value || preparing.value || applying.value || recognizing.value);
function toggleAll(checked: boolean) { selected.value = checked ? updates.value.map((update) => update.project_id) : []; }
function toggle(id: string, checked: boolean) { selected.value = checked ? [...new Set([...selected.value, id])] : selected.value.filter((value) => value !== id); }
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
  lastRequested.value = [...ids]; preparing.value = true; planError.value = ''; result.value = null;
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
    emit('updated');
  } catch (cause) { planError.value = `更新未完成：${String(cause)}`; }
  finally { applying.value = false; emit('busy', false); }
}
function back() { if (!applying.value) { preview.value = null; planError.value = ''; } }
watch(() => props.game.id, () => { ++request; updates.value = []; selected.value = []; checked.value = false; checking.value = false; preparing.value = false; preview.value = null; checkError.value = ''; planError.value = ''; result.value = null; recognitionResult.value = null; recognitionError.value = ''; });
onUnmounted(() => { ++request; });
</script>
<template>
  <div class="mod-updates">
    <template v-if="preview">
      <button
        class="text-button"
        :disabled="applying"
        @click="back"
      >
        ← 返回更新列表
      </button>
      <div class="update-plan-heading">
        <h2>本次更新 <span class="count">{{ preview.changes.length }}</span></h2><p class="setting-hint">
          确认更新和必需依赖后再安装。
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
          />{{ applying ? '更新中…' : planError ? '重试更新' : '确认更新' }}
        </button>
      </div>
    </template>
    <template v-else>
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
            </div><button
              class="secondary-button"
              :aria-label="`更新 ${update.name}`"
              :disabled="working || locked"
              @click="prepare([update.project_id])"
            >
              更新
            </button>
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
    <div
      v-if="planError"
      class="inline-error"
      role="alert"
    >
      <p>{{ planError }}</p><button
        v-if="!preview"
        class="text-button"
        :disabled="working || locked"
        @click="prepare(lastRequested)"
      >
        重试准备
      </button>
    </div>
  </div>
</template>
