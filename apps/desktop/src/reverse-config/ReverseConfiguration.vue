<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { t, type TranslationKey } from '../i18n';
import { loaderLabel } from '../games';
import MarketImage from '../MarketImage.vue';
import AppIcon from '../ui/AppIcon.vue';
import SearchField from '../ui/SearchField.vue';
import SelectField from '../ui/SelectField.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import { plannedMods, projectKey, type EnvironmentMod, type EnvironmentSource, type ReverseChoice, type ReverseConfiguration } from './types';

const props = defineProps<{ disabled?: boolean }>();
const emit = defineEmits<{ selected: [choice: ReverseChoice | null] }>();
const query = ref('');
const source = ref<EnvironmentSource>('modrinth');
const hits = ref<EnvironmentMod[]>([]);
const selected = ref<EnvironmentMod[]>([]);
const pickerOpen = ref(true);
const total = ref(0);
const searched = ref(false);
const searching = ref(false);
const searchError = ref(false);
const solving = ref(false);
const solveError = ref(false);
const result = ref<ReverseConfiguration | null>(null);
const selectedPlan = ref('');
const visibleChoiceLimit = ref(5);
const visibleChoices = computed(() => result.value?.choices.slice(0, visibleChoiceLimit.value) ?? []);
const choice = computed(() => result.value?.choices.find(item => item.plan_id === selectedPlan.value) ?? null);
const selectedIds = computed(() => new Set(selected.value.map(projectKey)));
const planMods = computed(() => choice.value ? plannedMods(choice.value.plan) : []);
const rootMods = computed(() => planMods.value.filter(mod => selectedIds.value.has(projectKey(mod))));
const dependencyMods = computed(() => planMods.value.filter(mod => !selectedIds.value.has(projectKey(mod))));
const conflictMessages: Record<string, TranslationKey> = {
  no_compatible_release: 'reverseConfig.noCompatibleRelease', no_common_target: 'reverseConfig.noCommonTarget',
  unsatisfiable: 'reverseConfig.unsatisfiable', candidate_limit: 'reverseConfig.candidateLimit',
  cancelled_or_limit: 'reverseConfig.solveLimit', metadata_unavailable: 'reverseConfig.metadataUnavailable',
  installer_unsupported: 'reverseConfig.targetUnsupported',
};
const limitationMessages: Record<string, TranslationKey> = {
  modrinth_only: 'reverseConfig.modrinthOnly', client_metadata_only: 'reverseConfig.clientMetadataOnly',
  stable_unpinned_versions_only: 'reverseConfig.stableOnly', candidate_limit_32_per_project: 'reverseConfig.candidateLimit',
  target_limit_32: 'reverseConfig.targetLimit',
};
let searchGeneration = 0;
let solveGeneration = 0;
let activeRequest = '';
let debounce: ReturnType<typeof setTimeout> | undefined;
let submitted = '';
let nextOffset = 0;
let failedOffset = 0;

