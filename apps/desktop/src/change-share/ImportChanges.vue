<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from '../games';
import SelectField from '../ui/SelectField.vue';
import ConfirmationDialog from '../ui/ConfirmationDialog.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import { Puzzle, SlidersHorizontal } from '@lucide/vue';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { label as l } from './labels';
import type { ApplyPreview, ApplyResult, ConfigChoice, ImportResult } from './types';

const props = defineProps<{ game: Game; importedId?: string }>();
const emit = defineEmits<{ applied: [targetId: string, result: ApplyResult] }>();
const games = ref<Game[]>([]);
const targetId = ref(props.game.id);
const imported = ref<ImportResult | null>(null);
const preview = ref<ApplyPreview | null>(null);
const choices = ref<Record<string, 'keep' | 'adopt'>>({});
const busy = ref(false);
const error = ref('');
const done = ref(false);
const result = ref<ApplyResult | null>(null);
const modsUndone = ref(false);
const restoredPaths = ref<string[]>([]);
const recovering = ref(false);
const leavePrompt = ref(false);
let leaveResolver: ((value: boolean) => void) | null = null;
let request = 0;
const gameOptions = computed(() => games.value.filter(game => game.status === 'installed').map(game => ({ value: game.id, label: game.name })));
const target = computed(() => games.value.find(game => game.id === targetId.value) ?? props.game);
const locked = computed(() => target.value.status !== 'installed' || !!target.value.content_installing || target.value.launch_state === 'preparing' || target.value.launch_state === 'running');
const added = computed(() => preview.value?.changes.filter(change => change.fromVersion === null) ?? []);
const replaced = computed(() => preview.value?.changes.filter(change => change.fromVersion !== null) ?? []);
const canApply = computed(() => !!preview.value && !busy.value && !locked.value && !result.value);
function confirmLeave(): Promise<boolean> {
  if (busy.value || recovering.value) return Promise.resolve(false);
  if (!result.value || result.value.complete) return Promise.resolve(true);
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) { leavePrompt.value = false; leaveResolver?.(value); leaveResolver = null; }
onUnmounted(() => { ++request; finishLeave(false); });
defineExpose({ confirmLeave });
function failure(cause: unknown, fallback: string) {
  const code = String(cause);
  if (code.includes('CHANGE_SHARE_STALE')) return l('stale');
  if (code.includes('CHANGE_SHARE_BUSY')) return l('locked');
  if (code.includes('CHANGE_SHARE_INCOMPATIBLE')) return l('incompatible');
  return fallback;
}
async function loadGames() {
  try { games.value = await invoke<Game[]>('list_games'); }
  catch { error.value = l('allGamesFailed'); }
}
async function chooseFile() {
  if (busy.value) return;
  const current = ++request; busy.value = true; error.value = ''; preview.value = null; done.value = false; result.value = null;
  try {
    const value = await invoke<ImportResult | null>('import_game_change');
    if (current !== request) return;
    if (value) imported.value = value;
    else error.value = l('cancelled');
  } catch (cause) { if (current === request) error.value = failure(cause, l('importFailed')); }
  finally { if (current === request) busy.value = false; }
}
async function previewChanges() {
  if (!imported.value || !targetId.value || busy.value || locked.value) return;
  const current = ++request; busy.value = true; error.value = ''; preview.value = null;
  try {
    const value = await invoke<ApplyPreview>('preview_game_change_apply', { id: targetId.value, importId: imported.value.importId });
    if (current !== request) return;
    preview.value = value;
    choices.value = Object.fromEntries(value.configs.map(config => [config.path, 'keep']));
  } catch (cause) { if (current === request) error.value = failure(cause, l('previewFailed')); }
  finally { if (current === request) busy.value = false; }
}
async function apply() {
  if (!canApply.value || !preview.value) return;
  busy.value = true; error.value = '';
  try {
    const configChoices: ConfigChoice[] = preview.value.configs.map(config => ({ path: config.path, choice: choices.value[config.path] ?? 'keep' }));
    const appliedResult = await invoke<ApplyResult>('apply_game_change', { id: targetId.value, planId: preview.value.planId, configChoices });
    result.value = appliedResult;
    done.value = appliedResult.complete;
  } catch (cause) { error.value = failure(cause, l('applyFailed')); }
  finally { busy.value = false; }
}
async function undoMods() {
  if (!result.value?.operationId || recovering.value || modsUndone.value) return;
  recovering.value = true; error.value = '';
  try { await invoke('undo_game_mod_operation', { id: targetId.value, operationId: result.value.operationId }); modsUndone.value = true; }
  catch { error.value = l('recoveryFailed'); }
  finally { recovering.value = false; }
}
async function restoreConfigs() {
  if (!result.value || recovering.value) return;
  recovering.value = true; error.value = '';
  try {
    for (const backup of result.value.configBackups) {
      if (restoredPaths.value.includes(backup.path)) continue;
      if (backup.backupId) await invoke('restore_game_config_backup', { gameId: targetId.value, configId: backup.configId, backupId: backup.backupId, expectedRevision: backup.newRevision });
      else if (backup.undoId) await invoke('undo_game_change_new_config', { id: targetId.value, undoId: backup.undoId });
      else continue;
      restoredPaths.value = [...restoredPaths.value, backup.path];
    }
  } catch { error.value = l('recoveryFailed'); }
  finally { recovering.value = false; }
}
watch(targetId, () => { ++request; preview.value = null; choices.value = {}; error.value = ''; done.value = false; result.value = null; modsUndone.value = false; restoredPaths.value = []; });
watch(() => props.game.id, id => { targetId.value = id; void loadGames(); });
watch(() => props.importedId, id => { if (id) { imported.value = { importId: id, document: { schemaVersion: 1, name: '', gameVersion: '', loader: null, loaderVersion: null, mods: [], configs: [] } }; preview.value = null; } }, { immediate: true });
void loadGames();
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
      {{ error }}
    </p>
    <div
      class="change-share-layout change-share-import-layout"
      :class="{ 'has-preview': !!preview }"
    >
      <aside class="change-share-aside">
        <div class="change-share-field">
          <label for="share-target">{{ l('target') }}</label><SelectField
            id="share-target"
            v-model="targetId"
            :label="l('target')"
            :options="gameOptions"
            :disabled="busy || !!result"
          />
        </div>
        <p
          v-if="!gameOptions.length"
          class="change-share-muted"
        >
          {{ l('noGames') }}
        </p>
        <div class="change-share-field">
          <span>{{ l('file') }}</span><button
            type="button"
            class="secondary-button"
            :disabled="busy || !!result"
            @click="chooseFile"
          >
            {{ imported ? l('chooseAgain') : l('chooseFile') }}
          </button>
        </div>
        <div
          v-if="imported"
          class="change-share-file"
        >
          <strong>{{ imported.document.name || l('file') }}</strong><span>{{ imported.document.gameVersion }} · {{ imported.document.mods.length }} {{ l('mods') }} · {{ imported.document.configs.length }} {{ l('configs') }}</span>
        </div>
      </aside><div class="change-share-main">
        <template v-if="preview">
          <div class="change-share-section-title">
            <h3>
              <Puzzle
                :size="18"
                aria-hidden="true"
              />{{ l('added') }} · {{ added.length }}
            </h3>
          </div>
          <div class="change-share-result-list">
            <p
              v-for="change in added"
              :key="change.projectId"
            >
              <strong>{{ change.name }}</strong><small>{{ change.toVersion }}</small>
            </p>
          </div>
          <div class="change-share-section-title">
            <h3>
              <Puzzle
                :size="18"
                aria-hidden="true"
              />{{ l('replaced') }} · {{ replaced.length }}
            </h3>
          </div>
          <div class="change-share-result-list">
            <p
              v-for="change in replaced"
              :key="change.projectId"
            >
              <strong>{{ change.name }}</strong><small>{{ change.fromVersion }} → {{ change.toVersion }}</small>
            </p>
          </div>
          <SettingsDisclosure
            v-if="preview.skipped.length"
            :title="`${l('skipped')} · ${preview.skipped.length}`"
          >
            <ul>
              <li
                v-for="item in preview.skipped"
                :key="item.projectId"
              >
                {{ item.name }}
              </li>
            </ul>
          </SettingsDisclosure>
          <p
            v-if="preview.additionalLocalMods"
            class="change-share-muted"
          >
            {{ l('localMods') }} · {{ preview.additionalLocalMods }}
          </p>
          <div
            v-if="preview.configs.length"
            class="change-share-section-title"
          >
            <h3>
              <SlidersHorizontal
                :size="18"
                aria-hidden="true"
              />{{ l('conflict') }} · {{ preview.configs.length }}
            </h3>
          </div>
          <div
            v-for="config in preview.configs"
            :key="config.path"
            class="change-share-config"
          >
            <strong>{{ config.path }}</strong><RadioGroupRoot
              v-model="choices[config.path]"
              class="change-share-choice"
              :aria-label="config.path"
              :disabled="busy"
            >
              <RadioGroupItem value="keep">
                {{ l('keep') }}
              </RadioGroupItem><RadioGroupItem value="adopt">
                {{ l('adopt') }}
              </RadioGroupItem>
            </RadioGroupRoot><SettingsDisclosure :title="l('preview')">
              <div class="change-share-compare">
                <div><h4>{{ l('current') }}</h4><pre>{{ config.currentText ?? '—' }}</pre></div><div><h4>{{ l('incoming') }}</h4><pre>{{ config.incomingText }}</pre></div>
              </div>
            </SettingsDisclosure>
          </div>
          <p
            v-if="!added.length && !replaced.length && !preview.configs.length"
            class="change-share-muted"
          >
            {{ l('empty') }}
          </p>
        </template>
        <div
          v-if="result && !result.complete"
          class="change-share-partial"
          role="status"
        >
          <h3>{{ l('partial') }}</h3>
          <p>{{ l('partialConfig') }}</p>
          <p>{{ l('added') }} {{ result.added }} · {{ l('replaced') }} {{ result.updated }} · {{ l('configs') }} {{ result.configsWritten }}</p>
          <p
            v-if="result.configError"
            class="settings-error"
          >
            {{ result.configError }}
          </p>
          <div class="change-share-choice">
            <button
              v-if="result.operationId && !modsUndone"
              class="secondary-button"
              type="button"
              :disabled="recovering"
              @click="undoMods"
            >
              {{ l('undoMods') }}
            </button>
            <button
              v-if="result.configBackups.some(item => (item.backupId || item.undoId) && !restoredPaths.includes(item.path))"
              class="secondary-button"
              type="button"
              :disabled="recovering"
              @click="restoreConfigs"
            >
              {{ l('restoreConfigs') }}
            </button>
          </div>
        </div>
        <p
          v-if="done"
          role="status"
        >
          {{ l('applied') }}
        </p>
      </div>
    </div><div class="change-share-footer">
      <span>{{ target.name }}</span><button
        v-if="!preview"
        class="primary-button"
        type="button"
        :disabled="busy || locked || !imported"
        @click="previewChanges"
      >
        {{ busy ? l('load') : l('preview') }}
      </button><button
        v-else-if="!result"
        class="primary-button"
        type="button"
        :disabled="!canApply"
        @click="apply"
      >
        {{ busy ? l('applying') : l('apply') }}
      </button><button
        v-else
        class="primary-button"
        type="button"
        @click="emit('applied', targetId, result)"
      >
        {{ l('finish') }}
      </button>
    </div>
    <ConfirmationDialog
      :open="leavePrompt"
      :title="l('leavePartial')"
      :description="l('leavePartialHelp')"
      :confirm-label="l('leave')"
      @confirm="finishLeave(true)"
      @cancel="finishLeave(false)"
    />
  </section>
</template>
