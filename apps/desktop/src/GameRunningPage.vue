<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale, t } from './i18n';
import { gameLoaderLabel, type Game, type PlayerProfile } from './games';
import { taskStateLabel, type TaskSnapshot } from './tasks';
import type { ResolvedCommand } from './extensions/contributions';
import { rankUsedTools } from './tool-usage';
import { isRunningToolPinned, rankRunningTools, readRunningToolPins, toggleRunningToolPin } from './running-tool-pins';
import LaunchRecoveryPanel from './LaunchRecoveryPanel.vue';
import AppIcon from './ui/AppIcon.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
const props = defineProps<{ game: Game; starting?: boolean; profile?: PlayerProfile | null; tasks?: TaskSnapshot[]; commands?: readonly ResolvedCommand[]; toolsLoading?: boolean; toolsBusy?: boolean; toolsError?: string }>();
const emit = defineEmits<{ manage: []; share: []; seed: []; extensionCommand: [ResolvedCommand]; play: [id: string]; retrySessionRecovery: [id: string]; login: []; extensions: []; runtime: []; repaired: [Game]; refreshTools: [] }>();
type BackupStatus = { preference: 'enabled' | 'disabled' | 'undecided'; last_backup: string | null; last_error: string | null };
const backup = ref<BackupStatus | null>(null);
const error = ref('');
function message(cause: unknown) { const value = String(cause).trim(); return value.length > 240 ? `${value.slice(0, 237)}…` : value; }
const backupError = ref('');
const busy = ref(false);
const recoveryBusy = ref(false);
const loader = computed(() => props.game.loader_version ? gameLoaderLabel(props.game) : t('runningGame.vanilla'));
const state = computed(() => props.starting || props.game.launch_state === 'preparing' ? 'starting' : props.game.launch_state === 'running' ? 'running' : props.game.launch_state === 'verifying' ? 'verifying' : props.game.launch_state === 'recovery_failed' ? 'recovery_failed' : props.game.launch_error ? 'failed' : 'closed');
const frozen = computed(() => ['starting', 'running', 'verifying', 'recovery_failed'].includes(state.value));
const pins = ref(readRunningToolPins());
const allTools = computed(() => rankRunningTools(rankUsedTools((props.commands ?? []).filter(command => command.pluginId !== 'seed-compass' && (!command.instanceId || command.instanceId === props.game.id))), pins.value[props.game.id] ?? []));
const showAllTools = ref(false);
const tools = computed(() => showAllTools.value ? allTools.value : allTools.value.slice(0, 4));
function toggleToolPin(command: ResolvedCommand) { pins.value = toggleRunningToolPin(pins.value, props.game.id, command); }
const backupTask = computed(() => props.tasks?.filter(task => task.game_id === props.game.id && task.kind === 'world_automatic_backup').sort((a,b) => b.updated_at - a.updated_at)[0]);
const backupTime = computed(() => {
  if (!backup.value?.last_backup) return '';
  const date = new Date(backup.value.last_backup.replace(/\[[^\]]+\]$/, ''));
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString(locale.value);
});
let revision = 0;
watch(() => [props.game.id, props.game.launch_state, backupTask.value?.updated_at], async () => {
  const current = ++revision; backup.value = null; backupError.value = ''; error.value = '';
  showAllTools.value = false;
  try { const value = await invoke<BackupStatus>('get_automatic_world_backup', { gameId: props.game.id }); if (current === revision) backup.value = value; }
  catch (cause) { if (current === revision) backupError.value = message(cause); }
}, { immediate: true });
onUnmounted(() => { revision++; });
async function open(command: 'open_game_log' | 'open_game_directory') {
  if (busy.value) return;
  const current = revision; busy.value = true; error.value = '';
  try { await invoke(command, { id: props.game.id }); }
  catch (cause) { if (current === revision) error.value = message(cause); }
  finally { busy.value = false; }
}
</script>
<template>
  <article class="page-content running-workspace">
    <header class="running-header">
      <div>
        <h1>{{ game.name }}</h1><p class="game-context">
          Minecraft {{ game.version }}<template v-if="loader">
            · {{ loader }}
          </template>
        </p>
      </div>
      <button
        class="secondary-button"
        @click="emit('manage')"
      >
        <AppIcon name="settings" />{{ t('runningGame.manage') }}
      </button>
    </header>
    <section
      class="session-status"
      :data-state="state"
      aria-live="polite"
    >
      <AppIcon :name="state === 'running' ? 'play' : state === 'starting' || state === 'verifying' ? 'loader-circle' : state === 'failed' || state === 'recovery_failed' ? 'alert-circle' : 'check'" />
      <h2>{{ t(`runningGame.${state}`) }}</h2>
      <p v-if="frozen">
        {{ t('runningGame.frozen') }}
      </p>
      <button
        v-if="state === 'running'"
        class="primary-button"
        @click="emit('share')"
      >
        <AppIcon name="network" />{{ t('runningGame.share') }}
      </button>
      <button
        v-if="state === 'recovery_failed'"
        class="secondary-button"
        @click="emit('retrySessionRecovery', game.id)"
      >
        {{ t('runningGame.retry') }}
      </button>
      <button
        v-if="state === 'closed'"
        class="primary-button"
        :disabled="recoveryBusy"
        @click="emit('play', game.id)"
      >
        <AppIcon name="play" />{{ t('runningGame.play') }}
      </button>
    </section>
    <LaunchRecoveryPanel
      v-if="state === 'failed'"
      :game="game"
      :tasks="tasks"
      :disabled="starting"
      @login="emit('login')"
      @runtime="emit('runtime')"
      @retry="emit('play', game.id)"
      @extensions="emit('extensions')"
      @repaired="emit('repaired', $event)"
      @busy="recoveryBusy = $event"
    />
    <p
      v-if="state === 'recovery_failed' && game.launch_error"
      role="alert"
    >
      {{ message(game.launch_error) }}
    </p>
    <div class="session-actions">
      <button
        class="text-button"
        :disabled="toolsLoading || toolsBusy"
        @click="emit('seed')"
      >
        <AppIcon name="compass" />{{ t('runningGame.seed') }}
      </button>
      <button
        class="text-button"
        :disabled="busy || !game.log_path"
        @click="open('open_game_log')"
      >
        <AppIcon name="external-link" />{{ t('runningGame.log') }}
      </button>
      <button
        class="text-button"
        :disabled="busy"
        @click="open('open_game_directory')"
      >
        <AppIcon name="folder" />{{ t('runningGame.folder') }}
      </button>
    </div>
    <p
      v-if="error"
      role="alert"
    >
      {{ error }}
    </p>
    <section
      v-if="allTools.length || toolsLoading || toolsError"
      class="workspace-section"
    >
      <h2>
        <AppIcon name="puzzle" />{{ t('runningGame.tools') }}<span
          v-if="allTools.length"
          class="section-count"
        >{{ allTools.length }}</span>
      </h2><p v-if="toolsLoading">
        {{ t('runningGame.loading') }}
      </p>
      <p
        v-if="toolsError"
        role="alert"
      >
        {{ message(toolsError) }} <button
          class="text-button"
          @click="emit('refreshTools')"
        >
          {{ t('runningGame.retry') }}
        </button>
      </p>
      <div class="tool-grid">
        <div
          v-for="command in tools"
          :key="`${command.pluginId}:${command.commandId}`"
          class="tool-entry"
        >
          <button
            class="text-button tool-action"
            :disabled="toolsBusy"
            @click="emit('extensionCommand', command)"
          >
            <AppIcon name="puzzle" />{{ command.label }}<AppIcon name="chevron-right" />
          </button><button
            class="text-button tool-pin"
            :aria-pressed="isRunningToolPinned(pins, game.id, command)"
            :aria-label="t(isRunningToolPinned(pins, game.id, command) ? 'runningGame.unpinTool' : 'runningGame.pinTool', { name: command.label })"
            @click="toggleToolPin(command)"
          >
            <AppIcon name="pin" />
          </button>
        </div>
      </div>
      <button
        v-if="allTools.length > 4"
        class="text-button tool-expand"
        @click="showAllTools = !showAllTools"
      >
        <AppIcon :name="showAllTools ? 'chevron-down' : 'chevron-right'" />{{ showAllTools ? t('runningGame.showLessTools') : t('runningGame.showAllTools') }}
      </button>
    </section>
    <section class="backup-status">
      <SettingsDisclosure
        :title="t('runningGame.backup')"
        icon="shield"
        :value="backup ? t(`runningGame.${backup.preference}`) : backupError ? undefined : t('runningGame.loading')"
      >
        <p v-if="backupTask">
          {{ taskStateLabel(backupTask) }}
        </p>
        <p
          v-if="backupTime"
          class="game-context"
        >
          {{ t('runningGame.lastBackup', { time: backupTime }) }}
        </p>
      </SettingsDisclosure>
      <p
        v-if="backup?.last_error || backupError"
        role="alert"
      >
        <AppIcon name="alert-circle" />{{ message(backup?.last_error || backupError) }}
      </p>
    </section>
  </article>