async function search(offset = 0) {
  clearTimeout(debounce);
  if (!isTauri() || props.disabled) return;
  const generation = ++searchGeneration;
  if (!offset) { submitted = query.value.trim(); hits.value = []; total.value = 0; }
  failedOffset = offset;
  searching.value = true; searched.value = true; searchError.value = false;
  try {
    const page = await invoke<{ hits: EnvironmentMod[]; offset: number; total_hits: number }>('search_environment_mods', { query: submitted, source: source.value, offset });
    if (generation !== searchGeneration) return;
    const combined = offset ? [...hits.value, ...page.hits] : page.hits;
    hits.value = [...new Map(combined.map(item => [projectKey(item), item])).values()];
    total.value = page.total_hits;
    nextOffset = page.offset + page.hits.length;
    if (!page.hits.length) total.value = nextOffset;
  } catch { if (generation === searchGeneration) searchError.value = true; }
  finally { if (generation === searchGeneration) searching.value = false; }
}
function cancel() {
  ++solveGeneration;
  const requestId = activeRequest;
  activeRequest = '';
  solving.value = false;
  if (requestId) void invoke('cancel_environment_configuration', { requestId }).catch(() => {});
}
function invalidate() {
  cancel(); result.value = null; selectedPlan.value = ''; solveError.value = false; emit('selected', null);
}
function toggle(hit: EnvironmentMod) {
  if (props.disabled) return;
  invalidate();
  selected.value = selectedIds.value.has(projectKey(hit)) ? selected.value.filter(item => projectKey(item) !== projectKey(hit)) : [...selected.value, hit];
}
async function solve() {
  if (!selected.value.length || solving.value || props.disabled || !isTauri()) return;
  invalidate();
  const generation = ++solveGeneration;
  const requestId = crypto.randomUUID();
  activeRequest = requestId; solving.value = true;
  try {
    const response = await invoke<ReverseConfiguration>('reverse_configure_environment', {
      roots: selected.value.map(item => ({ source: item.source, project_id: item.project_id })), preferredGameVersions: [], loaders: ['fabric', 'neoforge', 'forge'], requestId,
    });
    if (generation !== solveGeneration) return;
    result.value = response;
    visibleChoiceLimit.value = 5;
    selectedPlan.value = response.choices[0]?.plan_id ?? '';
    if (response.choices.length) pickerOpen.value = false;
  } catch { if (generation === solveGeneration) solveError.value = true; }
  finally { if (generation === solveGeneration) { solving.value = false; activeRequest = ''; } }
}
function selectPlan(value: unknown) { if (typeof value === 'string' && !props.disabled) selectedPlan.value = value; }
watch(choice, value => emit('selected', value));
watch(query, () => {
  ++searchGeneration; searching.value = false; searchError.value = false; hits.value = []; total.value = 0; searched.value = false;
  clearTimeout(debounce); debounce = setTimeout(() => { void search(); }, 300);
});
watch(source, () => { ++searchGeneration; clearTimeout(debounce); void search(); });
onMounted(() => { void search(); });
onUnmounted(() => { ++searchGeneration; clearTimeout(debounce); cancel(); });
defineExpose({ solve });
</script>

