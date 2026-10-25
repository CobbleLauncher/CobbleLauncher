<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { TaskSnapshot } from './tasks';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';
import AppIconButton from './ui/AppIconButton.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import SelectField from './ui/SelectField.vue';
import { RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import { PACK_UPDATE_GROUPS, packChangePath, packChangeMatches, packUpdateGroups, type PackUpdateContext, type PackUpdatePreview, type PackUpdateGroup, type JsonMergeSide, type JsonFieldValue } from './pack-update';
const props = defineProps<{gameId: string}>();
const emit = defineEmits<{busy: [value: boolean]; openGame: [id: string]}>();
const context = ref<PackUpdateContext | null>(null); const contextError = ref(false); const contextLoading = ref(false);
const visible = ref(false); const pending = ref(false); const error = ref(''); const preview = ref<PackUpdatePreview | null>(null);
const group = ref<PackUpdateGroup>('version_changed'); const search = ref(''); const page = ref(0); const pageSize = 50;
const copyName = ref(''); const includeWorlds = ref(false); const addedSelections = ref<Record<string,boolean>>({});
const addedSearch = ref(''); const addedPage = ref(0);
const choices = ref<Record<string, 'local' | 'new' | 'merge'>>({}); const taskId = ref(''); let stopTask: UnlistenFn | undefined;
const fieldChoices=ref<Record<string,Record<string,JsonMergeSide>>>({});
const jsonConfigs=computed(()=>new Map((preview.value?.json_configs??[]).map(config=>[config.path,config])));
function fieldValue(value:JsonFieldValue){return value.exists?JSON.stringify(value.value):t('packUpdate.jsonAbsent');}
const replacementChoices = ref<Record<string,string>>({});
const supported = computed(() => context.value?.supported === true);
const groups = computed(() => preview.value ? packUpdateGroups(preview.value) : null);
const matched = computed(() => (groups.value?.[group.value] ?? []).filter(change => packChangeMatches(change, search.value)));
const rows = computed(() => matched.value.slice(page.value * pageSize, (page.value + 1) * pageSize));
const migratable = computed(() => new Set(preview.value?.migratable_paths ?? []));
function isModJar(path:string) { return path.startsWith('mods/') && path.endsWith('.jar'); }
function migrationPath(change: {before: {path:string}|null;after:{path:string}|null}) { const before = change.before?.path; const after = change.after?.path; return before && migratable.value.has(before) ? before : after && migratable.value.has(after) ? after : ''; }
const conflicts = computed(() => preview.value ? [...preview.value.added,...preview.value.removed,...preview.value.version_changed].filter(change => change.local_state !== 'clean' && change.local_state !== 'missing' && migrationPath(change)) : []);
const newModPaths = computed(() => (preview.value?.added ?? []).flatMap(change => change.after && isModJar(change.after.path) ? [change.after.path] : []));
const removedLocalMods = computed(() => (preview.value?.removed ?? []).flatMap(change => change.before && isModJar(change.before.path) && choices.value[packChangePath(change)] === 'local' ? [change.before.path] : []));
const retainedLocalMods = computed(() => [...removedLocalMods.value,...(preview.value?.player_added ?? []).filter(path => isModJar(path) && addedSelections.value[path])]);
const allChosen = computed(() => conflicts.value.every(change => {
  const path=packChangePath(change);const choice=choices.value[path];
  return !!choice && (choice!=='merge' || jsonConfigs.value.get(path)?.fields.every(field=>!field.conflict || !!fieldChoices.value[path]?.[field.pointer]));
}) && (!newModPaths.value.length || retainedLocalMods.value.every(path => !!replacementChoices.value[path])));
function optionClaimed(newPath:string, oldPath:string) { return Object.entries(replacementChoices.value).some(([old,choice]) => old!==oldPath && choice===newPath); }
function replacementOptions(oldPath:string) { return [{value:'both',label:t('packUpdate.keepBothMods')},...newModPaths.value.map(path => ({value:path,label:t('packUpdate.replaceNewMod',{path}),disabled:optionClaimed(path,oldPath)}))]; }
const matchedAdded = computed(() => (preview.value?.player_added ?? []).filter(path => path.toLocaleLowerCase().includes(addedSearch.value.trim().toLocaleLowerCase())));
const addedRows = computed(() => matchedAdded.value.slice(addedPage.value * pageSize,(addedPage.value+1)*pageSize));
const selectedAddedCount = computed(() => Object.values(addedSelections.value).filter(Boolean).length);
watch(addedSearch,() => {addedPage.value=0;});
function preservePaths() {
  const result = new Set((preview.value?.player_added ?? []).filter(path => addedSelections.value[path]));
  for (const change of preview.value?.unchanged ?? []) if (change.local_state !== 'clean' && change.local_state !== 'missing') { const path = migrationPath(change); if (path) result.add(path); }
  for (const change of conflicts.value) if (choices.value[packChangePath(change)] === 'local') result.add(migrationPath(change));
  return [...result];
}
async function startCopy() {
  if (!preview.value || pending.value || !allChosen.value || !copyName.value.trim()) return;
  pending.value = true; error.value = '';
  try {
    const replaceNewMods = newModPaths.value.length ? retainedLocalMods.value.map(oldPath => ({oldPath,newPath:replacementChoices.value[oldPath] === 'both' ? null : replacementChoices.value[oldPath]})) : [];
    const jsonMerges=[...jsonConfigs.value].filter(([path])=>choices.value[path]==='merge').map(([path])=>({path,fields:Object.entries(fieldChoices.value[path]??{}).map(([pointer,side])=>({pointer,side}))}));
    const id = await invoke<string>('start_game_pack_update_copy', {request:{planId:preview.value.plan_id,name:copyName.value.trim(),preservePaths:preservePaths(),replaceNewMods,includeWorlds:includeWorlds.value,...(jsonMerges.length?{jsonMerges}:{})}});
    taskId.value = id; preview.value = null;
    stopTask?.();
    const handleTask = (task:TaskSnapshot) => {
      if (task.id !== id || disposed) return;
      if (task.state === 'succeeded' && task.game_id) { stopTask?.(); stopTask=undefined; emit('openGame',task.game_id); }
      if (['failed','cancelled','interrupted'].includes(task.state)) { stopTask?.(); stopTask=undefined; error.value = task.error ? `${t('packUpdate.copyFailed')} ${task.error}` : t('packUpdate.copyFailed'); }
    };
    stopTask = await listen<TaskSnapshot>('task-updated', event => handleTask(event.payload));
    (await invoke<TaskSnapshot[]>('list_tasks')).forEach(handleTask);
  } catch (cause) { error.value = errorMessage(cause); }
  finally { pending.value = false; }
}
let disposed = false; let request = 0;
watch(pending, value => emit('busy', value), {flush:'sync'});
watch([group, search], () => {page.value = 0;});
async function loadContext() {
  const current = ++request; contextLoading.value = true; contextError.value = false;
  try { const value = await invoke<PackUpdateContext>('get_game_pack_update_context', {gameId:props.gameId}); if (!disposed && current === request) context.value = value; }
  catch { if (!disposed && current === request) contextError.value = true; }
  finally { if (!disposed && current === request) contextLoading.value = false; }
}
function errorMessage(cause: unknown) {
  const code = String(cause); const keys = {PACK_UPDATE_UNSUPPORTED:'unsupported',PACK_UPDATE_SOURCE_CHANGED:'sourceChanged',PACK_UPDATE_STALE:'sourceChanged',PACK_UPDATE_JSON_CHOICE_REQUIRED:'jsonChoiceRequired',PACK_UPDATE_JSON_SELECTION_INVALID:'sourceChanged',PACK_UPDATE_ARCHIVE_TOO_LARGE:'tooLarge',PACK_UPDATE_INVALID_ARCHIVE:'invalidArchive',PACK_UPDATE_IO:'failed',PACK_UPDATE_TOO_MANY_FILES:'tooManyFiles'} as const;
  return t(`packUpdate.${keys[code as keyof typeof keys] ?? 'failed'}`);
}
async function choose() {
  if (!supported.value || pending.value) return;
  const current = request; pending.value = true; error.value = ''; visible.value = true;
  try {
    const result = await invoke<PackUpdatePreview | null>('preview_game_pack_update', {gameId:props.gameId});
    if (disposed || current !== request) return;
    if (!result) { if (!preview.value) visible.value = false; return; }
    preview.value = result; search.value = ''; page.value = 0; choices.value = {}; replacementChoices.value={}; includeWorlds.value = false; taskId.value = ''; addedSearch.value=''; addedPage.value=0;
    fieldChoices.value=Object.fromEntries((result.json_configs??[]).map(config=>[config.path,Object.fromEntries(config.fields.filter(field=>!field.conflict).map(field=>[field.pointer,field.suggested]))]));
    addedSelections.value = Object.fromEntries(result.player_added.map(path => [path,result.baseline_available]));
    copyName.value = `${result.target_name} ${result.target_version}`.slice(0,80);
    const all = packUpdateGroups(result); group.value = PACK_UPDATE_GROUPS.find(id => id !== 'unchanged' && all[id].length) ?? 'version_changed';
  } catch (cause) { if (!disposed && current === request) {preview.value = null; error.value = errorMessage(cause);} }
  finally { if (!disposed && current === request) pending.value = false; }
}
watch(() => props.gameId, () => { context.value = null; preview.value = null; visible.value = false; error.value = ''; pending.value = false; void loadContext(); }, {immediate:true});
onUnmounted(() => {disposed = true; ++request; stopTask?.(); if (pending.value) emit('busy',false);});
defineExpose({choose,supported,pending});
</script>
<template>
  <div
    v-if="contextError"
    class="pack-update-context-error"
  >
    <p
      class="settings-error"
      role="alert"
    >
      {{ t('packUpdate.contextFailed') }}
    </p><button
      class="text-button"
      :disabled="contextLoading"
      @click="loadContext"
    >
      {{ t('plugins.retry') }}
    </button>
  </div>
  <section
    v-if="visible"
    class="pack-update-panel"
    :aria-busy="pending"
  >
    <div class="pack-update-heading">
      <h2>{{ t('packUpdate.title') }}</h2><div>
        <button
          class="text-button"
          :disabled="pending"
          @click="choose"
        >
          {{ t('packUpdate.choose') }}
        </button><AppIconButton
          icon="x"
          :label="t('packUpdate.close')"
          :disabled="pending"
          @click="visible = false"
        />
      </div>
    </div>
    <LoadingIndicator
      v-if="pending"
      :label="t('packUpdate.loading')"
      compact
    />
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <p
      v-if="taskId"
      role="status"
    >
      {{ t('packUpdate.copyStarted') }}
    </p>
    <template v-if="preview && !pending">
      <dl class="pack-update-versions">
        <div><dt>{{ t('packUpdate.installed') }}</dt><dd>{{ context?.name }}<span v-if="context?.name"> · </span>{{ preview.current_version }}</dd></div><div><dt>{{ t('packUpdate.target') }}</dt><dd>{{ preview.target_name }} · {{ preview.target_version }}</dd></div>
      </dl>
      <p
        v-if="preview.baseline_status !== 'verified'"
        class="pack-update-warning"
      >
        {{ t(preview.baseline_status === 'missing' ? 'packUpdate.missingBaseline' : 'packUpdate.untrustedBaseline') }}
      </p>
      <p
        v-if="preview.identity_relation === 'unknown'"
        class="pack-update-warning"
      >
        {{ t('packUpdate.checkIdentity') }}
      </p>
      <p
        v-if="preview.runtime_changed"
        class="pack-update-warning"
      >
        {{ t('packUpdate.runtimeChanged') }}
      </p>
      <p
        v-if="preview.excluded_optional_files"
        class="setting-hint"
      >
        {{ t('packUpdate.optionalExcluded', {count:preview.excluded_optional_files}) }}
      </p>
      <div
        class="pack-update-groups"
        role="group"
        :aria-label="t('packUpdate.groups')"
      >
        <button
          v-for="id in PACK_UPDATE_GROUPS"
          :key="id"
          class="text-button"
          :aria-pressed="group === id"
          @click="group = id"
        >
          {{ t(`packUpdate.${id}`) }} <span>{{ groups?.[id].length ?? 0 }}</span>
        </button>
      </div>
      <label class="pack-update-search"><AppIcon name="search" /><input
        v-model="search"
        type="search"
        :aria-label="t('packUpdate.search')"
        :placeholder="t('packUpdate.search')"
      ></label>
      <p
        v-if="!matched.length"
        class="setting-hint"
      >
        {{ t(search.trim() ? 'packUpdate.noMatches' : 'packUpdate.empty') }}
      </p>
      <ul class="pack-update-files">
        <li
          v-for="(change,index) in rows"
          :key="`${page}:${index}:${packChangePath(change)}`"
        >
          <div class="pack-update-path">
            <strong>{{ packChangePath(change) }}</strong><span
              v-if="change.preserve && choices[packChangePath(change)] !== 'merge' && choices[packChangePath(change)] !== 'new'"
              class="pack-update-badge"
            >{{ t('packUpdate.keepLocal') }}</span>
          </div><p
            v-if="change.before && change.after && change.before.path !== change.after.path"
            class="setting-hint"
          >
            {{ change.before.path }} → {{ change.after.path }}
          </p><p
            v-if="change.before?.version_id || change.after?.version_id"
            class="setting-hint"
          >
            {{ change.before?.version_id ?? '—' }} → {{ change.after?.version_id ?? '—' }}
          </p><span
            v-if="change.local_state !== 'clean'"
            class="setting-hint"
          >{{ t(`packUpdate.local_${change.local_state}`) }}</span>
          <RadioGroupRoot
            v-if="group !== 'unchanged' && migrationPath(change) && change.local_state !== 'clean' && change.local_state !== 'missing'"
            v-model="choices[packChangePath(change)]"
            class="pack-update-choice"
            :aria-label="packChangePath(change)"
          >
            <RadioGroupItem
              value="local"
              class="pack-update-choice-option"
            >
              {{ t('packUpdate.keepLocal') }}
            </RadioGroupItem>
            <RadioGroupItem
              value="new"
              class="pack-update-choice-option"
            >
              {{ t('packUpdate.useNew') }}
            </RadioGroupItem>
            <RadioGroupItem
              v-if="jsonConfigs.has(packChangePath(change))"
              value="merge"
              class="pack-update-choice-option"
            >
              {{ t('packUpdate.jsonMerge') }}
            </RadioGroupItem>
          </RadioGroupRoot>
          <div
            v-if="choices[packChangePath(change)]==='merge' && jsonConfigs.has(packChangePath(change))"
            class="pack-update-json"
          >
            <table>
              <thead><tr><th>{{ t('packUpdate.jsonField') }}</th><th>{{ t('packUpdate.jsonOriginal') }}</th><th>{{ t('packUpdate.jsonLocal') }}</th><th>{{ t('packUpdate.jsonUpstream') }}</th><th>{{ t('packUpdate.jsonUse') }}</th></tr></thead>
              <tbody>
                <tr
                  v-for="field in jsonConfigs.get(packChangePath(change))!.fields"
                  :key="field.pointer"
                >
                  <th scope="row">
                    <code>{{ field.pointer }}</code><span
                      v-if="field.conflict"
                      class="pack-update-badge"
                    >{{ t('packUpdate.conflicts') }}</span>
                  </th>
                  <td :data-label="t('packUpdate.jsonOriginal')">
                    <pre>{{ fieldValue(field.base) }}</pre>
                  </td><td :data-label="t('packUpdate.jsonLocal')">
                    <pre>{{ fieldValue(field.local) }}</pre>
                  </td><td :data-label="t('packUpdate.jsonUpstream')">
                    <pre>{{ fieldValue(field.upstream) }}</pre>
                  </td>
                  <td>
                    <RadioGroupRoot
                      v-model="fieldChoices[packChangePath(change)]![field.pointer]"
                      :aria-label="field.pointer"
                      class="pack-update-choice"
                    >
                      <RadioGroupItem
                        value="local"
                        class="pack-update-choice-option"
                      >
                        {{ t('packUpdate.keepLocal') }}
                      </RadioGroupItem>
                      <RadioGroupItem
                        value="upstream"
                        class="pack-update-choice-option"
                      >
                        {{ t('packUpdate.jsonUpstream') }}
                      </RadioGroupItem>
                    </RadioGroupRoot>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <div
            v-if="change.before?.path && isModJar(change.before.path) && !change.after && choices[packChangePath(change)] === 'local' && newModPaths.length"
            class="pack-update-replacement"
          >
            <SelectField
              :id="`replace-${change.before.path}`"
              :model-value="replacementChoices[change.before.path] ?? ''"
              :label="t('packUpdate.modReplacement')"
              :placeholder="t('packUpdate.chooseReplacement')"
              :options="replacementOptions(change.before.path)"
              @update:model-value="replacementChoices[change.before.path] = $event"
            />
          </div>
        </li>
      </ul>
      <nav
        v-if="matched.length > pageSize"
        class="pack-update-pager"
        :aria-label="t('packUpdate.pages')"
      >
        <button
          class="text-button"
          :disabled="page === 0"
          @click="page--"
        >
          {{ t('packUpdate.previous') }}
        </button><span>{{ page * pageSize + 1 }}–{{ Math.min((page+1)*pageSize,matched.length) }} / {{ matched.length }}</span><button
          class="text-button"
          :disabled="(page+1)*pageSize >= matched.length"
          @click="page++"
        >
          {{ t('packUpdate.next') }}
        </button>
      </nav>
      <div
        v-if="preview.player_added.length"
        class="pack-update-added"
      >
        <p>{{ t('packUpdate.playerAdded', {count:preview.player_added.length}) }} · {{ t('packUpdate.selectedAdded', {count:selectedAddedCount}) }}</p>
        <div class="pack-update-added-tools">
          <input
            v-model="addedSearch"
            type="search"
            :aria-label="t('packUpdate.searchAdded')"
            :placeholder="t('packUpdate.searchAdded')"
          ><button
            class="text-button"
            @click="preview.player_added.forEach(path => addedSelections[path]=true)"
          >
            {{ t('packUpdate.selectAll') }}
          </button><button
            class="text-button"
            @click="preview.player_added.forEach(path => addedSelections[path]=false)"
          >
            {{ t('packUpdate.clearSelection') }}
          </button>
        </div>
        <ul>
          <li
            v-for="path in addedRows"
            :key="path"
          >
            <label><input
              v-model="addedSelections[path]"
              type="checkbox"
            >{{ path }}</label>
            <SelectField
              v-if="isModJar(path) && addedSelections[path] && newModPaths.length"
              :id="`replace-added-${path}`"
              :model-value="replacementChoices[path] ?? ''"
              :label="t('packUpdate.modReplacement')"
              :placeholder="t('packUpdate.chooseReplacement')"
              :options="replacementOptions(path)"
              @update:model-value="replacementChoices[path] = $event"
            />
          </li>
        </ul>
        <nav
          v-if="matchedAdded.length > pageSize"
          class="pack-update-pager"
          :aria-label="t('packUpdate.pages')"
        >
          <button
            class="text-button"
            :disabled="addedPage===0"
            @click="addedPage--"
          >
            {{ t('packUpdate.previous') }}
          </button><span>{{ addedPage*pageSize+1 }}–{{ Math.min((addedPage+1)*pageSize,matchedAdded.length) }} / {{ matchedAdded.length }}</span><button
            class="text-button"
            :disabled="(addedPage+1)*pageSize>=matchedAdded.length"
            @click="addedPage++"
          >
            {{ t('packUpdate.next') }}
          </button>
        </nav>
      </div>
      <div class="pack-update-copy-form">
        <label>{{ t('packUpdate.copyName') }}<input
          v-model="copyName"
          maxlength="80"
          type="text"
        ></label>
        <label><input
          v-model="includeWorlds"
          type="checkbox"
        >{{ t('packUpdate.includeWorlds') }}</label>
        <p
          v-if="includeWorlds && preview.runtime_changed"
          class="pack-update-warning"
        >
          {{ t('packUpdate.worldWarning') }}
        </p>
        <button
          class="primary-button"
          :disabled="pending || !allChosen || !copyName.trim()"
          @click="startCopy"
        >
          {{ t('packUpdate.createCopy') }}
        </button>
      </div>
    </template>
  </section>
</template>
<style scoped>
.pack-update-panel{margin:20px 0;padding:20px;border:1px solid var(--line);border-radius:12px;background:var(--panel);min-width:0}.pack-update-heading,.pack-update-heading>div,.pack-update-path,.pack-update-pager{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.pack-update-heading{justify-content:space-between}.pack-update-heading h2{font-size:16px;margin:0}.pack-update-versions{display:flex;gap:28px;flex-wrap:wrap;margin:18px 0}.pack-update-versions dt{font-size:11px;color:var(--muted)}.pack-update-versions dd{margin:5px 0;font-size:14px;overflow-wrap:anywhere}.pack-update-warning{font-size:12px;color:var(--ink);margin:12px 0}.pack-update-groups{display:flex;flex-wrap:wrap;gap:7px;margin:18px 0 12px}.pack-update-groups [aria-pressed=true]{border-color:var(--accent);background:var(--tint)}.pack-update-groups span{margin-left:5px;color:var(--muted)}.pack-update-search{display:flex;align-items:center;gap:8px;border:1px solid var(--line);border-radius:8px;padding:0 10px}.pack-update-search svg{width:15px;height:15px;flex:none}.pack-update-search input{min-width:0;width:100%;min-height:40px;background:transparent;color:var(--ink);font:inherit;border:0;padding:8px 0}.pack-update-search input::-webkit-search-decoration,.pack-update-search input::-webkit-search-cancel-button{appearance:none}.pack-update-files{list-style:none;padding:0;margin:12px 0}.pack-update-files li{border-bottom:1px solid var(--line);padding:12px 0;overflow-wrap:anywhere}.pack-update-path strong{font-size:12px;font-weight:500}.pack-update-badge{padding:3px 7px;border:1px solid var(--line);border-radius:6px;font-size:10px;color:var(--muted)}.pack-update-files p{margin:5px 0}.pack-update-files .setting-hint{font-size:11px}.pack-update-pager{justify-content:flex-end;font-size:11px;color:var(--muted)}.pack-update-context-error{display:flex;align-items:center;gap:12px}@media(max-width:600px){.pack-update-panel{padding:14px}.pack-update-versions{gap:16px}.pack-update-heading>div{gap:5px}}
.pack-update-choice,.pack-update-copy-form,.pack-update-added-tools{display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-top:12px}.pack-update-choice label,.pack-update-copy-form label{display:flex;align-items:center;gap:6px;font-size:12px}.pack-update-copy-form input[type=text],.pack-update-added input[type=search]{padding:8px;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--ink)}.pack-update-added{margin:16px 0;font-size:12px}.pack-update-added ul{list-style:none;padding:0;max-height:220px;overflow:auto;overflow-wrap:anywhere}.pack-update-added li{padding:6px 0;border-bottom:1px solid var(--line)}.pack-update-added li label{display:flex;align-items:center;gap:8px}.pack-update-copy-form{padding-top:14px;border-top:1px solid var(--line)}
.pack-update-replacement{display:flex;gap:8px;align-items:center;flex-wrap:wrap;font-size:12px;margin-top:8px}.pack-update-replacement select{padding:8px;border:1px solid var(--line);border-radius:6px;background:var(--panel);color:var(--ink);max-width:100%}
.pack-update-added input[type=search]{min-width:180px}
.pack-update-choice-option{font:inherit;font-size:12px;padding:7px 10px;border:1px solid var(--line);border-radius:7px;color:var(--muted);background:var(--panel)}.pack-update-choice-option[data-state=checked]{color:var(--ink);border-color:var(--accent);background:var(--tint)}.pack-update-choice-option:hover{color:var(--ink)}.pack-update-choice-option:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.pack-update-json{max-width:100%;overflow:auto;margin-top:12px}.pack-update-json table{border-collapse:collapse;width:100%;min-width:650px;font-size:11px}.pack-update-json th,.pack-update-json td{text-align:left;vertical-align:top;padding:8px;border:1px solid var(--line)}.pack-update-json thead th{font-weight:500;color:var(--muted)}.pack-update-json pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere;max-height:140px;overflow:auto;max-width:24ch;min-width:10ch;font:inherit}.pack-update-json .pack-update-choice{margin:0;gap:5px;min-width:130px}.pack-update-json code{font-size:11px}.pack-update-json .pack-update-badge{display:block;margin-top:5px;width:fit-content}
@media(max-width:700px){.pack-update-json table{min-width:0}.pack-update-json thead{position:absolute;width:1px;height:1px;clip-path:inset(50%);overflow:hidden}.pack-update-json tbody tr{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));margin-bottom:12px}.pack-update-json tbody th,.pack-update-json tbody td:last-child{grid-column:1/-1}.pack-update-json td[data-label]::before{content:attr(data-label);display:block;color:var(--muted);margin-bottom:6px}.pack-update-json pre{min-width:0;max-width:none}.pack-update-json th,.pack-update-json td{min-width:0}.pack-update-json .pack-update-badge{display:inline-block;margin:0 0 0 8px}}
</style>
