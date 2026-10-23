<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from './games';
import { pluginLocale } from './extensions/contributions';
import { t as globalT } from './i18n';
import type { PluginReport } from './plugin-reports';
import ReportResults from './ReportResults.vue';
import AppIcon from './ui/AppIcon.vue';
import { isActiveTask, taskPhaseLabel, type TaskSnapshot } from './tasks';
const props = defineProps<{ game: Game; disabled?: boolean; tasks?: TaskSnapshot[] }>();
const emit = defineEmits<{ login: []; runtime: []; retry: []; extensions: []; repaired: [game: Game]; busy: [value: boolean] }>();
const { t } = useI18n({ useScope: 'local', messages: {
  'en-US': { login: 'Sign in again', java: 'Choose compatible Java', repair: 'Repair game files', repairing: 'Repairing game files…', analyzing: 'Analyzing…', cancel: 'Cancel analysis', retry: 'Try launch again', log: 'Open launch log', analyze: 'Analyze this failure', extensions: 'Install or enable Crash Doctor', unavailable: 'Crash Doctor is unavailable.' },
  'zh-CN': { login: '重新登录', java: '选择兼容的 Java', repair: '修复游戏文件', repairing: '正在修复游戏文件…', analyzing: '正在分析…', cancel: '取消分析', retry: '重新启动', log: '打开本次日志', analyze: '分析本次故障', extensions: '安装或启用 Crash Doctor', unavailable: 'Crash Doctor 不可用。' },
} });
type Recovery = { kind: 'login' | 'java' | 'managed_artifacts' | 'unknown'; current_log: boolean; can_repair: boolean };
const recovery = ref<Recovery | null>(null);
const pending = ref(false);
const startedRepair = ref<TaskSnapshot | null>(null);
const repairTask = computed(() => props.tasks?.find(task => task.id === startedRepair.value?.id)
  ?? props.tasks?.find(task => task.game_id === props.game.id && task.kind === 'game_repair' && isActiveTask(task)) ?? startedRepair.value);