</template>
<style scoped>
.running-workspace{min-width:0}.running-header{display:flex;justify-content:space-between;align-items:start;gap:16px}.running-header h1{margin:0;font-size:28px;overflow-wrap:anywhere}.game-context{color:var(--ui-text-muted);font-size:14px}.session-status{margin:20px 0 16px;padding:18px;border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg,16px);background:var(--ui-surface);display:flex;align-items:center;gap:14px;flex-wrap:wrap}.session-status h2{font-size:20px;margin:0}.session-status p{width:100%;margin:0;color:var(--ui-text-muted);font-size:14px}.session-actions{display:flex;flex-wrap:wrap;gap:10px}.workspace-section{margin-top:30px;border-top:1px solid var(--ui-border);padding-top:20px}.workspace-section h2{font-size:17px;margin:0 0 12px;display:flex;align-items:center;gap:8px}.tool-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.tool-action{justify-content:flex-start;min-height:52px}.tool-action :last-child{margin-left:auto}.backup-status p{margin:8px 0}.running-workspace [role=alert]{color:var(--ui-danger);overflow-wrap:anywhere}@media(max-width:600px){.running-workspace{padding:0}.running-header{flex-wrap:wrap}.tool-grid{grid-template-columns:1fr}.session-status{padding:16px}}
.tool-entry{display:flex;align-items:center;gap:4px;min-width:0}.tool-entry .tool-action{flex:1;min-width:0}.tool-pin{display:grid;place-items:center;width:32px;height:40px;flex:none;border-radius:7px}.tool-pin[aria-pressed=true]{color:var(--accent);background:var(--hover)}
</style>
<style scoped>
.running-header h1{font-size:var(--ui-text-page);line-height:var(--ui-leading-page);font-weight:var(--ui-weight-semibold)}.game-context{font-size:var(--ui-text-caption);margin:var(--ui-space-1) 0 0}.session-status{display:grid;grid-template-columns:36px minmax(0,1fr) auto;gap:var(--ui-space-1) var(--ui-space-3);border:0;border-block:1px solid var(--ui-border);border-radius:0;background:transparent;padding:var(--ui-space-5) 0;margin:var(--ui-space-5) 0 var(--ui-space-3)}.session-status>svg{grid-column:1;grid-row:1 / span 2;width:24px;height:24px;align-self:center;color:var(--ui-text-muted)}.session-status[data-state=running]>svg{color:var(--ui-success)}.session-status[data-state=failed]>svg,.session-status[data-state=recovery_failed]>svg{color:var(--ui-danger)}.session-status h2{grid-column:2;grid-row:1;font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold)}.session-status p{grid-column:2;grid-row:2;width:auto;font-size:var(--ui-text-caption)}.session-status>button{grid-column:3;grid-row:1 / span 2}.session-actions{gap:var(--ui-space-2);padding-bottom:var(--ui-space-3)}.session-actions button{color:var(--ui-text-muted)}.workspace-section{margin-top:var(--ui-space-5);padding-top:var(--ui-space-5)}.workspace-section h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section)}.workspace-section h2>svg{width:20px;height:20px;color:var(--ui-text-muted)}.section-count{font-size:var(--ui-text-caption);font-weight:var(--ui-weight-regular);color:var(--ui-text-muted);margin-inline-start:auto}.tool-grid{column-gap:var(--ui-space-5);row-gap:0}.tool-entry{border-bottom:1px solid var(--ui-border);padding-block:var(--ui-space-1)}.tool-action{min-height:44px;background:transparent;border:0;padding-inline:var(--ui-space-2)}.tool-action>:last-child{width:16px;height:16px;color:var(--ui-text-muted)}.tool-pin{color:var(--ui-text-muted)}.tool-expand{display:inline-flex;gap:var(--ui-space-2);align-items:center;margin-top:var(--ui-space-3)}.backup-status{margin-top:var(--ui-space-5)}.backup-status [role=alert]{display:flex;gap:var(--ui-space-2);align-items:center;font-size:var(--ui-text-caption)}@media(max-width:600px){.session-status{grid-template-columns:28px minmax(0,1fr);padding-block:var(--ui-space-4)}.session-status>button{grid-column:2;grid-row:3;justify-self:start;margin-top:var(--ui-space-3)}}
</style>
