<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, JavaSelection, LauncherPreferences } from './games';
import MemoryField from './MemoryField.vue';
const props = defineProps<{ game: Game }>();
const emit = defineEmits<{ updated: [game: Game] }>();
const preferences = ref<LauncherPreferences | null>(null);
const defaultJava = ref<JavaSelection | null>(null);
const preferencesError = ref('');
const defaultJavaError = ref('');
const javaBusy = ref(false);
const javaError = ref('');
const memoryBusy = ref(false);
const memoryError = ref('');
const memoryMode = ref<'inherit' | 'custom'>(props.game.runtime_overrides?.memory_mb == null ? 'inherit' : 'custom');
const memoryDraft = ref(props.game.runtime_overrides?.memory_mb ?? 4096);
const javaMode = computed(() => props.game.runtime_overrides?.java.mode ?? 'inherit');
const javaPath = computed(() => {
  const path = javaMode.value === 'custom' ? props.game.runtime_overrides?.java.executable : javaMode.value === 'inherit' ? defaultJava.value?.executable : null;
  if (path?.startsWith('\\\\?\\UNC\\')) return `\\\\${path.slice(8)}`;
  return path?.startsWith('\\\\?\\') ? path.slice(4) : path;
});
const javaDescription = computed(() => javaMode.value === 'inherit' ? defaultJava.value ? `跟随默认 · ${defaultJava.value.mode === 'automatic' ? '自动选择 Java' : '指定 Java'}` : '跟随默认运行设置' : javaMode.value === 'automatic' ? '按游戏版本自动选择 Java' : '仅此游戏使用所选 Java');
async function loadPreferences() {
  preferencesError.value = '';
  try { preferences.value = await invoke<LauncherPreferences>('get_launcher_preferences'); if (props.game.runtime_overrides?.memory_mb == null) memoryDraft.value = preferences.value.memory_mb; }
  catch { preferencesError.value = '无法读取默认内存，请重试'; }
}
async function loadDefaultJava() {
  defaultJavaError.value = '';
  try { defaultJava.value = await invoke<JavaSelection>('get_java_settings'); }
  catch { defaultJavaError.value = '无法读取默认 Java，请重试'; }
}
async function setJava(mode: 'inherit' | 'automatic') {
  if (javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try { emit('updated', await invoke<Game>('set_game_java_mode', { id: props.game.id, mode })); }
  catch { javaError.value = '未能保存游戏 Java，请重试'; }
  finally { javaBusy.value = false; }
}
async function changeJava(event: Event) {
  const select = event.target as HTMLSelectElement;
  if (select.value === 'inherit' || select.value === 'automatic') await setJava(select.value);
  select.value = javaMode.value;
}
async function pickJava() {
  if (javaBusy.value) return;
  javaBusy.value = true; javaError.value = '';
  try { const game = await invoke<Game | null>('pick_game_java_executable', { id: props.game.id }); if (game) emit('updated', game); }
  catch { javaError.value = '请选择有效且兼容此游戏的 Java 可执行文件'; }
  finally { javaBusy.value = false; }
}
async function saveMemory(inherit = false) {
  if (memoryBusy.value) return;
  memoryError.value = '';
  if (!inherit && (!Number.isInteger(memoryDraft.value) || memoryDraft.value < 512 || memoryDraft.value > 262144)) { memoryError.value = '内存须为 512–262144 MiB 的整数'; return; }
  memoryBusy.value = true;
  try { emit('updated', await invoke<Game>('set_game_memory', { id: props.game.id, memoryMb: inherit ? null : memoryDraft.value })); if (inherit) memoryMode.value = 'inherit'; }
  catch { memoryError.value = '未能保存游戏内存，请重试'; }
  finally { memoryBusy.value = false; }
}
async function changeMemoryMode(event: Event) {
  const select = event.target as HTMLSelectElement;
  if (select.value === 'inherit') { await saveMemory(true); memoryMode.value = props.game.runtime_overrides?.memory_mb == null ? 'inherit' : 'custom'; }
  else memoryMode.value = 'custom';
  select.value = memoryMode.value;
}
watch(() => props.game.runtime_overrides?.memory_mb, (value) => { memoryMode.value = value == null ? 'inherit' : 'custom'; memoryDraft.value = value ?? preferences.value?.memory_mb ?? 4096; });
onMounted(() => { void loadPreferences(); void loadDefaultJava(); });
</script>
<template>
  <div class="game-runtime-options">
    <section
      class="runtime-option-section"
      aria-labelledby="game-java-heading"
    >
      <h2 id="game-java-heading">
        Java 运行环境
      </h2>
      <div class="form-field">
        <label for="game-java-mode">Java模式</label><select
          id="game-java-mode"
          :value="javaMode"
          :disabled="javaBusy"
          @change="changeJava"
        >
          <option value="inherit">
            跟随默认运行设置
          </option><option value="automatic">
            自动选择
          </option><option
            v-if="javaMode === 'custom'"
            value="custom"
          >
            已选 Java
          </option>
        </select>
      </div>
      <p class="setting-hint">
        {{ javaDescription }}
      </p><p
        v-if="javaPath"
        class="java-path"
      >
        {{ javaPath }}
      </p>
      <div class="java-actions">
        <button
          class="secondary-button"
          :disabled="javaBusy"
          @click="pickJava"
        >
          {{ javaBusy ? '请稍候…' : '选择游戏 Java' }}
        </button><button
          v-if="javaMode !== 'inherit'"
          class="text-button"
          :disabled="javaBusy"
          @click="setJava('inherit')"
        >
          恢复默认 Java
        </button>
      </div>
      <p
        v-if="javaError"
        class="settings-error"
        role="alert"
      >
        {{ javaError }}
      </p>
      <p
        v-if="defaultJavaError && javaMode === 'inherit'"
        class="settings-error"
        role="alert"
      >
        {{ defaultJavaError }} <button
          class="text-button"
          @click="loadDefaultJava"
        >
          重试
        </button>
      </p>
    </section>
    <section
      class="runtime-option-section"
      aria-labelledby="game-memory-heading"
    >
      <h2 id="game-memory-heading">
        最大内存
      </h2>
      <div class="form-field">
        <label for="game-memory-mode">内存设置</label><select
          id="game-memory-mode"
          :value="memoryMode"
          :disabled="memoryBusy"
          @change="changeMemoryMode"
        >
          <option value="inherit">
            跟随默认运行设置
          </option><option value="custom">
            单独设置
          </option>
        </select>
      </div>
      <p
        v-if="game.runtime_overrides?.memory_mb == null && preferences"
        class="setting-hint"
      >
        跟随默认 · {{ preferences.memory_mb }} MiB
      </p>
      <p
        v-else-if="game.runtime_overrides?.memory_mb != null"
        class="setting-hint"
      >
        此游戏使用 {{ game.runtime_overrides.memory_mb }} MiB
      </p>
      <form
        v-if="memoryMode === 'custom'"
        class="runtime-memory-form"
        novalidate
        @submit.prevent="saveMemory()"
      >
        <MemoryField
          id="game-memory"
          v-model="memoryDraft"
          label="游戏最大内存 (MiB)"
          :disabled="memoryBusy"
        /><div class="profile-form-actions">
          <button
            class="secondary-button"
            type="submit"
            :disabled="memoryBusy"
          >
            {{ memoryBusy ? '保存中…' : '保存游戏内存' }}
          </button><button
            class="text-button"
            type="button"
            :disabled="memoryBusy"
            @click="saveMemory(true)"
          >
            恢复默认内存
          </button>
        </div>
      </form>
      <p
        v-if="memoryError"
        class="settings-error"
        role="alert"
      >
        {{ memoryError }}
      </p>
      <p
        v-if="preferencesError && game.runtime_overrides?.memory_mb == null"
        class="settings-error"
        role="alert"
      >
        {{ preferencesError }} <button
          class="text-button"
          @click="loadPreferences"
        >
          重试
        </button>
      </p>
    </section>
  </div>
</template>
