<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type Game, type LauncherImportPreview } from './games';

const emit = defineEmits<{ created: [game: Game]; busy: [value: boolean] }>();
const preview = ref<LauncherImportPreview | null>(null);
const candidateId = ref('');
const contentId = ref('');
const candidate = computed(() => preview.value?.candidates.find((item) => item.candidate_id === candidateId.value));
const contentRoot = computed(() => candidate.value?.content_roots.find((item) => item.content_id === contentId.value));
const name = ref('');
const includeSaves = ref(false);
const source = ref(defaultDownloadSettings());
const directory = ref('');
const useDefaultDirectory = ref(true);
const loading = ref(true);
const picking = ref<'source' | 'destination' | null>(null);
const submitting = ref(false);
const busy = computed(() => picking.value !== null || submitting.value);
const error = ref('');
let disposed = false;

watch(busy, (value) => emit('busy', value), { flush: 'sync' });
watch(candidateId, () => {
  name.value = candidate.value?.name ?? '';
  contentId.value = candidate.value?.content_roots.length === 1 ? candidate.value.content_roots[0]!.content_id : '';
  includeSaves.value = false;
  error.value = '';
}, { flush: 'sync' });
watch(contentId, () => { includeSaves.value = false; }, { flush: 'sync' });

async function initialize() {
  try { source.value = await readDownloadSettings(); directory.value = isTauri() ? await invoke<string>('default_game_directory') : '默认游戏文件夹'; }
  catch (cause) { if (!disposed) error.value = `无法读取安装位置：${String(cause)}`; }
  finally { loading.value = false; }
}
async function chooseSource() {
  if (busy.value || loading.value) return;
  if (!isTauri()) { error.value = '请在桌面应用中导入已有游戏'; return; }
  picking.value = 'source'; error.value = '';
  try {
    const selected = await invoke<LauncherImportPreview | null>('preview_launcher_import');
    if (disposed || !selected) return;
    candidateId.value = '';
    preview.value = selected;
    contentId.value = ''; name.value = ''; includeSaves.value = false;
    if (selected.candidates.length === 1) candidateId.value = selected.candidates[0]!.candidate_id;
  } catch (cause) { if (!disposed) error.value = `无法读取来源游戏：${String(cause)}`; }
  finally { picking.value = null; }
}
async function chooseDirectory() {
  if (busy.value || loading.value) return;
  picking.value = 'destination'; error.value = '';
  try {
    const selected = await invoke<string | null>('pick_game_directory');
    if (!disposed && selected) { directory.value = selected; useDefaultDirectory.value = false; }
  } catch (cause) { if (!disposed) error.value = `无法选择文件夹：${String(cause)}`; }
  finally { picking.value = null; }
}
async function create() {
  if (busy.value || loading.value || !preview.value || !candidate.value?.supported || !contentRoot.value) return;
  error.value = validateDownloadSettings(source.value) ?? '';
  if (error.value) return;
  const trimmedName = name.value.trim();
  if (!trimmedName) { error.value = '请填写游戏名称'; return; }
  if (trimmedName.length > 80) { error.value = '游戏名称不能超过 80 个字符'; return; }
  submitting.value = true;
  try {
    const game = await invoke<Game>('create_launcher_import', {
      previewId: preview.value.preview_id, candidateId: candidate.value.candidate_id,
      contentId: contentRoot.value.content_id, name: trimmedName,
      includeSaves: contentRoot.value.has_saves && includeSaves.value,
      source: { ...source.value, mirror: source.value.mirror.trim() }, useDefaultDirectory: useDefaultDirectory.value,
    });
    if (!disposed) emit('created', game);
  } catch (cause) { if (!disposed) error.value = `无法导入游戏：${String(cause)}`; }
  finally { submitting.value = false; }
}
onMounted(() => { void initialize(); });
onUnmounted(() => { disposed = true; emit('busy', false); });
</script>

