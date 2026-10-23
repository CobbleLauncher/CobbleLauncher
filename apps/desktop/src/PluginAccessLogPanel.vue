<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t, locale } from './i18n';
import { accessTarget, accessTime, type PluginAccessEntry } from './plugin-access-log';
const props = defineProps<{ pluginId: string; embedded?: boolean }>();
const entries = ref<PluginAccessEntry[]>([]);
const busy = ref(false);
const failed = ref(false);
let generation = 0;
async function load() {
  const current = ++generation;
  busy.value = true;
  failed.value = false;
  try {
    const result = await invoke<PluginAccessEntry[]>('plugin_access_log', { pluginId: props.pluginId });
    if (current === generation) entries.value = result;
  } catch { if (current === generation) failed.value = true; }
  finally { if (current === generation) busy.value = false; }
}
watch(() => props.pluginId, () => { entries.value = []; void load(); }, { immediate: true });
onUnmounted(() => { ++generation; });
</script>
<template>
  <section
    class="plugin-access-log"
    :aria-busy="busy"
  >
    <div class="plugin-access-log-heading">
      <h2 v-if="!embedded">
        {{ t('pluginAccessLog.title') }}
      </h2>
      <button
        type="button"
        class="secondary-button"
        :disabled="busy"
        @click="load"
      >
        {{ t('pluginAccessLog.refresh') }}
      </button>
    </div>
    <p
      v-if="failed"
      role="status"
    >
      {{ t('pluginAccessLog.failed') }}
    </p>
    <p v-else-if="!busy && !entries.length">
      {{ t('pluginAccessLog.empty') }}
    </p>
    <ol
      v-else
      class="plugin-access-log-list"
    >
      <li
        v-for="(entry, index) in entries"
        :key="`${entry.timestamp_ms}-${index}`"
      >
        <div><span>{{ t(`pluginAccessLog.actions.${entry.action.replaceAll('.', '_')}`, entry.action) }}</span><span>{{ t(`pluginAccessLog.outcomes.${entry.outcome}`) }}</span></div>
        <div><span v-if="entry.target">{{ accessTarget(entry) }}</span><time :datetime="new Date(entry.timestamp_ms).toISOString()">{{ accessTime(entry.timestamp_ms, locale) }}</time></div>
      </li>
    </ol>
  </section>
</template>
<style scoped>
.plugin-access-log-heading, .plugin-access-log-list li > div { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.plugin-access-log-heading h2 { margin: 0; font-size: 1rem; }
.plugin-access-log-list { list-style: none; margin: 12px 0 0; padding: 0; max-height: 360px; overflow: auto; }
.plugin-access-log-list li { padding: 10px 0; border-bottom: 1px solid var(--ui-border); }
.plugin-access-log-list li > div:last-child { margin-top: 4px; opacity: .7; font-size: .85rem; overflow-wrap: anywhere; flex-wrap: wrap; }
.plugin-access-log-list time { margin-left: auto; }
</style>