<template>
  <div class="reverse-config">
    <div
      v-if="!pickerOpen"
      class="reverse-selected-summary"
    >
      <span>{{ t('reverseConfig.selectionSummary', { count: selected.length }) }}</span><button
        type="button"
        class="text-button"
        :disabled="disabled"
        @click="pickerOpen = true"
      >
        {{ t('reverseConfig.editSelection') }}
      </button>
    </div>
    <div
      v-show="pickerOpen"
      class="reverse-mod-picker"
    >
      <section
        class="reverse-catalog"
        :aria-label="t('reverseConfig.searchLabel')"
      >
        <div class="reverse-section-heading">
          <h2>{{ t('reverseConfig.pickMods') }}</h2>
        </div>
        <div class="reverse-search">
          <SearchField
            id="reverse-mod-search"
            v-model="query"
            autocomplete="off"
            :placeholder="t('reverseConfig.searchPlaceholder')"
            :label="t('reverseConfig.searchLabel')"
            :disabled="disabled"
            @submit="search()"
          />
          <button
            type="button"
            class="text-button"
            :disabled="disabled"
            @click="search()"
          >
            {{ t('reverseConfig.search') }}
          </button>
        </div>
        <div class="reverse-source">
          <SelectField
            id="reverse-mod-source"
            :model-value="source"
            :label="t('curseforge.provider')"
            :disabled="disabled"
            :options="[{value:'modrinth',label:'Modrinth'},{value:'curseforge',label:'CurseForge'}]"
            @update:model-value="source = $event as EnvironmentSource"
          />
        </div>
        <div
          class="reverse-results"
          :aria-busy="searching"
        >
          <p
            v-if="searching && !hits.length"
            class="reverse-empty"
            role="status"
          >
            <AppIcon name="loader-circle" />{{ t('reverseConfig.searching') }}
          </p>
          <ul
            v-if="hits.length"
            class="reverse-hit-list"
          >
            <li
              v-for="hit in hits"
              :key="projectKey(hit)"
            >
              <button
                type="button"
                class="reverse-hit"
                :class="{ 'is-selected': selectedIds.has(projectKey(hit)) }"
                :aria-pressed="selectedIds.has(projectKey(hit))"
                :disabled="disabled"
                @click="toggle(hit)"
              >
                <MarketImage
                  :provider="hit.source"
                  :url="hit.icon_url"
                  :title="hit.title"
                />
                <span class="reverse-hit-copy"><strong>{{ hit.title }}</strong><span>{{ hit.description }}</span></span>
                <AppIcon :name="selectedIds.has(projectKey(hit)) ? 'check' : 'plus'" />
              </button>
            </li>
          </ul>
          <p
            v-if="searched && !searching && !searchError && !hits.length"
            class="reverse-empty"
          >
            {{ t('reverseConfig.noResults') }}
          </p>
          <div
            v-if="searchError"
            class="reverse-inline-error"
            role="alert"
          >
            <span>{{ t('reverseConfig.searchFailed') }}</span><button
              type="button"
              class="text-button"
              :disabled="disabled"
              @click="search(failedOffset)"
            >
              {{ t('createGame.retry') }}
            </button>
          </div>
          <button
            v-else-if="hits.length && nextOffset < total"
            type="button"
            class="text-button reverse-more"
            :disabled="searching || disabled"
            @click="search(nextOffset)"
          >
            {{ t(searching ? 'reverseConfig.searching' : 'reverseConfig.more') }}
          </button>
        </div>
      </section>
      <section
        class="reverse-selection"
        :aria-label="t('reverseConfig.selectedMods')"
      >
        <div class="reverse-section-heading">
          <h2>{{ t('reverseConfig.selectedMods') }}</h2><span class="count">{{ selected.length }}</span>
        </div>
        <p
          v-if="!selected.length"
          class="reverse-empty reverse-selection-empty"
        >
          <AppIcon
            name="puzzle"
            :size="28"
          />{{ t('reverseConfig.selectPrompt') }}
        </p>
        <ul
          v-else
          class="reverse-selected-list"
        >
          <li
            v-for="mod in selected"
            :key="projectKey(mod)"
          >
            <span>{{ mod.title }}<small>{{ mod.source === 'modrinth' ? 'Modrinth' : 'CurseForge' }}</small></span><button
              type="button"
              class="ui-icon-button"
              :aria-label="t('reverseConfig.removeMod', { name: mod.title })"
              :disabled="disabled"
              @click="toggle(mod)"
            >
              <AppIcon name="x" />
            </button>
          </li>
        </ul>
        <div class="reverse-solve-actions">
          <button
            type="button"
            class="primary-button"
            :disabled="!selected.length || solving || disabled"
            @click="solve"
          >
            <AppIcon :name="solving ? 'loader-circle' : 'search'" />{{ t(solving ? 'reverseConfig.solving' : 'reverseConfig.findConfiguration') }}
          </button><button
            v-if="solving"
            type="button"
            class="text-button"
            @click="cancel"
          >
            {{ t('reverseConfig.cancel') }}
          </button>
        </div>
        <p
          v-if="solveError"
          class="settings-error"
          role="alert"
        >
          {{ t('reverseConfig.solveFailed') }}
        </p>
      </section>
    </div>
    <section
      v-if="result"
      class="reverse-proposals"
      :aria-label="t('reverseConfig.configurations')"
    >
      <template v-if="result.choices.length">
        <div class="reverse-section-heading">
          <h2 id="reverse-proposals-label">
            {{ t('reverseConfig.configurations') }}
          </h2><span class="count">{{ result.choices.length }}</span>
        </div>
        <RadioGroupRoot
          class="reverse-choice-list"
          :model-value="selectedPlan"
          :disabled="disabled"
          aria-labelledby="reverse-proposals-label"
          @update:model-value="selectPlan"
        >
          <RadioGroupItem
            v-for="proposal in visibleChoices"
            :key="proposal.plan_id"
            :value="proposal.plan_id"
            class="reverse-choice"
          >
            <span><strong>Minecraft {{ proposal.game_version }}</strong><span>{{ loaderLabel(proposal.loader) }}</span></span><AppIcon
              v-if="selectedPlan === proposal.plan_id"
              name="check"
            />
          </RadioGroupItem>
        </RadioGroupRoot>
        <button
          v-if="result.choices.length > visibleChoiceLimit"
          type="button"
          class="text-button reverse-more"
          :disabled="disabled"
          @click="visibleChoiceLimit += 5"
        >
          {{ t('reverseConfig.moreCombinations') }}
        </button>
        <template v-if="choice">
          <div class="reverse-plan-heading">
            <h3>{{ t('reverseConfig.installContents') }}</h3><span class="count">{{ planMods.length }}</span>
          </div>
          <ul class="reverse-plan">
            <li
              v-for="mod in rootMods"
              :key="projectKey(mod)"
              :title="`${mod.project_id} · ${mod.version_id}`"
            >
              <span class="reverse-plan-name">{{ mod.name }}<small>{{ mod.source === 'modrinth' ? 'Modrinth' : 'CurseForge' }}</small></span><span class="reverse-mod-version">{{ mod.version_number }}</span>
            </li>
          </ul>
          <SettingsDisclosure
            v-if="dependencyMods.length"
            :key="choice.plan_id"
            :title="t('reverseConfig.dependencies', { count: dependencyMods.length })"
          >
            <ul class="reverse-plan reverse-dependencies">
              <li
                v-for="mod in dependencyMods"
                :key="projectKey(mod)"
              >
                <span class="reverse-plan-name">{{ mod.name }}<small>{{ mod.source === 'modrinth' ? 'Modrinth' : 'CurseForge' }} · {{ t('reverseConfig.requiredDependency') }}</small></span><span class="reverse-mod-version">{{ mod.version_number }}</span>
              </li>
            </ul>
          </SettingsDisclosure>
          <p
            v-if="choice.loader === 'forge' || choice.loader === 'neoforge'"
            class="setting-hint"
          >
            {{ t('reverseConfig.installerNotice', { loader: loaderLabel(choice.loader) }) }}
          </p>
        </template>
      </template>
      <div
        v-else
        class="reverse-no-plan"
        role="status"
      >
        <AppIcon
          name="alert-circle"
          :size="24"
        /><h2>{{ t('reverseConfig.noConfiguration') }}</h2><p>{{ t('reverseConfig.adjustSelection') }}</p>
      </div>
      <details
        v-if="result.conflicts.length"
        class="reverse-conflicts"
      >
        <summary>{{ t('reverseConfig.conflicts', { count: result.conflicts.length }) }}</summary><ul>
          <li
            v-for="(conflict, index) in result.conflicts"
            :key="index"
          >
            <strong v-if="conflict.game_version || conflict.loader">{{ [conflict.game_version, conflict.loader ? loaderLabel(conflict.loader) : ''].filter(Boolean).join(' · ') }}</strong><span>{{ conflictMessages[conflict.code] ? t(conflictMessages[conflict.code]!) : conflict.message }}</span><span
              v-if="conflict.projects.length"
              class="reverse-conflict-projects"
            >{{ conflict.projects.map(project => `${selected.find(mod => projectKey(mod) === projectKey(project))?.title ?? project.project_id} (${project.source === 'modrinth' ? 'Modrinth' : 'CurseForge'})`).join(', ') }}</span>
            <details
              v-if="conflictMessages[conflict.code] && conflict.message && conflict.code !== 'installer_unsupported'"
              class="reverse-conflict-detail"
            >
              <summary>{{ t('reverseConfig.conflictDetails') }}</summary>
              <p>{{ conflict.message }}</p>
            </details>
          </li>
        </ul>
      </details>
      <details
        v-if="result.limitations.length"
        class="reverse-conflicts"
      >
        <summary>{{ t('reverseConfig.metadataLimits') }}</summary><ul>
          <li
            v-for="limitation in result.limitations"
            :key="limitation"
          >
            {{ limitationMessages[limitation] ? t(limitationMessages[limitation]!) : limitation }}
          </li>
        </ul>
      </details>
    </section>
  </div>
