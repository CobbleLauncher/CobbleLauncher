<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import { pluginLocale } from './extensions/contributions';
import type { Game } from './games';
import type { GameConfigFile, GameConfigDocument } from './game-configs';
import type { PluginReport, PluginFolderReport } from './plugin-reports';
import ReportResults from './ReportResults.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import AppIcon from './ui/AppIcon.vue';

const props = withDefaults(defineProps<{
  pluginId: string;
  games: Game[];
  instanceId?: string;
  extensions: string[];
  disabled?: boolean;
}>(), { instanceId: undefined, disabled: false });
const emit = defineEmits<{ busy: [value: boolean]; target: [id: string | undefined] }>();
const pageSize = 50;
const maxBatch = 64;
const targetId = ref(props.instanceId ?? '');
const targets = computed(() => props.games.filter(game => game.status === 'installed'));
const target = computed(() => targets.value.find(game => game.id === targetId.value));
const files = ref<GameConfigFile[]>([]);
const loading = ref(false);
const listError = ref(false);
const search = ref('');
const page = ref(0);
const selectedIds = ref<string[]>([]);
const eligible = computed(() => files.value.filter(file =>
  props.extensions.includes(file.name.split('.').pop()?.toLowerCase() ?? '')));
const matching = computed(() => eligible.value.filter(file =>
  file.name.toLocaleLowerCase().includes(search.value.trim().toLocaleLowerCase())));
