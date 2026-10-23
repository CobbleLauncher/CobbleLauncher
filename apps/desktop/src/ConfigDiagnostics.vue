<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { pluginLocale } from './extensions/contributions';
import type { GameConfigDocument } from './game-configs';
import type { CatalogManifest } from './plugin-catalogs';
import { isReportPlugin, type PluginReport } from './plugin-reports';
import ReportResults from './ReportResults.vue';
import AppIcon from './ui/AppIcon.vue';

const props = defineProps<{ gameId: string; document: GameConfigDocument; disabled?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const plugins = ref<{ manifest: CatalogManifest; enabled: boolean; problem?: string }[]>([]);
const selected = ref('');
const busy = ref(false);
const report = ref<PluginReport | null>(null);
const error = ref('');
let generation = 0;
let disposed = false;
const compatible = computed(() => plugins.value.filter(plugin => plugin.enabled && !plugin.problem
  && isReportPlugin(plugin.manifest)
  && plugin.manifest.file_extensions.includes(props.document.name.split('.').pop()?.toLowerCase() ?? '')));
watch(compatible, choices => { if (!choices.some(item => item.manifest.id === selected.value)) selected.value = choices[0]?.manifest.id ?? ''; });
watch(() => [props.gameId, props.document.id, props.document.revision], () => { ++generation; report.value = null; error.value = ''; });
onMounted(async () => {
  try { const value = await invoke<typeof plugins.value>('list_plugins'); if (!disposed) plugins.value = value ?? []; }
  catch { /* Diagnosis is optional; the configuration editor remains available. */ }
});
onUnmounted(() => { disposed = true; ++generation; });
async function analyze() {
  if (busy.value || props.disabled || !selected.value) return;
  const current = generation;
  busy.value = true; emit('busy', true); error.value = ''; report.value = null;
  try {
    const value = await invoke<PluginReport | null>('analyze_game_config_report', {
      gameId: props.gameId, configId: props.document.id, expectedRevision: props.document.revision,
      pluginId: selected.value, locale: pluginLocale(),
    });
    if (!disposed && generation === current) report.value = value;
  } catch { if (!disposed && generation === current) error.value = t('gameConfigs.diagnoseFailed'); }
  finally { busy.value = false; if (!disposed) emit('busy', false); }
}
async function cancel() {
  try { await invoke('cancel_plugin_report'); }
  catch { error.value = t('gameConfigs.diagnoseFailed'); }
}
</script>

<template>
  <section
    v-if="compatible.length"
    class="config-diagnostics"
    :aria-busy="busy"
  >
    <div class="config-diagnostics-actions">
      <select
        v-if="compatible.length > 1"
        v-model="selected"
        :disabled="busy"
        :aria-label="t('gameConfigs.diagnosticTool')"
      >
        <option
          v-for="plugin in compatible"
          :key="plugin.manifest.id"
          :value="plugin.manifest.id"
        >
          {{ plugin.manifest.name }}
        </option>
      </select>
      <button
        class="secondary-button"
        :disabled="disabled || busy"
        @click="analyze"
      >
        <AppIcon
          :name="busy ? 'loader-circle' : 'search'"
          :class="{ 'ui-busy-icon': busy }"
        />
        {{ t(busy ? 'reports.running' : 'gameConfigs.diagnose') }}
      </button>
      <button
        v-if="busy"
        class="text-button"
        @click="cancel"
      >
        {{ t('reports.cancel') }}
      </button>
    </div>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <ReportResults
      v-if="report"
      :report="report"
    />
  </section>
</template>

<style scoped>
.config-diagnostics{display:grid;gap:12px;border-top:1px solid var(--border);padding-top:16px}.config-diagnostics-actions{display:flex;flex-wrap:wrap;align-items:center;gap:10px}.config-diagnostics-actions select{max-width:100%}
</style>