</template>

<style scoped>
.reverse-config { display: grid; gap: var(--ui-space-5); }.reverse-selected-summary{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-4);font-size:var(--ui-text-body)}.reverse-selected-list small{display:block;margin-top:var(--ui-space-1);font-size:var(--ui-text-caption);color:var(--ui-text-muted)}
.reverse-mod-picker { display: grid; grid-template-columns: minmax(0, 1.65fr) minmax(210px, 1fr); gap: 24px; }
.reverse-catalog, .reverse-selection { min-width: 0; }
.reverse-section-heading { display: flex; gap: 8px; align-items: center; margin-bottom: 14px; }
.reverse-section-heading h2 { font-size: var(--ui-text-section); letter-spacing: -.2px; }
.reverse-provider { margin-left: auto; color: var(--muted); font-size: var(--ui-text-caption); }
.reverse-search { display: flex; align-items: center; gap: var(--ui-space-2); }.reverse-search .search-field { flex: 1; }.reverse-source { max-width: 180px; margin-top: var(--ui-space-2); }
.reverse-search > .app-icon { color: var(--muted); }
.reverse-search input { min-width: 0; width: 100%; min-height: 42px; border: 0; background: transparent; color: var(--ink); padding: 8px 0; outline-offset: -2px; }
.reverse-search button { min-height: 40px; min-width: 52px; padding: 8px 12px; white-space: nowrap; }
.reverse-results { max-height: 356px; overflow: auto; margin-top: 8px; }
.reverse-hit-list, .reverse-selected-list, .reverse-plan { list-style: none; margin: 0; padding: 0; }
.reverse-hit { display: flex; align-items: center; gap: 12px; width: 100%; text-align: left; border: 0; border-radius: 10px; padding: 10px; background: transparent; }
.reverse-hit:hover, .reverse-hit.is-selected { background: var(--hover); }
.reverse-hit > .app-icon { color: var(--muted); }
.reverse-hit.is-selected > .app-icon { color: var(--accent); }
.reverse-hit-copy { min-width: 0; flex: 1; display: grid; gap: 5px; }
.reverse-hit-copy strong { font-size: var(--ui-text-body); font-weight: 600; overflow-wrap: anywhere; }
.reverse-hit-copy > span { font-size: var(--ui-text-caption); line-height: 1.5; color: var(--muted); overflow: hidden; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.reverse-selected-list { max-height: 294px; overflow: auto; }
.reverse-selected-list li { display: flex; gap: 8px; align-items: center; justify-content: space-between; min-height: 42px; border-bottom: 1px solid var(--line); font-size: var(--ui-text-body); }
.reverse-selected-list li > span { min-width: 0; overflow-wrap: anywhere; }
.reverse-selection-empty { flex-direction: column; text-align: center; min-height: 150px; }
.reverse-empty { display: flex; align-items: center; justify-content: center; gap: 12px; padding: 28px 12px; color: var(--muted); font-size: var(--ui-text-label); line-height: 1.6; }
.reverse-solve-actions { display: flex; align-items: center; gap: 12px; margin-top: 16px; }
.reverse-solve-actions .primary-button { flex: 1; padding: 0 12px; }
.reverse-inline-error { padding: 16px 8px; font-size: var(--ui-text-label); display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
.reverse-more { display: block; margin: 8px auto; }
.reverse-proposals { border-top: 1px solid var(--line); padding-top: 24px; }
.reverse-choice-list { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px; padding: 3px; }
.reverse-choice { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 70px; padding: 14px; text-align: left; background: var(--panel); border: 1px solid var(--line); border-radius: 10px; }
.reverse-choice > span { display: grid; gap: 7px; }
.reverse-choice strong { font-size: var(--ui-text-body); font-weight: 600; }
.reverse-choice span span { font-size: var(--ui-text-label); color: var(--muted); }
.reverse-choice[data-state=checked] { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--panel)); }
.reverse-choice .app-icon { color: var(--accent); }
.reverse-plan-heading { display: flex; align-items: center; gap: 8px; margin: 24px 0 8px; }
.reverse-plan-heading h3 { margin: 0; font-size: var(--ui-text-body); font-weight: 600; }
.reverse-plan { min-width: 0; }
.reverse-plan li { display: flex; justify-content: space-between; align-items: center; gap: 20px; min-height: 43px; padding: 8px 0; border-bottom: 1px solid var(--line); font-size: var(--ui-text-body); }
.reverse-plan-name { min-width: 0; overflow-wrap: anywhere; display: grid; gap: 4px; }
.reverse-plan-name small { color: var(--muted); font-size: var(--ui-text-caption); }
.reverse-mod-version { color: var(--muted); font-size: var(--ui-text-label); text-align: right; overflow-wrap: anywhere; }
.reverse-proposals > .setting-hint, .reverse-plan + .setting-hint { margin-top: 12px; }
.reverse-no-plan { display: grid; justify-items: center; gap: 12px; padding: 16px; text-align: center; }
.reverse-no-plan h2 { font-size: var(--ui-text-section); }
.reverse-no-plan p { font-size: var(--ui-text-label); color: var(--muted); }
.reverse-conflicts { margin-top: 18px; color: var(--muted); font-size: var(--ui-text-label); }
.reverse-conflicts summary { cursor: pointer; padding: 8px 0; }
.reverse-conflicts ul { margin: 4px 0; padding-left: 20px; }
.reverse-conflicts li { padding: 8px 0; line-height: 1.6; }
.reverse-conflicts li strong, .reverse-conflicts li span { display: block; }
.reverse-conflicts li strong { color: var(--ink); font-weight: 500; }
.reverse-conflict-projects { margin-top: 4px; }
@media (max-width: 660px) { .reverse-mod-picker { grid-template-columns: 1fr; gap: 20px; } .reverse-selection-empty { min-height: 0; padding: 12px; flex-direction: row; } .reverse-results { max-height: 290px; } .reverse-selected-list { max-height: 180px; } }
</style>