const rows = computed(() => matching.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
const document = ref<GameConfigDocument | null>(null);
const reading = ref(false);
const readError = ref(false);
const report = ref<PluginReport | null>(null);
const scope = ref<Pick<PluginFolderReport, 'files' | 'skipped' | 'truncated'> | null>(null);
const pending = ref(false);
const cancelling = ref(false);
let reportStarted = false;
const error = ref('');
const hasOutput = computed(() => Boolean(reading.value || readError.value || document.value
  || pending.value || report.value || error.value || scope.value));
let revision = 0;
let readRequest = 0;
let disposed = false;

watch(pending, value => emit('busy', value), { flush: 'sync' });
watch(search, () => { page.value = 0; });

function clearResult() {
  report.value = null;
  scope.value = null;
  error.value = '';
}
function closeSource() {
  ++readRequest;
  reading.value = false;
  readError.value = false;
  document.value = null;
}
function toggle(id: string) {
  if (pending.value || props.disabled) return;
  if (selectedIds.value.includes(id)) {
    selectedIds.value = selectedIds.value.filter(value => value !== id);
  } else if (selectedIds.value.length < maxBatch) {
    selectedIds.value = [...selectedIds.value, id];
  }
}
async function load() {
  if (!target.value) return;
  const current = ++revision;
  const gameId = target.value.id;
  loading.value = true;
  listError.value = false;
  files.value = [];
  selectedIds.value = [];
  page.value = 0;
  closeSource();
  clearResult();
  try {
    const result = await invoke<GameConfigFile[]>('list_game_configs', { gameId });
    if (!disposed && current === revision) files.value = result;
  } catch {
    if (!disposed && current === revision) listError.value = true;
  } finally {
    if (!disposed && current === revision) loading.value = false;
  }
}
async function show(file: GameConfigFile) {
  if (pending.value || props.disabled || !target.value) return;
  const current = revision;
  closeSource();
  clearResult();
  const request = readRequest;
  reading.value = true;
  try {
    const value = await invoke<GameConfigDocument>('read_game_config', {
      gameId: target.value.id, configId: file.id,
    });
    if (!disposed && current === revision && request === readRequest) document.value = value;
  } catch {
    if (!disposed && current === revision && request === readRequest) readError.value = true;
  } finally {
    if (!disposed && current === revision && request === readRequest) reading.value = false;
  }
}
async function check(ids: string[]) {
  if (pending.value || props.disabled || !target.value || !ids.length || ids.length > maxBatch) return;
  const current = revision;
  const gameId = target.value.id;
  const configIds = [...ids];
  const pluginId = props.pluginId;
  pending.value = true;
  reportStarted = false;
  cancelling.value = false;
  closeSource();
  clearResult();
  try {
    if (configIds.length === 1) {
      const value = await invoke<GameConfigDocument>('read_game_config', { gameId, configId: configIds[0] });
      if (disposed || current !== revision || cancelling.value) return;
      reportStarted = true;
      const result = await invoke<PluginReport | null>('analyze_game_config_report', {
        gameId, configId: configIds[0], expectedRevision: value.revision, pluginId, locale: pluginLocale(),
      });
      if (!disposed && current === revision) report.value = result;
    } else {
      reportStarted = true;
      const result = await invoke<PluginFolderReport | null>('analyze_game_config_batch_report', {
        gameId, configIds, pluginId, locale: pluginLocale(),
      });
      if (!disposed && current === revision) {
        report.value = result?.report ?? null;
        scope.value = result ? { files: result.files, skipped: result.skipped, truncated: result.truncated } : null;
      }
    }
  } catch {
    if (!disposed && current === revision) error.value = t('managedConfigReports.checkFailed');
  } finally {
    if (!disposed) {
      pending.value = false;
      reportStarted = false;
      cancelling.value = false;
    }
  }
}
async function cancel() {
  if (!pending.value || cancelling.value) return;
  const current = revision;
  cancelling.value = true;
  if (!reportStarted) return;
  try {
    await invoke('cancel_plugin_report');
  } catch {
    if (!disposed && current === revision) {
      error.value = t('managedConfigReports.cancelFailed');
      cancelling.value = false;
    }
  }
}
watch(() => props.instanceId, value => { targetId.value = value ?? ''; });
watch(() => [target.value?.id, props.pluginId, props.extensions.join(',')] as const, () => {
  ++revision;
  files.value = [];
  selectedIds.value = [];
  search.value = '';
  listError.value = false;
  loading.value = false;
  closeSource();
  clearResult();
  emit('target', target.value?.id);
  if (target.value) void load();
}, { immediate: true });
onUnmounted(() => {
  disposed = true;
  ++revision;
  ++readRequest;
  if (pending.value) emit('busy', false);
});
</script>
<template>
  <section
    class="managed-config-report"
    :aria-busy="loading||pending"
  >
    <label class="managed-target">{{ t('managedConfigReports.game') }}<select
      v-model="targetId"
      :disabled="pending||disabled"
      :aria-label="t('managedConfigReports.game')"
    ><option value="">{{ t('managedConfigReports.chooseGame') }}</option><option
      v-if="targetId&&!target"
      :value="targetId"
      disabled
    >{{ t('managedConfigReports.unavailableGame') }}</option><option
      v-for="game in targets"
      :key="game.id"
      :value="game.id"
    >{{ game.name }}</option></select></label>
    <div
      v-if="target"
      class="managed-config-workspace"
      :class="{'has-output':hasOutput}"
    >
      <div
        v-if="hasOutput"
        class="managed-config-output"
      >
        <LoadingIndicator
          v-if="pending"
          :label="t('managedConfigReports.checking')"
          compact
        />
        <LoadingIndicator
          v-if="reading"
          :label="t('managedConfigReports.reading')"
          compact
        />
        <p
          v-if="readError"
          class="settings-error"
          role="alert"
        >
          {{ t('managedConfigReports.readFailed') }}
        </p>
        <section
          v-if="document"
          class="managed-config-source"
        >
          <div>
            <strong>{{ document.name }}</strong><button
              class="text-button"
              @click="closeSource"
            >
              {{ t('managedConfigReports.closeSource') }}
            </button>
          </div><textarea
            readonly
            :value="document.text"
            :aria-label="t('managedConfigReports.contents')"
            spellcheck="false"
            rows="10"
          />
        </section>
        <p
          v-if="error"
          class="settings-error"
          role="alert"
        >
          {{ error }}
        </p><p
          v-if="scope"
          class="setting-hint"
          role="status"
        >
          {{ t('reports.folderSummary',{files:scope.files,skipped:scope.skipped}) }}<span v-if="scope.truncated"> · {{ t('reports.folderLimited') }}</span>
        </p>
        <ReportResults
          v-if="report"
          :report="report"
        />
      </div>
      <div class="managed-config-browser">
        <template v-if="target">
          <div class="managed-config-toolbar">
            <label class="managed-config-search"><AppIcon name="search" /><input
              v-model="search"
              type="search"
              :aria-label="t('managedConfigReports.search')"
              :placeholder="t('managedConfigReports.search')"
            ></label><button
              class="text-button"
              :disabled="loading||pending||disabled"
              @click="load"
            >
              <AppIcon name="refresh-cw" />{{ t('managedConfigReports.refresh') }}
            </button>
          </div>
          <LoadingIndicator
            v-if="loading"
            :label="t('managedConfigReports.loading')"
            compact
          />
          <div v-else-if="listError">
            <p
              class="settings-error"
              role="alert"
            >
              {{ t('managedConfigReports.listFailed') }}
            </p><button
              class="text-button"
              @click="load"
            >
              {{ t('reports.retry') }}
            </button>
          </div>
          <template v-else>
            <div class="managed-check-actions">
              <button
                class="secondary-button"
                :disabled="pending||disabled||!selectedIds.length"
                @click="check(selectedIds)"
              >
                {{ t('managedConfigReports.checkSelected',{count:selectedIds.length}) }}
              </button><button
                class="text-button"
                :disabled="pending||disabled||!eligible.length||eligible.length>maxBatch"
                @click="check(eligible.map(file=>file.id))"
              >
                {{ t('managedConfigReports.checkAll',{count:eligible.length}) }}
              </button><span
                v-if="selectedIds.length"
                class="ui-caption"
              >{{ selectedIds.length }} / {{ maxBatch }}</span><button
                v-if="selectedIds.length"
                class="text-button"
                :disabled="pending"
                @click="selectedIds=[]"
              >
                {{ t('managedConfigReports.clear') }}
              </button><button
                v-if="pending"
                class="text-button"
                :disabled="cancelling"
                @click="cancel"
              >
                {{ t('reports.cancel') }}
              </button>
            </div>
            <p
              v-if="eligible.length>maxBatch"
              class="setting-hint"
            >
              {{ t('managedConfigReports.limit',{count:maxBatch}) }}
            </p>
            <p
              v-if="!matching.length"
              class="setting-hint"
            >
              {{ t(search.trim()?'managedConfigReports.noMatches':'managedConfigReports.empty') }}
            </p>
            <ul class="managed-config-files">
              <li
                v-for="file in rows"
                :key="file.id"
              >
                <input
                  type="checkbox"
                  :aria-label="t('managedConfigReports.select',{name:file.name})"
                  :checked="selectedIds.includes(file.id)"
                  :disabled="pending||disabled||(!selectedIds.includes(file.id)&&selectedIds.length>=maxBatch)"
                  @change="toggle(file.id)"
                ><button
                  class="managed-config-name"
                  :disabled="pending||disabled"
                  @click="show(file)"
                >
                  {{ file.name }}
                </button>
              </li>
            </ul>
            <nav
              v-if="matching.length>pageSize"
              class="managed-config-pages"
              :aria-label="t('managedConfigReports.pages')"
            >
              <button
                class="text-button"
                :disabled="page===0"
                @click="page--"
              >
                {{ t('reports.previous') }}
              </button><span>{{ page*pageSize+1 }}–{{ Math.min((page+1)*pageSize,matching.length) }} / {{ matching.length }}</span><button
                class="text-button"
                :disabled="(page+1)*pageSize>=matching.length"
                @click="page++"
              >
                {{ t('reports.next') }}
              </button>
            </nav>
          </template>
        </template>
      </div>
    </div>
  </section>
</template>
<style scoped>
.managed-config-report{min-width:0;margin-bottom:var(--ui-space-5)}.managed-target{display:grid;gap:var(--ui-space-2);font-size:var(--ui-text-label);max-width:460px}.managed-target select{width:100%;min-height:var(--ui-control-height);padding:8px 12px;border:1px solid var(--line);border-radius:var(--ui-radius-md);background:var(--panel);color:var(--ink);font:inherit}.managed-config-toolbar,.managed-check-actions,.managed-config-pages{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap;margin:var(--ui-space-4) 0}.managed-config-search{display:flex;align-items:center;gap:var(--ui-space-2);flex:1;min-width:160px;padding:0 10px;border:1px solid var(--line);border-radius:var(--ui-radius-md);background:var(--panel)}.managed-config-search input{width:100%;min-width:0;min-height:var(--ui-control-height);padding:8px 0;border:0;color:var(--ink);background:transparent;font:inherit;font-size:var(--ui-text-label)}.managed-config-search svg,.managed-config-toolbar button svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm);flex:none}.managed-config-toolbar button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.managed-config-search input::-webkit-search-decoration,.managed-config-search input::-webkit-search-cancel-button{appearance:none}.managed-config-files{list-style:none;margin:0;padding:0}.managed-config-files li{display:flex;align-items:center;gap:var(--ui-space-3);border-bottom:1px solid var(--line);padding:4px 0;min-width:0}.managed-config-files input{width:16px;height:16px;flex:none;accent-color:var(--accent)}.managed-config-name{min-height:var(--ui-control-height);padding:6px 4px;border:0;background:transparent;color:var(--ink);font:inherit;font-size:var(--ui-text-label);text-align:left;overflow-wrap:anywhere;cursor:pointer}.managed-config-name:hover{text-decoration:underline}.managed-config-pages{justify-content:flex-end;font-size:var(--ui-text-caption);color:var(--muted)}.managed-config-source{margin:0}.managed-config-source>div{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-2);margin-bottom:var(--ui-space-3);overflow-wrap:anywhere}.managed-config-source strong{font-size:var(--ui-text-label)}.managed-config-source textarea{width:100%;min-width:0;box-sizing:border-box;padding:var(--ui-space-3);border:1px solid var(--line);border-radius:var(--ui-radius-sm);background:var(--bg);color:var(--ink);font:13px/1.6 ui-monospace,Consolas,monospace;resize:vertical}.managed-config-report .setting-hint,.managed-config-report .settings-error{font-size:var(--ui-text-caption)}
</style>

<style scoped>.managed-config-workspace{display:grid;gap:var(--ui-space-5);margin-top:var(--ui-space-4);align-items:start}.managed-config-browser,.managed-config-output{min-width:0}.managed-config-output{padding:var(--ui-space-4);border:1px solid var(--line);border-radius:var(--ui-radius-lg);background:var(--panel)}.managed-config-browser>.managed-config-toolbar{margin-top:0}@media(min-width:900px){.managed-config-workspace.has-output{grid-template-columns:minmax(240px,.85fr) minmax(0,1.15fr);grid-template-areas:'browser output'}.has-output>.managed-config-browser{grid-area:browser}.has-output>.managed-config-output{grid-area:output}}</style>