const repairing = computed(() => !!repairTask.value && isActiveTask(repairTask.value));
const activeAction = ref<'repair' | 'log' | 'analyze' | null>(null);
const analyzing = ref(false);
const cancelling = ref(false);
const error = ref('');
const report = ref<PluginReport | null>(null);
const unavailable = ref(false);
const blocked = computed(() => props.disabled || pending.value || repairing.value || props.game.launch_state !== 'idle' || props.game.content_installing);
let revision = 0;
watch(() => [props.game.id, props.game.launch_error, props.game.log_path], async () => {
  const request = ++revision;
  recovery.value = null; report.value = null; error.value = ''; unavailable.value = false;
  if (!props.game.launch_error) return;
  try { const result = await invoke<Recovery>('get_launch_recovery', { id: props.game.id }); if (request === revision) recovery.value = result; }
  catch { /* A launch rejected before preparation has no host recovery evidence. */ }
}, { immediate: true });
async function run(action: 'repair' | 'log' | 'analyze') {
  if (blocked.value) return;
  const request = revision;
  pending.value = true; activeAction.value = action; error.value = ''; unavailable.value = false; emit('busy', true);
  try {
    if (action === 'repair') startedRepair.value = await invoke<TaskSnapshot>('repair_game_artifacts', { id: props.game.id });
    else if (action === 'log') await invoke('open_current_launch_log', { id: props.game.id });
    else {
      const plugins = await invoke<{ manifest: { id: string }; enabled: boolean; problem?: string }[]>('list_plugins');
      if (!plugins.some(plugin => plugin.manifest.id === 'crash-doctor' && plugin.enabled && !plugin.problem)) {
        unavailable.value = true; error.value = t('unavailable'); return;
      }
      analyzing.value = true;
      const result = await invoke<PluginReport | null>('analyze_game_report', { gameId: props.game.id, locale: pluginLocale(), currentLaunch: true });
      if (request === revision) report.value = result;
    }
  } catch (cause) {
    if (request === revision) {
      unavailable.value = String(cause).includes('请先启用支持此文件类型的分析插件');
      error.value = unavailable.value ? t('unavailable') : String(cause);
    }
  } finally { pending.value = false; activeAction.value = null; analyzing.value = false; cancelling.value = false; emit('busy', false); }
}
async function cancel() {
  if (!analyzing.value || cancelling.value) return;
  cancelling.value = true;
  try { await invoke('cancel_plugin_report'); } catch (cause) { error.value = String(cause); cancelling.value = false; }
}
async function cancelRepair() {
  if (!repairTask.value || !repairing.value || repairTask.value.state === 'cancelling') return;
  try { startedRepair.value = await invoke<TaskSnapshot>('cancel_task', { taskId: repairTask.value.id }); }
  catch (cause) { error.value = String(cause); }
}
onUnmounted(() => { if (analyzing.value) void invoke('cancel_plugin_report').catch(() => {}); });
</script>
<template>
  <div
    v-if="game.launch_error"
    class="launch-recovery"
    :aria-busy="pending || repairing"
  >
    <p role="alert">
      {{ game.launch_error }}
    </p>
    <div class="recovery-actions">
      <button
        v-if="recovery?.kind === 'login'"
        class="secondary-button"
        :disabled="blocked"
        @click="emit('login')"
      >
        <AppIcon name="user" />{{ t('login') }}
      </button>
      <button
        v-if="recovery?.kind === 'java'"
        class="secondary-button"
        :disabled="blocked"
        @click="emit('runtime')"
      >
        <AppIcon name="cpu" />{{ t('java') }}
      </button>
      <button
        v-if="recovery?.can_repair"
        class="secondary-button"
        :disabled="blocked"
        @click="run('repair')"
      >
        <AppIcon :name="activeAction === 'repair' || repairing ? 'loader-circle' : 'download'" />{{ t(activeAction === 'repair' || repairing ? 'repairing' : 'repair') }}
      </button>
      <button
        class="secondary-button"
        :disabled="blocked"
        @click="emit('retry')"
      >
        <AppIcon name="refresh-cw" />{{ t('retry') }}
      </button>
      <template v-if="recovery?.current_log">
        <button
          class="secondary-button"
          :disabled="blocked"
          @click="run('log')"
        >
          <AppIcon name="external-link" />{{ t('log') }}
        </button>
        <button
          class="secondary-button"
          :disabled="blocked"
          @click="run('analyze')"
        >
          <AppIcon :name="activeAction === 'analyze' ? 'loader-circle' : 'search'" />{{ t(activeAction === 'analyze' ? 'analyzing' : 'analyze') }}
        </button>
      </template>
      <button
        v-if="analyzing"
        class="secondary-button"
        :disabled="cancelling"
        @click="cancel"
      >
        {{ t('cancel') }}
      </button>
      <button
        v-if="unavailable"
        class="secondary-button"
        :disabled="blocked"
        @click="emit('extensions')"
      >
        <AppIcon name="puzzle" />{{ t('extensions') }}
      </button>
    </div>
    <div
      v-if="repairing && repairTask"
      class="repair-progress"
      role="status"
    >
      <div v-if="repairTask.progress" class="repair-meter">
        <span>{{ taskPhaseLabel(repairTask.progress) }}</span>
        <progress
          :value="repairTask.progress.completed"
          :max="Math.max(1, repairTask.progress.total)"
        />
      </div>
      <button
        class="secondary-button"
        :disabled="repairTask.state === 'cancelling'"
        @click="cancelRepair"
      >
        {{ globalT('reports.cancel') }}
      </button>
    </div>
    <p
      v-if="repairTask?.error"
      role="alert"
      class="settings-error"
    >
      {{ repairTask.error }}
    </p>
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
  </div>
</template>
<style scoped>
.launch-recovery{padding:16px;border:1px solid var(--line);border-radius:11px}.launch-recovery>p{margin:0 0 12px;overflow-wrap:anywhere}.recovery-actions{display:flex;flex-wrap:wrap;gap:8px}.settings-error{margin-top:12px!important}
.repair-progress{display:flex;align-items:center;gap:16px;margin-top:16px}.repair-meter{display:grid;flex:1;min-width:0;gap:8px;font-size:14px}.repair-meter progress{appearance:none;width:100%;height:5px;border:0;border-radius:4px;overflow:hidden;background:var(--ui-surface-2);accent-color:var(--accent)}.repair-meter progress::-webkit-progress-bar{background:var(--ui-surface-2)}.repair-meter progress::-webkit-progress-value{background:var(--accent);border-radius:4px}
</style>