<template>
  <div class="import-launcher-page page-content">
    <p class="create-description">
      从其他启动器迁移游戏，创建独立副本。
    </p>
    <button
      class="pack-picker"
      :class="{ 'has-pack': preview }"
      :disabled="loading || busy"
      @click="chooseSource"
    >
      <svg
        viewBox="0 0 40 40"
        fill="none"
        aria-hidden="true"
      ><path d="M5 12h12l4 4h14v17H5zM5 12V7h12l4 5M20 21v8m-4-4 4 4 4-4" /></svg>
      <span><strong>{{ picking === 'source' ? '正在读取…' : preview ? '更换来源目录' : '选择游戏目录' }}</strong><span class="setting-hint">.minecraft 或 Prism / MultiMC 实例目录</span></span>
      <span
        class="picker-arrow"
        aria-hidden="true"
      >↗</span>
    </button>
    <form
      v-if="preview"
      class="create-form pack-form"
      @submit.prevent="create"
    >
      <section
        class="launcher-import-source"
        aria-label="来源信息"
      >
        <p class="launcher-source-label">
          来源目录
        </p>
        <p class="pack-origin">
          {{ preview.source_display }}
        </p>
      </section>
      <p
        v-if="!preview.candidates.length"
        class="profile-notice"
      >
        没有找到可识别的游戏，请选择游戏目录或实例目录。
      </p>
      <div
        v-else
        class="form-field"
      >
        <label for="launcher-import-candidate">来源游戏</label>
        <select
          id="launcher-import-candidate"
          v-model="candidateId"
          :disabled="busy"
        >
          <option
            disabled
            value=""
          >
            请选择来源游戏
          </option>
          <option
            v-for="item in preview.candidates"
            :key="item.candidate_id"
            :value="item.candidate_id"
          >
            {{ item.name }}{{ item.supported ? '' : '（暂不支持）' }}
          </option>
        </select>
      </div>
      <template v-if="candidate">
        <section
          class="launcher-import-summary"
          aria-label="游戏信息"
        >
          <div class="compatibility-tags">
            <span v-if="candidate.minecraft_version">Minecraft {{ candidate.minecraft_version }}</span>
            <span v-if="candidate.supported || candidate.loader_kind">{{ candidate.loader_kind ? `${loaderLabel(candidate.loader_kind)} ${candidate.loader_version ?? ''}` : '原版' }}</span>
          </div>
          <p
            v-if="!candidate.supported"
            class="settings-error"
            role="status"
          >
            {{ candidate.unsupported_reason || '暂不支持导入此游戏' }}
          </p>
          <ul
            v-if="candidate.warnings.length"
            class="launcher-import-warnings"
          >
            <li
              v-for="warning in candidate.warnings"
              :key="warning"
            >
              {{ warning }}
            </li>
          </ul>
        </section>
        <template v-if="candidate.supported">
          <div
            v-if="candidate.content_roots.length > 1"
            class="form-field"
          >
            <label for="launcher-import-root">游戏数据目录</label>
            <select
              id="launcher-import-root"
              v-model="contentId"
              :disabled="busy"
            >
              <option
                disabled
                value=""
              >
                请选择数据目录
              </option>
              <option
                v-for="root in candidate.content_roots"
                :key="root.content_id"
                :value="root.content_id"
              >
                {{ root.label }}
              </option>
            </select>
            <p class="setting-hint">
              选择要复制的 Mods、配置和其他游戏内容。
            </p>
          </div>
          <p
            v-else-if="!candidate.content_roots.length"
            class="profile-notice"
          >
            未找到可导入的游戏数据目录。
          </p>
          <template v-if="contentRoot">
            <label
              v-if="contentRoot.has_saves"
              class="pack-optional launcher-import-saves"
            >
              <input
                v-model="includeSaves"
                type="checkbox"
                :disabled="busy"
              ><span>包含存档</span>
            </label>
            <div class="form-field">
              <label for="launcher-import-name">游戏名称</label>
              <input
                id="launcher-import-name"
                v-model="name"
                maxlength="80"
                autocomplete="off"
                :disabled="busy"
              >
            </div>
            <div class="form-field">
              <label id="launcher-import-source-label">下载源</label>
              <RadioGroupRoot
                v-model="source.source"
                class="source-options"
                :disabled="busy"
                aria-labelledby="launcher-import-source-label"
              >
                <RadioGroupItem
                  value="automatic"
                  class="source-option"
                >
                  自动（推荐）
                </RadioGroupItem>
                <RadioGroupItem
                  value="official"
                  class="source-option"
                >
                  官方
                </RadioGroupItem>
                <RadioGroupItem
                  value="bmclapi"
                  class="source-option"
                >
                  BMCLAPI
                </RadioGroupItem>
                <RadioGroupItem
                  value="custom"
                  class="source-option"
                >
                  自定义镜像
                </RadioGroupItem>
              </RadioGroupRoot>
              <p
                v-if="source.source === 'automatic'"
                class="setting-hint"
              >
                自动选择响应较快的下载源，失败时切换。
              </p>
              <template v-if="source.source === 'custom'">
                <label
                  class="sr-only"
                  for="launcher-import-mirror"
                >镜像地址</label>
                <input
                  id="launcher-import-mirror"
                  v-model="source.mirror"
                  type="url"
                  placeholder="https://mirror.example.com/"
                  :disabled="busy"
                >
              </template>
            </div>
            <div class="form-field">
              <label>安装位置</label>
              <button
                type="button"
                class="folder-button"
                :disabled="busy"
                @click="chooseDirectory"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  aria-hidden="true"
                ><path d="M3 7h7l2 2h9v11H3zM3 7V4h7l2 3" /></svg>
                <span>{{ directory || '默认游戏文件夹' }}</span><span class="folder-change">更改</span>
              </button>
              <p class="setting-hint">
                将在此文件夹内创建独立的游戏目录。
              </p>
            </div>
            <p class="launcher-import-copy-hint">
              请先关闭来源游戏。将复制 Mods、配置、资源包和光影包，来源目录不会修改。
            </p>
          </template>
        </template>
        <button
          class="done-button"
          type="submit"
          :disabled="loading || busy || !candidate.supported || !contentRoot"
        >
          {{ submitting ? '正在创建…' : '导入为独立游戏' }}
        </button>
      </template>
    </form>
    <p
      v-if="error"
      class="settings-error pack-error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
