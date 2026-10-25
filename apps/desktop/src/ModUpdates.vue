<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import CurseForgeUpdates from './CurseForgeUpdates.vue';
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Game, ManagedGameMod, ModRecognitionResult, ModUpdateCandidate, ModUpdateCheck, ModUpdatePreview, ModUpdateResult, ModVersion } from './games';
import ModVersionPicker from './ModVersionPicker.vue';
import ModVersionPolicy from './ModVersionPolicy.vue';
const props = defineProps<{ game: Game; locked: boolean; revision?: number }>();
const emit = defineEmits<{ busy: [value: boolean]; recognizing: [value: boolean]; updated: [] }>();
const updates = ref<ModUpdateCandidate[]>([]);
const curseForgeUpdates = ref<InstanceType<typeof CurseForgeUpdates> | null>(null);
const cfCount = ref(0);
const cfFailed = ref(false);
const cfBusy = ref(false);
function curseForgeBusy(value: boolean) { cfBusy.value = value; emit('busy', value); }
const selected = ref<string[]>([]);
const checked = ref(false);
const checking = ref(false);
const managedCount = ref(0);
const checkError = ref('');
const preparing = ref(false);
const applying = ref(false);
const preview = ref<ModUpdatePreview | null>(null);
const planError = ref('');
const lastRequested = ref<string[]>([]);
const result = ref<ModUpdateResult | null>(null);
const recognizing = ref(false);
const recognitionResult = ref<ModRecognitionResult | null>(null);
const recognitionError = ref('');
const versionMode = ref(false);
const managed = ref<ManagedGameMod[]>([]);
const loadingManaged = ref(false);
const managedError = ref('');
const pinning = ref(false);
const selectedManaged = ref<ManagedGameMod | null>(null);
const managedVersion = ref<ModVersion | null>(null);
const preferredVersionId = ref<string | undefined>();
const planKind = ref<'update' | 'version'>('update');
const versionChanged = ref(false);
const requestedFilename = ref<string | null>(null);
const selectedCurseForge = ref(false);
const policyBusy = ref(false);
let managedRequest = 0;
let request = 0;
const allSelected = computed(() => updates.value.length > 0 && updates.value.every((update) => selected.value.includes(update.project_id)));
const working = computed(() => checking.value || preparing.value || applying.value || recognizing.value || cfBusy.value || policyBusy.value);
function policyChanged() { ++request; preview.value = null; updates.value = []; selected.value = []; checked.value = false; checkError.value = ''; curseForgeUpdates.value?.reset(); }
function policyWorking(value: boolean) { policyBusy.value = value; emit('busy', value); }
async function setPinned(mod: ManagedGameMod) {
  if (working.value || pinning.value || props.locked) return;
  pinning.value = true; managedError.value = '';
  try {
    await invoke('set_game_mod_pinned', { id: props.game.id, projectId: mod.project_id, pinned: !mod.pinned });
    await loadManaged();
    if (selectedManaged.value?.project_id === mod.project_id) selectedManaged.value = managed.value.find(item => item.project_id === mod.project_id) ?? null;
    preview.value = null;
    updates.value = updates.value.filter(update => update.project_id !== mod.project_id);
    selected.value = selected.value.filter(id => id !== mod.project_id);
  } catch { managedError.value = t('modUpdates.pinFailed'); }
  finally { pinning.value = false; }
}
function toggleAll(checked: boolean) { selected.value = checked ? updates.value.map((update) => update.project_id) : []; }
function toggle(id: string, checked: boolean) { selected.value = checked ? [...new Set([...selected.value, id])] : selected.value.filter((value) => value !== id); }
async function loadManaged() {
  const current = ++managedRequest;
  loadingManaged.value = true; managedError.value = '';
  try {
    const value = await invoke<ManagedGameMod[]>('list_managed_game_mods', { id: props.game.id });
    if (current === managedRequest) managed.value = value;
  } catch { if (current === managedRequest) managedError.value = t('modUpdates.loadFailed'); }
  finally { if (current === managedRequest) loadingManaged.value = false; }
}
async function openInstalledVersion(filename: string) {
  if (working.value) return;
  ++request; preview.value = null; result.value = null; preparing.value = false;
  versionMode.value = true; versionChanged.value = false; planError.value = '';
  selectedManaged.value = null; managedVersion.value = null; preferredVersionId.value = undefined;
  requestedFilename.value = filename;
  selectedCurseForge.value = false;
  const gameId = props.game.id;
  const current = managedRequest + 1;
  await loadManaged();
  if (current !== managedRequest || gameId !== props.game.id || !versionMode.value || managedError.value) return;
  const normalized = filename.replace(/\.disabled$/i, '');
  const mod = managed.value.find(item => item.filename === normalized);
  if (mod) manageVersion(mod);
  else {
    try {
      const curseForge = await invoke<ManagedGameMod[]>('list_game_curseforge_mods', { id: gameId });
      if (current !== managedRequest || gameId !== props.game.id || !versionMode.value) return;
      selectedCurseForge.value = curseForge.some(item => item.filename === normalized);
      managedError.value = t(selectedCurseForge.value ? 'modUpdates.curseForgeVersions' : 'modUpdates.recognizeForVersions');
    } catch { if (current === managedRequest) managedError.value = t('modUpdates.loadFailed'); }
  }
}
async function recognizeSelected() {
  const filename = requestedFilename.value;
  const gameId = props.game.id;
  await recognize();
  if (filename && gameId === props.game.id && filename === requestedFilename.value && !recognitionError.value) await openInstalledVersion(filename);
}
defineExpose({ openInstalledVersion });
function openVersionManager() {
  if (working.value) return;
  requestedFilename.value = null; versionMode.value = true; versionChanged.value = false; planError.value = ''; selectedManaged.value = null; managedVersion.value = null;
  void loadManaged();
}
function manageVersion(mod: ManagedGameMod) {
  if (working.value) return;
  preferredVersionId.value = undefined; selectedManaged.value = mod; managedVersion.value = null; planError.value = ''; versionChanged.value = false;
}
async function viewUpdateVersion(update: ModUpdateCandidate) {
  if (working.value) return;
  versionMode.value = true; selectedManaged.value = null; managedVersion.value = null; planError.value = ''; versionChanged.value = false;
  const current = managedRequest + 1;
  await loadManaged();
  if (current !== managedRequest || !versionMode.value || managedError.value) return;
  const mod = managed.value.find((item) => item.project_id === update.project_id);
  if (!mod) { managedError.value = t('modUpdates.missingRecord'); return; }
  preferredVersionId.value = update.version_id; selectedManaged.value = { ...mod, name: mod.name ?? update.name };
}
function chooseVersion(value: ModVersion | null) {
  ++request; managedVersion.value = value; preparing.value = false; planError.value = '';
  if (planKind.value === 'version') preview.value = null;
}
function leaveVersionManager() {
  if (applying.value) return;
  ++request; preparing.value = false; planError.value = '';
  if (selectedManaged.value) { selectedManaged.value = null; managedVersion.value = null; }
  else { ++managedRequest; loadingManaged.value = false; versionMode.value = false; }
}
async function prepareVersion() {
  if (!selectedManaged.value || !managedVersion.value || working.value || props.locked || selectedManaged.value.pinned || selectedManaged.value.version_id === managedVersion.value.version_id) return;
  const current = ++request;
  planKind.value = 'version'; preparing.value = true; planError.value = ''; versionChanged.value = false;
  try {
    const value = await invoke<ModUpdatePreview>('preview_game_mod_version_changes', { id: props.game.id, selections: [{ project_id: selectedManaged.value.project_id, version_id: managedVersion.value.version_id }] });
    if (current === request) preview.value = value;
  } catch { if (current === request) planError.value = t('modUpdates.prepareFailed'); }
  finally { if (current === request) preparing.value = false; }
}
async function recognize() {
  if (working.value || props.locked) return;
  const current = ++request;
  recognizing.value = true; recognitionError.value = ''; recognitionResult.value = null;
  emit('recognizing', true); emit('busy', true);
  try {
    const value = await invoke<ModRecognitionResult>('recognize_game_mods', { id: props.game.id });
    if (current !== request) return;
    recognitionResult.value = value;
    updates.value = []; selected.value = []; preview.value = null; checked.value = false;
    managedCount.value = 0; lastRequested.value = []; checkError.value = ''; planError.value = ''; result.value = null;
  } catch { if (current === request) recognitionError.value = t('modUpdates.recognitionFailed'); }
  finally { recognizing.value = false; emit('recognizing', false); emit('busy', false); }
}
async function check() {
  if (working.value || props.locked) return;
  const current = ++request;
  const cfCheck = curseForgeUpdates.value?.check();
  checking.value = true; checkError.value = ''; planError.value = ''; result.value = null; recognitionResult.value = null;
  try {
    const value = await invoke<ModUpdateCheck>('check_game_mod_updates', { id: props.game.id });
    if (current !== request) return;
    updates.value = value.updates; managedCount.value = value.managed_count; checked.value = true;
    selected.value = selected.value.filter((id) => value.updates.some((update) => update.project_id === id));
  } catch { if (current === request) checkError.value = t('modUpdates.checkFailed'); }
  finally { await cfCheck; if (current === request) checking.value = false; }
}
async function prepare(ids: string[]) {
  if (!ids.length || working.value || props.locked) return;
  const current = ++request;
  planKind.value = 'update'; lastRequested.value = [...ids]; preparing.value = true; planError.value = ''; result.value = null;
  try {
    const value = await invoke<ModUpdatePreview>('preview_game_mod_updates', { id: props.game.id, projectIds: ids });
    if (current === request) preview.value = value;
  } catch { if (current === request) planError.value = t('modUpdates.prepareFailed'); }
  finally { if (current === request) preparing.value = false; }
}
async function apply() {
  if (!preview.value || applying.value || props.locked) return;
  applying.value = true; emit('busy', true); planError.value = '';
  try {
    result.value = await invoke<ModUpdateResult>('apply_game_mod_updates', { id: props.game.id, planId: preview.value.plan_id });
    const completed = new Set(preview.value.changes.map((change) => change.project_id));
    updates.value = updates.value.filter((update) => !completed.has(update.project_id));
    selected.value = selected.value.filter((id) => !completed.has(id));
    managedCount.value += result.value.added;
    preview.value = null;
    if (planKind.value === 'version') {
      versionChanged.value = true; result.value = null; selectedManaged.value = null; managedVersion.value = null;
      updates.value = []; selected.value = []; checked.value = false; recognitionResult.value = null;
      await loadManaged();
    }
    emit('updated');
  } catch { planError.value = t('modUpdates.applyFailed'); }
  finally { applying.value = false; emit('busy', false); }
}
function back() { if (!applying.value) { preview.value = null; planError.value = ''; } }
watch(() => props.revision, () => {
  ++request; preview.value = null; planError.value = ''; result.value = null; updates.value = []; selected.value = [];
  checked.value = false; checking.value = false; preparing.value = false; checkError.value = ''; recognitionResult.value = null;
  if (versionMode.value) { selectedManaged.value = null; managedVersion.value = null; void loadManaged(); }
});
watch(() => props.game.id, () => { requestedFilename.value = null; ++request; ++managedRequest; versionMode.value = false; managed.value = []; loadingManaged.value = false; managedError.value = ''; selectedManaged.value = null; managedVersion.value = null; versionChanged.value = false; updates.value = []; selected.value = []; checked.value = false; checking.value = false; preparing.value = false; preview.value = null; checkError.value = ''; planError.value = ''; result.value = null; recognitionResult.value = null; recognitionError.value = ''; });
onUnmounted(() => { ++request; ++managedRequest; });
</script>
<template>
  <div class="mod-updates">
    <ModVersionPolicy
      v-if="!preview"
      :game-id="game.id"
      :project-id="versionMode ? selectedManaged?.project_id : undefined"
      :selected-version="managedVersion && managedVersion.version_id !== selectedManaged?.version_id ? managedVersion : undefined"
      :disabled="locked || working"
      @changed="policyChanged"
      @busy="policyWorking"
    />
    <template v-if="preview">
      <button
        class="text-button"
        :disabled="applying"
        @click="back"
      >
        {{ t(planKind === 'version' ? 'modUpdates.backVersions' : 'modUpdates.backUpdates') }}
      </button>
      <div class="update-plan-heading">
        <h2>{{ t(planKind === 'version' ? 'modUpdates.changeVersion' : 'modUpdates.updatePlan') }} <span class="count">{{ preview.changes.length }}</span></h2>
      </div>
      <ul
        class="mod-list"
        :aria-label="t('modUpdates.changes')"
      >
        <li
          v-for="change in preview.changes"
          :key="change.project_id"
          class="mod-row"
        >
          <div class="mod-info">
            <h3>
              {{ change.name }}<span
                v-if="!change.from_version"
                class="mod-state"
              >{{ t('modUpdates.addedDependency') }}</span><span
                v-if="!change.enabled"
                class="mod-state"
              >{{ t('modUpdates.keepDisabled') }}</span>
            </h3><p class="mod-meta">
              {{ change.from_version ? `${change.from_version} → ${change.to_version}` : change.to_version }}
            </p>
          </div>
        </li>
      </ul>
      <div class="plan-actions">
        <button
          class="primary-button"
          :disabled="applying || locked || !preview.changes.length"
          @click="apply"
        >
          <span
            v-if="applying"
            class="busy-spinner"
            aria-hidden="true"
          />{{ t(applying ? 'modUpdates.applying' : 'modUpdates.confirm') }}
        </button>
      </div>
      <p
        v-if="planKind === 'version'"
        class="setting-hint"
      >
        {{ t('modUpdates.backupBeforeChange') }}
      </p>
    </template>
    <template v-else-if="!versionMode">
      <div class="content-heading">
        <div>
          <h2>{{ t('curseforge.updates') }}</h2>
        </div><button
          class="secondary-button"
          :disabled="working || locked"
          @click="check"
        >
          {{ t(checking ? 'curseforge.checking' : checked ? 'curseforge.checkAgain' : 'curseforge.checkUpdates') }}
        </button>
      </div>
      <div class="mod-recognition-tools">
        <button
          class="text-button"
          :disabled="working"
          @click="openVersionManager"
        >
          {{ t('curseforge.versionManagement') }}
        </button>
        <button
          class="text-button"
          :disabled="working || locked"
          @click="recognize"
        >
          {{ t(recognizing ? 'curseforge.recognizing' : recognitionError ? 'curseforge.retryRecognition' : 'curseforge.recognize') }}
        </button>
      </div>
      <p
        v-if="recognitionResult"
        class="update-result"
        role="status"
      >
        <template v-if="!recognitionResult.recognized && !recognitionResult.unmatched">
          {{ t('modUpdates.noneToRecognize') }}
        </template><template v-else>
          {{ t('modUpdates.recognized', {count: recognitionResult.recognized}) }}<span v-if="recognitionResult.unmatched"> {{ t('modUpdates.unmatched', {count: recognitionResult.unmatched}) }}</span>
        </template>
      </p>
      <p
        v-if="recognitionError"
        class="settings-error"
        role="alert"
      >
        {{ recognitionError }}
      </p>
      <p
        v-if="checkError"
        class="settings-error"
        role="alert"
      >
        {{ checkError }}
      </p>
      <p
        v-if="result"
        class="update-result"
        role="status"
      >
        {{ t('modUpdates.updated', {count: result.updated}) }}<span v-if="result.added"> {{ t('modUpdates.added', {count: result.added}) }}</span>
      </p>
      <template v-if="updates.length">
        <div class="mod-selection-toolbar">
          <label class="mod-selection-label"><input
            type="checkbox"
            :aria-label="t('modUpdates.selectAll')"
            :checked="allSelected"
            :indeterminate="selected.length > 0 && !allSelected"
            :disabled="working || locked"
            @change="toggleAll(($event.target as HTMLInputElement).checked)"
          ><span>{{ t('modUpdates.selectAll') }}</span></label><button
            class="secondary-button"
            :aria-label="t('modUpdates.updateSelected', {count: selected.length})"
            :disabled="!selected.length || working || locked"
            @click="prepare(selected)"
          >
            {{ preparing ? t('modUpdates.preparing') : t('modUpdates.updateSelected', {count: selected.length}) }}
          </button>
        </div>
        <ul
          class="mod-list update-list"
          :aria-label="t('modUpdates.available')"
        >
          <li
            v-for="update in updates"
            :key="update.project_id"
            class="mod-row"
          >
            <input
              class="mod-selection-checkbox"
              type="checkbox"
              :aria-label="t('modUpdates.selectUpdate', {name: update.name})"
              :checked="selected.includes(update.project_id)"
              :disabled="working || locked"
              @change="toggle(update.project_id, ($event.target as HTMLInputElement).checked)"
            ><div class="mod-info">
              <h3>
                {{ update.name }}<span class="mod-state">Modrinth</span><span
                  v-if="update.pack_owned"
                  class="mod-state"
                >{{ t('modUpdates.packOwned') }}</span><span
                  v-if="!update.enabled"
                  class="mod-state"
                >{{ t('modUpdates.disabled') }}</span>
              </h3><p class="mod-meta">
                {{ update.current_version }} <span
                  class="update-arrow"
                  aria-hidden="true"
                >→</span> {{ update.version_number }}
              </p>
            </div><div class="update-version-actions">
              <button
                class="text-button"
                :aria-label="t('modUpdates.viewVersionLog', {name: update.name})"
                :disabled="working"
                @click="viewUpdateVersion(update)"
              >
                {{ t('modUpdates.versionLog') }}
              </button><button
                class="secondary-button"
                :aria-label="t('modUpdates.update', {name: update.name})"
                :disabled="working || locked"
                @click="prepare([update.project_id])"
              >
                {{ t('curseforge.update') }}
              </button>
            </div>
          </li>
        </ul>
      </template>
      <p
        v-else-if="checked && !checking && !cfCount && !cfFailed && !checkError"
        class="content-empty"
      >
        {{ t('curseforge.noUpdates') }}
      </p>
      <p
        v-if="preparing"
        class="setting-hint"
        role="status"
      >
        {{ t('modUpdates.preparingDependencies') }}
      </p>
    </template>
    <CurseForgeUpdates
      v-show="!versionMode && !preview"
      ref="curseForgeUpdates"
      :game-id="game.id"
      :locked="locked || working"
      :revision="props.revision"
      @count="cfCount = $event"
      @failed="cfFailed = $event"
      @busy="curseForgeBusy"
      @updated="emit('updated')"
      @policy-changed="policyChanged"
    />
    <section
      v-show="versionMode && !preview"
      class="managed-mod-versions"
    >
      <button
        class="text-button"
        :disabled="applying"
        @click="leaveVersionManager"
      >
        {{ t(selectedManaged ? 'modUpdates.backList' : 'modUpdates.backUpdates') }}
      </button>
      <template v-if="selectedManaged">
        <div class="managed-version-heading">
          <h2>{{ selectedManaged.name ?? selectedManaged.filename }}</h2><p
            v-if="selectedManaged.name && selectedManaged.name !== selectedManaged.filename"
            class="mod-filename"
          >
            {{ selectedManaged.filename }}
          </p><p
            v-if="!selectedManaged.enabled"
            class="mod-meta"
          >
            {{ t('modUpdates.disabledHelp') }}
          </p>
        </div>
        <button
          class="text-button"
          :disabled="working || pinning || locked"
          @click="setPinned(selectedManaged)"
        >
          {{ t(selectedManaged.pinned ? 'modUpdates.unpin' : 'modUpdates.pin') }}
        </button>
        <ModVersionPicker
          :key="selectedManaged.project_id"
          :game-id="game.id"
          :project-id="selectedManaged.project_id"
          :current-version-id="selectedManaged.version_id"
          :preferred-version-id="preferredVersionId"
          :disabled="applying || preparing"
          @selected="chooseVersion"
        />
        <p
          v-if="managedVersion"
          class="version-change-summary"
        >
          {{ managedVersion.version_id === selectedManaged.version_id ? t('modUpdates.current') : t('modUpdates.changeTo', {version: managedVersion.version_number}) }}
        </p>
        <button
          class="secondary-button mod-version-preview-button"
          :disabled="working || locked || selectedManaged.pinned || !managedVersion || managedVersion.version_id === selectedManaged.version_id"
          @click="prepareVersion"
        >
          {{ t(preparing ? 'modUpdates.preparing' : 'modUpdates.preview') }}
        </button>
        <p
          v-if="locked"
          class="setting-hint"
        >
          {{ t('modUpdates.closeGame') }}
        </p>
      </template>
      <template v-else>
        <div class="managed-version-heading">
          <h2>{{ t('modUpdates.manage') }}</h2>
        </div>
        <p
          v-if="versionChanged"
          class="update-result"
          role="status"
        >
          {{ t('modUpdates.changed') }}
        </p>
        <p
          v-if="loadingManaged"
          class="content-empty"
          role="status"
        >
          {{ t('modUpdates.loading') }}
        </p>
        <div
          v-if="managedError"
          class="inline-error"
          role="alert"
        >
          <p>{{ managedError }}</p><button
            class="text-button"
            @click="loadManaged"
          >
            {{ t('modUpdates.retryLoad') }}
          </button>
          <button
            v-if="requestedFilename && !selectedCurseForge"
            class="secondary-button"
            :disabled="working || locked"
            @click="recognizeSelected"
          >
            {{ t('modUpdates.recognizeSelected') }}
          </button>
          <button
            v-if="selectedCurseForge"
            class="secondary-button"
            :disabled="working || locked"
            @click="versionMode = false; check()"
          >
            {{ t('modUpdates.checkProviderUpdates') }}
          </button>
          <p
            v-if="recognitionError"
            role="alert"
          >
            {{ recognitionError }}
          </p>
        </div>
        <ul
          v-if="managed.length"
          class="mod-list managed-mod-list"
          :aria-label="t('modUpdates.managed')"
        >
          <li
            v-for="mod in managed"
            :key="mod.project_id"
            class="mod-row"
          >
            <div class="mod-info">
              <h3>{{ mod.name ?? mod.filename }}</h3><p
                v-if="mod.name && mod.name !== mod.filename"
                class="mod-filename"
              >
                {{ mod.filename }}
              </p><p
                v-if="!mod.enabled"
                class="mod-meta"
              >
                {{ t('modUpdates.disabled') }}
              </p>
              <p
                v-if="mod.pinned"
                class="mod-meta"
              >
                {{ t('modUpdates.pinned') }}
              </p>
            </div><button
              class="text-button"
              :disabled="loadingManaged || working || pinning || locked"
              @click="setPinned(mod)"
            >
              {{ t(mod.pinned ? 'modUpdates.unpin' : 'modUpdates.pin') }}
            </button><button
              class="secondary-button"
              :aria-label="t('modUpdates.manageNamed', {name: mod.name ?? mod.filename})"
              :disabled="loadingManaged || working"
              @click="manageVersion(mod)"
            >
              {{ t('modUpdates.manageVersion') }}
            </button>
          </li>
        </ul>
        <p
          v-else-if="!loadingManaged && !managedError"
          class="content-empty"
        >
          {{ t('modUpdates.noManaged') }}
        </p>
      </template>
    </section>
    <div
      v-if="planError"
      class="inline-error"
      role="alert"
    >
      <p>{{ planError }}</p><button
        v-if="!preview && !versionMode"
        class="text-button"
        :disabled="working || locked"
        @click="prepare(lastRequested)"
      >
        {{ t('modUpdates.retry') }}
      </button>
    </div>
  </div>
</template>






