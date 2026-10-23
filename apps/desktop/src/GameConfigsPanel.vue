<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui';
import type { Game } from './games';
import { locale, t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import ConfigDiagnostics from './ConfigDiagnostics.vue';
import { configTextBytes, MAX_CONFIG_BYTES, type GameConfigFile, type GameConfigDocument, type GameConfigBackup } from './game-configs';
const props = defineProps<{ game: Game; locked?: boolean }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const files = ref<GameConfigFile[]>([]); const query = ref(''); const offset = ref(0); const pageSize = 50;
const document = ref<GameConfigDocument | null>(null); const draft = ref(''); const selectedId = ref('');
const listing = ref(false); const reading = ref(false); const mutating = ref(false); const error = ref(''); const listError = ref(''); const saved = ref(false);
const operation = ref<'save' | 'restore' | null>(null);
const diagnosing = ref(false);
const backupId = ref(''); const restoreTarget = ref<{ document: GameConfigDocument; backup: GameConfigBackup } | null>(null);
const leavePrompt = ref(false); let leaveResolver: ((value: boolean) => void) | null = null;
let context = 0; let listRequest = 0; let readRequest = 0; let disposed = false;
const gameRunning = computed(() => props.game.launch_state === 'preparing' || props.game.launch_state === 'running');
const locked = computed(() => Boolean(props.locked || props.game.content_installing || gameRunning.value));
const lockMessage = computed(() => t(gameRunning.value ? 'gameConfigs.locked' : 'gameConfigs.operationLocked'));
const dirty = computed(() => document.value !== null && draft.value !== document.value.text);
const tooLarge = computed(() => configTextBytes(draft.value) > MAX_CONFIG_BYTES);
const matched = computed(() => files.value.filter(file => file.name.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())));
const visibleFiles = computed(() => matched.value.slice(offset.value, offset.value + pageSize));
const canSave = computed(() => dirty.value && !tooLarge.value && !locked.value && !mutating.value && !reading.value);
watch(query, () => { offset.value = 0; });
watch(() => mutating.value || diagnosing.value, value => emit('busy', value), { flush: 'sync' });
function accept(value: GameConfigDocument) {
  document.value = value; draft.value = value.text; selectedId.value = value.id; backupId.value = '';
  files.value = files.value.map(file => file.id === value.id ? { ...file, name: value.name, size: configTextBytes(value.text) } : file);
}
function mutationError(cause: unknown, operation: 'save' | 'restore' = 'save') {
  if (cause === 'CONFIG_CHANGED') return t('gameConfigs.changed');
  if (cause === 'CONFIG_BUSY') return lockMessage.value;
  return t(operation === 'restore' ? 'gameConfigs.restoreFailed' : 'gameConfigs.saveFailed');
}
async function listFiles() {
  const version = context; const request = ++listRequest; const gameId = props.game.id;
  listing.value = true; listError.value = '';
  try { const value = await invoke<GameConfigFile[]>('list_game_configs', { gameId }); if (!disposed && context === version && listRequest === request) files.value = value; }
  catch { if (!disposed && context === version && listRequest === request) listError.value = t('gameConfigs.listFailed'); }
  finally { if (!disposed && context === version && listRequest === request) listing.value = false; }
}
async function readFile(id: string) {
  const version = context; const request = ++readRequest; const gameId = props.game.id;
  selectedId.value = id; document.value = null; draft.value = ''; reading.value = true; error.value = ''; saved.value = false;
  try {
    const value = await invoke<GameConfigDocument>('read_game_config', { gameId, configId: id });
    if (disposed || context !== version || readRequest !== request) return;
    if (value.id !== id) throw new Error('Invalid config identity');
    accept(value);
  } catch { if (!disposed && context === version && readRequest === request) error.value = t('gameConfigs.readFailed'); }
  finally { if (!disposed && context === version && readRequest === request) reading.value = false; }
}
async function selectFile(id: string) {
  if (id === selectedId.value && document.value) return;
  if (!await confirmLeave()) return;
  await readFile(id);
}
function revert() { if (document.value && !mutating.value) { draft.value = document.value.text; error.value = ''; saved.value = false; } }
async function save(): Promise<boolean> {
  if (!canSave.value || !document.value) return !dirty.value;
  const version = context; const gameId = props.game.id; const current = document.value; const text = draft.value;
  operation.value = 'save'; mutating.value = true; error.value = ''; saved.value = false;
  try {
    const value = await invoke<GameConfigDocument>('save_game_config', { gameId, configId: current.id, text, expectedRevision: current.revision });
    if (disposed || version !== context) return false;
    if (value.id !== current.id) throw new Error('Invalid config identity');
    accept(value); saved.value = true; return true;
  } catch (cause) { if (!disposed && version === context) error.value = mutationError(cause); return false; }
  finally { if (!disposed && version === context) { mutating.value = false; operation.value = null; } }
}
async function reload() { if (selectedId.value && await confirmLeave()) await readFile(selectedId.value); }
function canLeave() { return !dirty.value && !mutating.value && !diagnosing.value && !restoreTarget.value; }
async function confirmLeave(): Promise<boolean> {
  if (mutating.value || diagnosing.value || restoreTarget.value) return false;
  if (!dirty.value) return true;
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
async function saveAndLeave() { if (await save()) finishLeave(true); }
function discardAndLeave() { revert(); finishLeave(true); }
async function requestRestore() {
  if (locked.value || mutating.value || !document.value || !backupId.value) return;
  const selected = document.value.backups.find(backup => backup.id === backupId.value); if (!selected) return;
  if (!await confirmLeave() || locked.value || !document.value) return;
  restoreTarget.value = { document: document.value, backup: selected };
}
async function restore() {
  const target = restoreTarget.value; if (!target || locked.value || mutating.value) return;
  const version = context; const gameId = props.game.id;
  operation.value = 'restore'; mutating.value = true; error.value = ''; saved.value = false;
  try {
    const value = await invoke<GameConfigDocument>('restore_game_config_backup', { gameId, configId: target.document.id, backupId: target.backup.id, expectedRevision: target.document.revision });
    if (disposed || version !== context) return;
    if (value.id !== target.document.id) throw new Error('Invalid config identity');
    accept(value); saved.value = true; restoreTarget.value = null;
  } catch (cause) { if (!disposed && version === context) { error.value = mutationError(cause, 'restore'); restoreTarget.value = null; } }
  finally { if (!disposed && version === context) { mutating.value = false; operation.value = null; } }
}
function backupLabel(backup: GameConfigBackup) { return new Date(backup.createdAt).toLocaleString(locale.value); }
watch(() => props.game.id, () => {
  ++context; ++readRequest; ++listRequest; files.value = []; query.value = ''; offset.value = 0; document.value = null; draft.value = ''; selectedId.value = ''; backupId.value = ''; restoreTarget.value = null; saved.value = false; error.value = ''; reading.value = false; mutating.value = false;
  finishLeave(false); void listFiles();
}, { immediate: true });
onUnmounted(() => { disposed = true; ++context; leaveResolver?.(false); if (mutating.value) emit('busy', false); });
defineExpose({ canLeave, confirmLeave });
</script>
<template>
  <section
    class="game-configs-panel"
    :aria-busy="listing || reading || mutating"
  >
    <p
      v-if="locked"
      class="setting-hint"
      role="status"
    >
      {{ lockMessage }}
    </p>
    <div class="configs-layout">
      <aside class="config-files">
        <label class="config-search"><AppIcon name="search" /><input
          v-model="query"
          type="search"
          :aria-label="t('gameConfigs.search')"
          :placeholder="t('gameConfigs.search')"
        ><AppIconButton
          v-if="query"
          icon="x"
          :label="t('tools.clearSearch')"
          @click="query = ''"
        /></label>
        <LoadingIndicator
          v-if="listing"
          :label="t('gameConfigs.loading')"
          compact
        />
        <p
          v-else-if="!files.length && !listError"
          class="setting-hint"
        >
          {{ t('gameConfigs.empty') }}
        </p>
        <p
          v-else-if="!matched.length && !listError"
          class="setting-hint"
        >
          {{ t('gameConfigs.noMatches') }}
        </p>
        <ul>
          <li
            v-for="file in visibleFiles"
            :key="file.id"
          >
            <button
              class="config-file"
              :aria-pressed="selectedId === file.id"
              :disabled="mutating"
              @click="selectFile(file.id)"
            >
              {{ file.name }}
            </button>
          </li>
        </ul>
        <div
          v-if="matched.length > pageSize"
          class="config-pagination"
        >
          <button
            class="text-button"
            :disabled="offset === 0"
            @click="offset -= pageSize"
          >
            {{ t('gameConfigs.previous') }}
          </button><span>{{ offset + 1 }}–{{ Math.min(offset + pageSize, matched.length) }} / {{ matched.length }}</span><button
            class="text-button"
            :disabled="offset + pageSize >= matched.length"
            @click="offset += pageSize"
          >
            {{ t('gameConfigs.next') }}
          </button>
        </div>
        <p
          v-if="listError"
          class="settings-error"
          role="alert"
        >
          {{ listError }}
        </p><button
          v-if="listError"
          class="text-button"
          @click="listFiles"
        >
          {{ t('plugins.retry') }}
        </button>
      </aside>
      <div class="config-editor">
        <LoadingIndicator
          v-if="reading"
          :label="t('gameConfigs.reading')"
          compact
        />
        <template v-if="document">
          <div class="config-editor-heading">
            <h2>{{ document.name }}</h2><AppIconButton
              icon="refresh-cw"
              :label="t('gameConfigs.reload')"
              :disabled="mutating"
              @click="reload"
            />
          </div>
          <textarea
            v-model="draft"
            class="config-text"
            :aria-label="t('gameConfigs.editor', { name: document.name })"
            :aria-invalid="tooLarge"
            :readonly="locked || mutating"
            spellcheck="false"
            @input="saved = false"
          />
          <p
            v-if="tooLarge"
            class="settings-error"
            role="alert"
          >
            {{ t('gameConfigs.tooLarge') }}
          </p>
          <div class="config-actions">
            <button
              class="secondary-button"
              :disabled="!canSave"
              @click="save"
            >
              <AppIcon
                v-if="operation === 'save'"
                name="loader-circle"
                class="ui-busy-icon"
              />{{ operation === 'save' ? t('gameConfigs.saving') : t('gameConfigs.save') }}
            </button><button
              class="text-button"
              :disabled="!dirty || mutating"
              @click="revert"
            >
              {{ t('gameConfigs.revert') }}
            </button>
          </div>
          <div
            v-if="document.backups.length"
            class="config-backups"
          >
            <label for="config-backup">{{ t('gameConfigs.backup') }}</label><select
              id="config-backup"
              v-model="backupId"
              :disabled="locked || mutating"
            >
              <option
                value=""
                disabled
              >
                {{ t('gameConfigs.chooseBackup') }}
              </option><option
                v-for="backup in document.backups"
                :key="backup.id"
                :value="backup.id"
              >
                {{ backupLabel(backup) }}
              </option>
            </select><button
              class="text-button"
              :disabled="!backupId || locked || mutating"
              @click="requestRestore"
            >
              {{ t('gameConfigs.restore') }}
            </button>
          </div>
          <p
            v-if="saved"
            class="setting-hint"
            role="status"
          >
            {{ t('gameConfigs.saved') }}
          </p>
          <ConfigDiagnostics
            :game-id="game.id"
            :document="document"
            :disabled="dirty || mutating || reading"
            @busy="diagnosing = $event"
          />
        </template>
        <p
          v-else-if="!reading && !error && files.length"
          class="setting-hint"
        >
          {{ t('gameConfigs.chooseFile') }}
        </p>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }}
        </p><button
          v-if="error && !document && selectedId"
          class="text-button"
          :disabled="reading"
          @click="readFile(selectedId)"
        >
          {{ t('plugins.retry') }}
        </button>
      </div>
    </div>
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="config-dialog-overlay" /><DialogContent
          class="config-dialog"
          :aria-describedby="undefined"
        >
          <DialogTitle>{{ t('gameConfigs.leaveTitle') }}</DialogTitle><p
            v-if="error"
            class="settings-error"
            role="alert"
          >
            {{ error }}
          </p><div class="config-actions">
            <button
              class="secondary-button"
              :disabled="!canSave"
              @click="saveAndLeave"
            >
              <AppIcon
                v-if="operation === 'save'"
                name="loader-circle"
                class="ui-busy-icon"
              />{{ operation === 'save' ? t('gameConfigs.saving') : t('gameConfigs.save') }}
            </button><button
              class="text-button"
              :disabled="mutating"
              @click="discardAndLeave"
            >
              {{ t('settingsLayout.discard') }}
            </button><button
              class="text-button"
              :disabled="mutating"
              @click="finishLeave(false)"
            >
              {{ t('settingsLayout.continueEditing') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
    <DialogRoot
      :open="!!restoreTarget"
      @update:open="!$event && !mutating && (restoreTarget = null)"
    >
      <DialogPortal>
        <DialogOverlay class="config-dialog-overlay" /><DialogContent
          class="config-dialog"
          :aria-describedby="undefined"
        >
          <DialogTitle>{{ t('gameConfigs.restoreTitle', { name: restoreTarget?.document.name ?? '' }) }}</DialogTitle><p>{{ t('gameConfigs.restoreWarning') }}</p><div class="config-actions">
            <button
              class="secondary-button"
              :disabled="locked || mutating"
              @click="restore"
            >
              <AppIcon
                v-if="operation === 'restore'"
                name="loader-circle"
                class="ui-busy-icon"
              />{{ operation === 'restore' ? t('gameConfigs.restoring') : t('gameConfigs.restore') }}
            </button><button
              class="text-button"
              :disabled="mutating"
              @click="restoreTarget = null"
            >
              {{ t('settingsLayout.cancel') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
  </section>
</template>
<style scoped>
.game-configs-panel{min-width:0}.configs-layout{display:grid;grid-template-columns:minmax(180px,260px) minmax(0,1fr);gap:20px;margin-top:14px}.config-files,.config-editor{min-width:0}.config-search{display:flex;gap:8px;align-items:center;padding:0 10px;min-height:40px;border:1px solid var(--line);border-radius:8px;background:var(--panel)}.config-search>svg{flex:none;width:16px;height:16px}.config-search input{width:100%;min-width:0;flex:1;background:transparent;border:0;outline:0;font:inherit;font-size:14px;color:var(--ink)}.config-search input::-webkit-search-cancel-button,.config-search input::-webkit-search-decoration{appearance:none}.config-files ul{list-style:none;padding:0;margin:12px 0}.config-file{display:block;width:100%;padding:10px;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--ink);font:inherit;font-size:14px;text-align:left;overflow-wrap:anywhere;cursor:pointer}.config-file[aria-pressed=true]{background:var(--panel);border-color:var(--accent)}.config-editor-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:10px}.config-editor-heading h2{font-size:14px;margin:0;overflow-wrap:anywhere;min-width:0}.config-text{display:block;width:100%;min-height:360px;resize:vertical;box-sizing:border-box;padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--panel);color:var(--ink);font:14px/1.6 ui-monospace,SFMono-Regular,Consolas,monospace;tab-size:2}.config-actions button{display:inline-flex;align-items:center;justify-content:center;gap:7px}.config-actions button>svg{width:15px;height:15px;flex:none}.config-actions{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-top:12px}.config-backups{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-top:20px}.config-backups label{font-size:14px}.config-backups select{min-width:0;max-width:100%;padding:8px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:14px}.config-pagination{display:flex;gap:8px;align-items:center;flex-wrap:wrap;font-size:14px}.config-dialog-overlay{position:fixed;inset:0;background:#0006;z-index:100}.config-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(460px,calc(100vw - 40px));box-sizing:border-box;padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}.config-dialog h2{font-size:16px;margin:0}.config-dialog p{font-size:14px;line-height:1.6}@media(max-width:760px){.configs-layout{grid-template-columns:minmax(0,1fr)}.config-text{min-height:320px}}
</style>
