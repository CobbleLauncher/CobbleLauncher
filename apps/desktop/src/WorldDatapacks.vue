<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { LocalWorld } from './games';
interface Pack { file_name: string; name: string; description: string | null; size_bytes: number; pack_format: number | null }
interface Inventory { installed: Pack[]; removed: Pack[] }
const props = defineProps<{ gameId: string; world: LocalWorld; locked: boolean }>();
const emit = defineEmits<{ close: []; busy: [value: boolean] }>();
const inventory = ref<Inventory>({ installed: [], removed: [] });
const busy = ref(false);
const error = ref('');
const selected = ref<string | null>(null);
let disposed = false;
async function run(command: string, args: Record<string, unknown> = {}) {
  if (busy.value) return;
  busy.value = true; error.value = ''; emit('busy', true);
  try {
    const result = await invoke<Inventory | null>(command, { id: props.gameId, folderName: props.world.folder_name, ...args });
    if (!disposed && result) { inventory.value = result; selected.value = null; }
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { busy.value = false; emit('busy', false); }
}
function change(file: string, recover: boolean) {
  if (!props.locked) void run('move_world_datapack', { fileName: file, recover });
}
onMounted(() => { void run('list_world_datapacks'); });
onUnmounted(() => { disposed = true; });
</script>
<template>
  <section class="world-datapacks">
    <button
      class="text-button"
      :disabled="busy"
      @click="emit('close')"
    >
      ← {{ t('worlds.back') }}
    </button>
    <div class="content-heading">
      <h2>{{ t('worlds.datapacksFor', { name: world.name }) }}</h2><button
        class="secondary-button"
        :disabled="busy || locked"
        @click="run('import_world_datapacks')"
      >
        {{ t('worlds.importDatapacks') }}
      </button>
    </div>
    <p class="setting-hint">
      {{ t('worlds.datapackSelection') }}
    </p>
    <p
      v-if="locked"
      class="setting-hint"
    >
      {{ t('worlds.datapacksLocked') }}
    </p>
    <button
      class="text-button"
      :disabled="busy"
      @click="run('list_world_datapacks')"
    >
      {{ busy ? t('worlds.loading') : t('worlds.refresh') }}
    </button>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <ul class="packs">
      <li
        v-for="pack in inventory.installed"
        :key="pack.file_name"
      >
        <div class="mod-info">
          <h3>{{ pack.name }}</h3><p
            v-if="pack.description"
            class="mod-meta"
          >
            {{ pack.description }}
          </p><p class="mod-filename">
            {{ pack.file_name }}
          </p>
        </div>
        <button
          class="text-button danger-text"
          :disabled="busy || locked"
          @click="selected = pack.file_name"
        >
          {{ t('worlds.removeDatapack') }}
        </button>
      </li>
    </ul>
    <p
      v-if="!busy && !inventory.installed.length"
      class="content-empty"
    >
      {{ t('worlds.noDatapacks') }}
    </p>
    <div
      v-if="selected"
      class="world-replace-confirm"
    >
      <p>{{ t('worlds.removeDatapackConfirm', { name: selected }) }}</p>
      <button
        class="secondary-button danger-text"
        :disabled="busy || locked"
        @click="change(selected, false)"
      >
        {{ t('worlds.removeDatapack') }}
      </button>
      <button
        class="text-button"
        :disabled="busy"
        @click="selected = null"
      >
        {{ t('worlds.cancel') }}
      </button>
    </div>
    <details v-if="inventory.removed.length">
      <summary>{{ t('worlds.removedDatapacks') }} · {{ inventory.removed.length }}</summary><ul class="packs">
        <li
          v-for="pack in inventory.removed"
          :key="pack.file_name"
        >
          <span class="mod-info">{{ pack.file_name }}</span><button
            class="text-button"
            :disabled="busy || locked"
            @click="change(pack.file_name, true)"
          >
            {{ t('worlds.recoverDatapack') }}
          </button>
        </li>
      </ul>
    </details>
  </section>
</template>
<style scoped>
.world-datapacks>.content-heading{margin-top:20px}.world-datapacks>.setting-hint{margin:12px 0}.packs{list-style:none;padding:0}.packs li{display:flex;align-items:center;gap:16px;padding:15px 0;border-bottom:1px solid var(--line)}.packs h3{font-size:14px;margin:0;overflow-wrap:anywhere}.packs .text-button{flex-shrink:0}.world-replace-confirm{padding:16px;border:1px solid var(--line);border-radius:10px;font-size:13px}.world-replace-confirm button{margin:10px 14px 0 0}details{margin-top:24px}summary{cursor:pointer;font-size:13px}.world-datapacks .secondary-button{min-height:36px;padding:0 13px}
</style>
