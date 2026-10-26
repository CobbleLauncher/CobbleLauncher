<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Puzzle, SlidersHorizontal } from '@lucide/vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import { label as l } from './labels';
import type { ExportPreview, ExportResult } from './types';

const props = defineProps<{ gameId: string; locked: boolean }>();
const preview = ref<ExportPreview | null>(null);
const selectedMods = ref<string[]>([]);
const selectedConfigs = ref<string[]>([]);
const step = ref<'select' | 'review' | 'done'>('select');
const busy = ref(false);
const error = ref('');
let request = 0;
const count = computed(() => selectedMods.value.length + selectedConfigs.value.length);
function confirmLeave(): Promise<boolean> { return Promise.resolve(!busy.value); }
defineExpose({ confirmLeave });
const selectedModRows = computed(() => preview.value?.mods.filter(item => selectedMods.value.includes(item.projectId)) ?? []);
const selectedConfigRows = computed(() => preview.value?.configs.filter(item => selectedConfigs.value.includes(item.id)) ?? []);
function message(cause: unknown, fallback: string) {
  const code = String(cause);
  if (code.includes('CHANGE_SHARE_STALE')) return l('stale');
  if (code.includes('CHANGE_SHARE_BUSY')) return l('locked');
  return fallback;
}
async function load() {
  const current = ++request;
  busy.value = true; error.value = '';
  try {
    const value = await invoke<ExportPreview>('preview_game_change_export', { id: props.gameId });
    if (current !== request) return;
    preview.value = value;
    selectedMods.value = [];
    selectedConfigs.value = [];
    step.value = 'select';
  } catch (cause) { if (current === request) error.value = message(cause, l('loadFailed')); }
  finally { if (current === request) busy.value = false; }
}
async function save() {
  if (!preview.value || !count.value || busy.value || props.locked) return;
  busy.value = true; error.value = '';
  try {
    const result = await invoke<ExportResult>('export_game_change', {
      id: props.gameId, revision: preview.value.revision,
      modIds: selectedMods.value, configIds: selectedConfigs.value,
    });
    if (result.saved) step.value = 'done';
    else step.value = 'select';
  } catch (cause) { error.value = message(cause, l('exportFailed')); }
  finally { busy.value = false; }
}
watch(() => props.gameId, () => { preview.value = null; void load(); }, { immediate: true });
</script>

<template>
  <section
    class="change-share-body"
    :aria-busy="busy"
  >
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }} <button
        class="text-button"
        type="button"
        @click="load"
      >
        {{ l('retry') }}
      </button>
    </p>
    <p
      v-if="busy && !preview"
      role="status"
    >
      {{ l('load') }}
    </p>
    <div
      v-if="preview && step === 'select'"
      class="change-share-layout"
    >
      <div class="change-share-main">
        <div class="change-share-section-title">
          <h3>
            <Puzzle
              :size="18"
              aria-hidden="true"
            />{{ l('mods') }}
          </h3><button
            class="text-button"
            type="button"
            :disabled="busy"
            @click="selectedMods = selectedMods.length === preview.mods.length ? [] : preview.mods.map(item => item.projectId)"
          >
            {{ selectedMods.length === preview.mods.length ? l('clear') : l('selectAll') }}
          </button>
        </div>
        <div class="change-share-options">
          <label
            v-for="item in preview.mods"
            :key="item.projectId"
            class="change-share-option"
          >
            <input
              v-model="selectedMods"
              type="checkbox"
              :value="item.projectId"
              :disabled="busy"
            ><span><strong>{{ item.name }}</strong><small>{{ item.versionId }}</small></span>
          </label>
          <p
            v-if="!preview.mods.length"
            class="change-share-muted"
          >
            {{ l('none') }}
          </p>
        </div>
        <div class="change-share-section-title">
          <h3>
            <SlidersHorizontal
              :size="18"
              aria-hidden="true"
            />{{ l('configs') }}
          </h3><button
            class="text-button"
            type="button"
            :disabled="busy"
            @click="selectedConfigs = selectedConfigs.length === preview.configs.length ? [] : preview.configs.map(item => item.id)"
          >
            {{ selectedConfigs.length === preview.configs.length ? l('clear') : l('selectAll') }}
          </button>
        </div>
        <p
          v-if="preview.configs.length"
          class="change-share-muted"
        >
          {{ l('configHelp') }}
        </p>
        <div class="change-share-options">
          <label
            v-for="item in preview.configs"
            :key="item.id"
            class="change-share-option"
          >
            <input
              v-model="selectedConfigs"
              type="checkbox"
              :value="item.id"
              :disabled="busy"
            ><span><strong>{{ item.name }}</strong><small>{{ (item.size / 1024).toFixed(1) }} KB</small></span>
          </label>
        </div>
      </div><aside class="change-share-aside">
        <div class="change-share-count">
          <strong>{{ count }}</strong><span>{{ l('selected') }}</span>
        </div><SettingsDisclosure
          v-if="preview.excludedMods.length"
          :title="`${l('excluded')} · ${preview.excludedMods.length}`"
        >
          <p>{{ l('excludedHelp') }}</p><ul>
            <li
              v-for="name in preview.excludedMods"
              :key="name"
            >
              {{ name }}
            </li>
          </ul>
        </SettingsDisclosure>
      </aside>
    </div>
    <div
      v-if="preview && step === 'review'"
      class="change-share-layout"
    >
      <div class="change-share-main">
        <h3>{{ l('review') }}</h3>
        <div
          v-if="selectedModRows.length"
          class="change-share-review-group"
        >
          <h4>
            <Puzzle
              :size="18"
              aria-hidden="true"
            />{{ l('mods') }} · {{ selectedModRows.length }}
          </h4><p
            v-for="item in selectedModRows"
            :key="item.projectId"
          >
            {{ item.name }} <small>{{ item.versionId }}</small>
          </p>
        </div>
        <div
          v-if="selectedConfigRows.length"
          class="change-share-review-group"
        >
          <h4>
            <SlidersHorizontal
              :size="18"
              aria-hidden="true"
            />{{ l('configs') }} · {{ selectedConfigRows.length }}
          </h4><SettingsDisclosure
            v-for="item in selectedConfigRows"
            :key="item.id"
            :title="item.name"
            default-open
          >
            <pre>{{ item.text }}</pre>
          </SettingsDisclosure>
        </div>
        <p
          v-if="selectedConfigRows.length"
          class="change-share-muted"
        >
          {{ l('configHelp') }}
        </p>
      </div><aside class="change-share-aside">
        <div class="change-share-count">
          <strong>{{ count }}</strong><span>{{ l('selected') }}</span>
        </div>
      </aside>
    </div>
    <p
      v-if="step === 'done'"
      role="status"
    >
      {{ l('saved') }}
    </p>
    <div
      v-if="preview && step !== 'done'"
      class="change-share-footer"
    >
      <span>{{ l('selected') }} · {{ count }}</span>
      <button
        v-if="step === 'review'"
        type="button"
        class="text-button"
        :disabled="busy"
        @click="step = 'select'"
      >
        {{ l('edit') }}
      </button>
      <button
        class="primary-button"
        type="button"
        :disabled="busy || locked || !count"
        @click="step === 'select' ? step = 'review' : save()"
      >
        {{ busy ? l('saving') : step === 'select' ? l('review') : l('save') }}
      </button>
    </div>
  </section>
</template>
