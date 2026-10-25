<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from './games';
import type { TaskSnapshot } from './tasks';
import type { GameReadiness, ReadinessPreparation } from './readiness';
import { i18n, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import type { AppIconName } from './ui/icons';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import SpaceEstimateView from './readiness/SpaceEstimateView.vue';
const props = defineProps<{ game: Game }>();
const emit = defineEmits<{ task: [task: TaskSnapshot]; tasks: []; account: []; runtime: [] }>();
const result = ref<GameReadiness | null>(null);
const preparation = ref<ReadinessPreparation | null>(null);
const busy = ref<'check' | 'preview' | 'prepare' | null>(null);
const error = ref('');
const detail = ref('');
const startedTask = ref<string | null>(null);
let generation = 0;
let requestId: string | null = null;
const running = computed(() => ['running', 'preparing', 'verifying'].includes(props.game.launch_state ?? 'idle'));
const changed = computed(() => (result.value?.files.changed_files ?? 0) > 0);
const conditionIcons: Record<string, AppIconName> = { files: 'folder', java: 'cpu', account: 'user', server: 'network' };
function conditionText(code: string) {
  const key = `readiness.conditions.${code}`;
  return t(i18n.global.te(key) ? key : 'readiness.conditions.unknown');
}
function issueLabel(category: string, path: string | null) {
  if (path) return path;
  const key = `readiness.categories.${category}`;
  return t(i18n.global.te(key) ? key : 'readiness.files');
}
const conditions = computed(() => result.value ? [
  { id: 'files' as const, state: result.value.files.ready ? 'ready' : result.value.files.missing_files > 0 ? 'missing' : 'unknown', note: t('readiness.fileCounts', { ready: result.value.files.ready_files, total: result.value.files.checked_files }) },
  ...(['java', 'account', 'server'] as const).map(id => ({ id, state: result.value![id].state, note: conditionText(result.value![id].code) })),
] : []);
async function cancel() {
  ++generation;
  const id = requestId; requestId = null;
  if (busy.value !== 'prepare') busy.value = null;
  if (id) await invoke('cancel_game_readiness_check', { requestId: id }).catch(() => {});
}
async function check(mode: 'check' | 'preview' = 'check') {
  if (busy.value || running.value) return;
  const current = ++generation;
  requestId = crypto.randomUUID(); busy.value = mode; error.value = ''; detail.value = '';
  if (mode === 'check') preparation.value = null;
  try {
    const args = { id: props.game.id, requestId };
    if (mode === 'check') {
      const value = await invoke<GameReadiness>('inspect_game_readiness', args);
      if (current === generation) result.value = value;
    } else {
      const value = await invoke<ReadinessPreparation>('preview_game_readiness', args);
      if (current === generation) preparation.value = value;
    }
  } catch (cause) {
    if (current === generation) { error.value = t('readiness.checkFailed'); detail.value = String(cause); }
  } finally { if (current === generation) { busy.value = null; requestId = null; } }
}
async function prepare() {
  if (busy.value || running.value || !preparation.value?.space.can_proceed || changed.value) return;
  busy.value = 'prepare'; error.value = ''; detail.value = '';
  const current = ++generation;
  try {
    const task = await invoke<TaskSnapshot>('prepare_game_readiness', { id: props.game.id });
    if (current === generation) { startedTask.value = task.id; emit('task', task); }
  } catch (cause) {
    if (current === generation) { error.value = t('readiness.prepareFailed'); detail.value = String(cause); }
  } finally { if (current === generation) busy.value = null; }
}
watch(() => props.game.id, async () => { await cancel(); result.value = null; startedTask.value = null; void check(); }, { immediate: true });
onUnmounted(() => { void cancel(); });
</script>
<template>
  <section class="page-content readiness-page">
    <header class="readiness-heading">
      <div class="readiness-title">
        <span class="readiness-page-icon"><AppIcon
          name="download"
          :size="24"
        /></span><div><h1>{{ t('readiness.title') }}</h1><p>{{ game.name }}</p></div>
      </div><button
        class="secondary-button"
        :disabled="!!busy || running"
        @click="check()"
      >
        <AppIcon name="refresh-cw" />{{ t('readiness.checkAgain') }}
      </button>
    </header>
    <p
      v-if="running"
      class="readiness-notice"
    >
      {{ t('readiness.running') }}
    </p>
    <div
      v-if="busy === 'check' || busy === 'preview'"
      class="readiness-busy"
    >
      <LoadingIndicator :label="t(busy === 'check' ? 'readiness.checking' : 'readiness.planning')" /><button
        class="text-button"
        @click="cancel"
      >
        {{ t('common.cancel') }}
      </button>
    </div>
    <div
      v-if="error"
      class="readiness-error"
      role="alert"
    >
      <p>{{ error }}</p><SettingsDisclosure :title="t('readiness.details')">
        <pre>{{ detail }}</pre>
      </SettingsDisclosure>
    </div>
    <div
      v-if="result"
      class="readiness-layout"
      :class="{ 'readiness-has-plan': preparation }"
    >
      <div class="readiness-content">
        <dl class="readiness-conditions">
          <div
            v-for="condition in conditions"
            :key="condition.id"
            class="readiness-condition"
            :data-state="condition.state"
          >
            <span class="readiness-kind-icon"><AppIcon
              :name="conditionIcons[condition.id]!"
              :size="20"
            /></span><div><dt>{{ t(`readiness.${condition.id}`) }}</dt><dd>{{ condition.note }}</dd></div>
            <span class="readiness-state"><AppIcon :name="condition.state === 'ready' ? 'check' : condition.state === 'missing' ? 'alert-circle' : 'info'" />{{ t(`readiness.states.${condition.state}`) }}</span>
            <button
              v-if="condition.id === 'account' && condition.state !== 'ready'"
              class="secondary-button"
              @click="emit('account')"
            >
              {{ t('readiness.manageAccount') }}
            </button>
            <button
              v-if="condition.id === 'java' && condition.state !== 'ready'"
              class="secondary-button"
              @click="emit('runtime')"
            >
              {{ t('readiness.runtimeSettings') }}
            </button>
          </div>
        </dl>
        <p
          v-if="changed"
          class="readiness-notice"
        >
          {{ t('readiness.changed') }}
        </p>
        <SettingsDisclosure
          v-if="result.files.issues.length"
          icon="alert-circle"
          :title="t('readiness.fileDetails', { count: result.files.issues.length })"
        >
          <ul class="readiness-issues">
            <li
              v-for="(issue, index) in result.files.issues"
              :key="index"
            >
              <span :title="issueLabel(issue.category, issue.path)">{{ issueLabel(issue.category, issue.path) }}</span><span>{{ t(`readiness.issueStates.${['missing', 'changed'].includes(issue.state) ? issue.state : 'unknown'}`) }}</span>
            </li>
          </ul>
          <p v-if="result.files.issues_truncated">
            {{ t('readiness.truncated') }}
          </p>
        </SettingsDisclosure>
      </div>
      <aside class="readiness-next">
        <SpaceEstimateView
          v-if="preparation"
          :space="preparation.space"
        />
        <p
          v-if="preparation && !preparation.requirements.ready"
          class="readiness-notice"
        >
          {{ t('readiness.manualFiles', { count: preparation.requirements.missing.length }) }}
        </p>
        <footer class="readiness-actions">
          <button
            v-if="startedTask"
            class="primary-button"
            @click="emit('tasks')"
          >
            {{ t('readiness.viewTask') }}
          </button>
          <button
            v-else-if="preparation"
            class="primary-button"
            :disabled="!!busy || running || changed || !preparation.space.can_proceed"
            @click="prepare"
          >
            <AppIcon name="download" />{{ t(busy === 'prepare' ? 'readiness.starting' : 'readiness.startPreparation') }}
          </button>
          <button
            v-else-if="!result.files.ready || result.java.state !== 'ready'"
            class="primary-button"
            :disabled="!!busy || running || changed"
            @click="check('preview')"
          >
            <AppIcon name="download" />{{ t('readiness.prepareFiles') }}
          </button>
        </footer>
      </aside>
    </div>
  </section>
</template>
<style scoped>
.readiness-page{min-width:0}.readiness-heading{display:flex;justify-content:space-between;align-items:center;gap:var(--ui-space-4);margin-bottom:var(--ui-space-6)}
.readiness-title{display:flex;align-items:center;gap:var(--ui-space-4);min-width:0}.readiness-page-icon{display:grid;place-items:center;width:48px;height:48px;flex:none;border-radius:var(--ui-radius-lg);background:var(--ui-accent);color:var(--accent-ink)}
.readiness-heading h1{margin:0}.readiness-heading p{margin:4px 0 0;color:var(--ui-text-muted);overflow-wrap:anywhere}
.readiness-layout{max-width:1000px}.readiness-content{min-width:0}.readiness-conditions{margin:0}.readiness-condition{display:grid;grid-template-columns:40px minmax(0,1fr) auto;align-items:center;gap:var(--ui-space-3);padding:var(--ui-space-5) var(--ui-space-3);border-bottom:1px solid var(--ui-border)}
.readiness-condition:first-child{background:var(--ui-surface-1);border-radius:var(--ui-radius-md);border-bottom:0;box-shadow:inset 3px 0 var(--ui-accent)}
.readiness-kind-icon{display:grid;place-items:center;width:40px;height:40px;background:var(--ui-surface-2);color:var(--ui-text-muted);border-radius:var(--ui-radius-md)}
.readiness-condition dt{font-weight:var(--ui-weight-semibold);font-size:var(--ui-text-body)}.readiness-condition dd{margin:4px 0 0;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.readiness-state{display:inline-flex;align-items:center;gap:6px;padding:4px 8px;border-radius:var(--ui-radius-sm);color:var(--ui-text-muted);background:var(--ui-surface-2);font-size:var(--ui-text-caption);white-space:nowrap}
.readiness-condition[data-state=ready] .readiness-state{color:var(--ui-success);background:color-mix(in srgb,var(--ui-success) 10%,var(--ui-surface-1))}.readiness-condition[data-state=missing] .readiness-state{color:var(--ui-warning);background:var(--ui-warning-soft)}
.readiness-condition>button{grid-column:2/-1;justify-self:start;min-height:var(--ui-control-compact);font-size:var(--ui-text-caption);padding-inline:12px}
.readiness-next{min-width:0}.readiness-has-plan .readiness-next{padding:var(--ui-space-5);margin-top:var(--ui-space-5);background:var(--ui-surface-1);border-radius:var(--ui-radius-lg)}
.readiness-busy,.readiness-actions{display:flex;align-items:center;gap:var(--ui-space-3);padding-block:var(--ui-space-4)}.readiness-has-plan .readiness-actions{padding-bottom:0}.readiness-has-plan .readiness-actions>button{width:100%;justify-content:center}
.readiness-notice{padding:var(--ui-space-3) var(--ui-space-4);background:var(--ui-warning-soft);color:var(--ui-warning);border-radius:var(--ui-radius-sm)}
.readiness-error{color:var(--ui-danger)}.readiness-error pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}
.readiness-issues{padding:0;margin:0;list-style:none}.readiness-issues li{display:flex;gap:var(--ui-space-4);justify-content:space-between;padding:var(--ui-space-3) 0;border-bottom:1px solid var(--ui-border);font-size:var(--ui-text-body)}
.readiness-issues li>span:first-child{min-width:0;overflow-wrap:anywhere}.readiness-issues li>span:last-child{flex-shrink:0;color:var(--ui-text-muted);font-size:var(--ui-text-caption)}
@media(min-width:1000px){.readiness-has-plan{max-width:none;display:grid;grid-template-columns:minmax(0,1fr) minmax(280px,340px);align-items:start;gap:var(--ui-space-6)}.readiness-has-plan .readiness-next{margin-top:0}}
@media(max-width:600px){.readiness-heading{align-items:flex-start;gap:var(--ui-space-3)}.readiness-page-icon{display:none}.readiness-heading>button{flex-shrink:0}.readiness-condition{grid-template-columns:32px minmax(0,1fr);padding:var(--ui-space-4) var(--ui-space-3)}.readiness-kind-icon{width:32px;height:32px}.readiness-state{grid-column:2;justify-self:start}.readiness-has-plan .readiness-next{padding:var(--ui-space-4)}}
</style>
