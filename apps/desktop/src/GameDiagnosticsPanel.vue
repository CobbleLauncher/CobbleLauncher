<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { pluginLocale } from './extensions/contributions';
import { computed, onUnmounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { PluginReport } from './plugin-reports';
import ReportResults from './ReportResults.vue';
const props = defineProps<{ gameId: string; disabled?: boolean; externalTrigger?: boolean }>();
const emit = defineEmits<{ extensions: []; runtime: []; mods: [] }>();
const pending = ref(false);
const cancelling = ref(false);
const report = ref<PluginReport | null>(null);
const error = ref('');
const unavailable = computed(() => error.value.includes('请先启用支持此文件类型的分析插件'));
const runtime = computed(() => report.value?.findings.some(finding => ['java_class_version', 'java_heap_exhausted'].includes(finding.code)));
const mods = computed(() => report.value?.findings.some(finding => ['fabric_required_dependency_missing', 'forge_required_dependency_missing'].includes(finding.code)));
let disposed = false;
async function analyze() {
  if (props.disabled || pending.value) return;
  pending.value = true; error.value = ''; report.value = null;
  try {
    const result = await invoke<PluginReport | null>('analyze_game_report', { gameId: props.gameId, locale: pluginLocale() });
    if (!disposed) report.value = result;
  } catch (cause) { if (!disposed) error.value = String(cause); }
  finally { pending.value = false; cancelling.value = false; }
}
async function cancel() {
  cancelling.value = true;
  try { await invoke('cancel_plugin_report'); }
  catch (cause) { if (!disposed) { error.value = String(cause); cancelling.value = false; } }
}
defineExpose({analyze, pending});
onUnmounted(() => { disposed = true; if (pending.value) void invoke('cancel_plugin_report').catch(() => {}); });
</script>

<template>
  <div
    v-if="!externalTrigger || pending || report || error"
    class="game-diagnostics"
    :aria-busy="pending"
  >
    <div class="diagnostic-actions">
      <button
        v-if="!externalTrigger"
        class="secondary-button"
        :disabled="disabled || pending"
        @click="analyze"
      >
        {{ pending ? t('reports.running') : error ? t('reports.retry') : t('reports.analyzeLogs') }}
      </button>
      <button
        v-if="pending"
        class="secondary-button"
        :disabled="cancelling"
        @click="cancel"
      >
        {{ t('reports.cancel') }}
      </button>
      <button
        v-if="unavailable"
        class="secondary-button"
        @click="emit('extensions')"
      >
        {{ t('reports.extensions') }}
      </button>
    </div>
    <p
      v-if="error"
      role="alert"
      class="settings-error"
    >
      {{ unavailable ? t('reports.unavailable') : error }}
    </p>
    <ReportResults
      v-if="report"
      :report="report"
    />
    <div
      v-if="report && (runtime || mods)"
      class="diagnostic-actions"
    >
      <button
        v-if="runtime"
        class="secondary-button"
        @click="emit('runtime')"
      >
        {{ t('reports.runtime') }}
      </button>
      <button
        v-if="mods"
        class="secondary-button"
        @click="emit('mods')"
      >
        {{ t('reports.mods') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.game-diagnostics{min-width:0;padding:16px;border:1px solid var(--line);border-radius:11px;margin:16px 0}.diagnostic-actions{display:flex;gap:12px;align-items:center;flex-wrap:wrap}
</style>
