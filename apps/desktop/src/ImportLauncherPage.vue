<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import CompatibilityNotice from './CompatibilityNotice.vue';
import SwitchField from './ui/SwitchField.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import TransferWorldScene from './ui/TransferWorldScene.vue';
import GameImportResult from './GameImportResult.vue';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui';
import { defaultDownloadSettings, readDownloadSettings, validateDownloadSettings } from './download-settings';
import { loaderLabel, type Game, type LauncherImportPreview } from './games';
import { taskPhaseLabel, taskFailureMessage, type TaskSnapshot } from './tasks';
import { importPhaseRatio, importSceneState } from './launcher-import-progress';
import { usePageWorkspace } from './use-page-workspace';
import { t } from './i18n';
const props = withDefaults(defineProps<{ games?:Game[];tasks?:TaskSnapshot[] }>(),{games:()=>[],tasks:()=>[]});
const emit = defineEmits<{ started:[game:Game];open:[game:Game];busy:[value:boolean] }>();
// Only a selected host game reference and display labels survive navigation.
const workspace=usePageWorkspace('launcher-import',undefined,()=>({gameId:'',sourceName:'',targetName:''}));
const step=ref<'source'|'select'|'confirm'|'progress'>(workspace.gameId?'progress':'source');
const preview=ref<LauncherImportPreview|null>(null);
const candidateId=ref('');const contentId=ref('');
const candidate=computed(()=>preview.value?.candidates.find(item=>item.candidate_id===candidateId.value));
const contentRoot=computed(()=>candidate.value?.content_roots.find(item=>item.content_id===contentId.value));
const name=ref('');const includeSaves=ref(false);const moveSource=ref(false);
const source=ref(defaultDownloadSettings());const directory=ref('');const useDefaultDirectory=ref(true);
const loading=ref(true);const picking=ref<'source'|'destination'|null>(null);const submitting=ref(false);
const busy=computed(()=>picking.value!==null||submitting.value);
const error=ref('');let disposed=false;
const initialGame=ref<Game|null>(null);
const returnedTask=ref<TaskSnapshot|null>(null);
const importedGame=computed(()=>props.games.find(game=>game.id===workspace.gameId)??initialGame.value);
const task=computed(()=>[...props.tasks,...(returnedTask.value?[returnedTask.value]:[])].filter(task=>task.game_id===workspace.gameId&&task.kind==='game_install').sort((a,b)=>b.updated_at-a.updated_at)[0]);
const progress=computed(()=>task.value?.progress??importedGame.value?.progress??null);
const state=computed(()=>importSceneState(importedGame.value,task.value));
const ratio=computed(()=>importPhaseRatio(progress.value));
const copiedRatio=ref<number|null>(null);
// Installation phases reset their counters. Only real copy progress drives blocks.
watch(progress,value=>{if(value?.phase==='import-copy')copiedRatio.value=importPhaseRatio(value);},{immediate:true});
const sceneProgress=computed(()=>state.value==='complete'?1:copiedRatio.value);
const phaseLabel=computed(()=>state.value==='complete'?t('launcherImport.complete'):state.value==='failed'?t('launcherImport.failed'):state.value==='cancelled'?t('launcherImport.cancelled'):task.value?.state==='cancelling'?t('tasks.states.cancelling'):progress.value?taskPhaseLabel(progress.value):t('launcherImport.waiting'));
const progressError=computed(()=>importedGame.value?.error||(task.value?.error?taskFailureMessage(task.value):''));
const canRetry=computed(()=>task.value?.can_retry??importedGame.value?.status==='failed');
const rootLabel=(root:{content_id:string;label:string})=>root.content_id==='isolated'?t('launcherImport.isolatedRoot'):root.content_id==='shared'?t('launcherImport.sharedRoot'):root.label;
const sourcePath=computed(()=>contentRoot.value?.source_display??candidate.value?.source_display??preview.value?.source_display??'');
const sourceSummary=computed(()=>source.value.source==='automatic'?t('launcherImport.automaticRecommended'):source.value.source==='official'?t('launcherImport.official'):source.value.source==='bmclapi'?'BMCLAPI':t('launcherImport.customMirror'));
const steps=['sourceStep','gameStep','confirmStep','transferStep'] as const;
const stepIndex=computed(()=>step.value==='source'?0:step.value==='select'?1:step.value==='confirm'?2:3);
watch(busy,value=>emit('busy',value),{flush:'sync'});
watch(candidateId,()=>{name.value=candidate.value?.name??'';contentId.value=candidate.value?.content_roots.length===1?candidate.value.content_roots[0]!.content_id:'';includeSaves.value=false;moveSource.value=false;error.value='';},{flush:'sync'});
watch(contentId,()=>{includeSaves.value=false;moveSource.value=false;},{flush:'sync'});
async function initialize(){
 try{const settings=await readDownloadSettings();const target=isTauri()?await invoke<string>('default_game_directory'):t('launcherImport.defaultGameDirectory');if(!disposed){source.value=settings;directory.value=target;}}
 catch(cause){if(!disposed)error.value=t('launcherImport.readLocationError',{cause:String(cause)});}finally{loading.value=false;}
}
async function chooseSource(discover=false){
 if(busy.value||loading.value)return;
 if(!isTauri()){error.value=t('launcherImport.desktopOnly');return;}
 picking.value='source';error.value='';
 try{const selected=await invoke<LauncherImportPreview|null>(discover?'discover_launcher_import':'preview_launcher_import');if(disposed||!selected)return;
 candidateId.value='';preview.value=selected;contentId.value='';name.value='';includeSaves.value=false;moveSource.value=false;
 if(selected.candidates.length===1)candidateId.value=selected.candidates[0]!.candidate_id;
 step.value='select';
 }catch(cause){if(!disposed)error.value=t('launcherImport.readSourceError',{cause:String(cause)});}finally{picking.value=null;}
}
async function chooseDirectory(){
 if(busy.value||loading.value)return;picking.value='destination';error.value='';
 try{const selected=await invoke<string|null>('pick_game_directory');if(!disposed&&selected){directory.value=selected;useDefaultDirectory.value=false;}}
 catch(cause){if(!disposed)error.value=t('launcherImport.chooseFolderError',{cause:String(cause)});}finally{picking.value=null;}
}
async function create(){
 if(busy.value||loading.value||!preview.value||!candidate.value?.supported||!contentRoot.value)return;
 const downloadError=validateDownloadSettings(source.value);error.value=downloadError&&source.value.source!=='custom'?t('launcherImport.invalidDownloadSource'):downloadError?t('launcherImport.invalidMirror'):'';if(error.value)return;
 const trimmed=name.value.trim();if(!trimmed){error.value=t('launcherImport.nameRequired');return;}if(trimmed.length>80){error.value=t('launcherImport.nameTooLong');return;}
 submitting.value=true;
 try{const game=await invoke<Game>('create_launcher_import',{previewId:preview.value.preview_id,candidateId:candidate.value.candidate_id,contentId:contentRoot.value.content_id,name:trimmed,includeSaves:contentRoot.value.has_saves&&includeSaves.value,moveSource:contentRoot.value.content_id==='isolated'&&moveSource.value,source:{...source.value,mirror:source.value.mirror.trim()},useDefaultDirectory:useDefaultDirectory.value});
 if(disposed)return;initialGame.value=game;workspace.gameId=game.id;workspace.sourceName=candidate.value.name;workspace.targetName=trimmed;copiedRatio.value=null;step.value='progress';emit('started',game);
 }catch(cause){if(!disposed)error.value=t('launcherImport.importError',{cause:String(cause)});}finally{submitting.value=false;}
}
async function retry(){
 if(busy.value||!importedGame.value||!canRetry.value)return;submitting.value=true;error.value='';
 try{const game=await invoke<Game>('retry_game',{id:importedGame.value.id});if(!disposed){initialGame.value=game;emit('started',game);}}
 catch(cause){if(!disposed)error.value=t('launcherImport.importError',{cause:String(cause)});}finally{submitting.value=false;}
}
async function cancelImport(){
 if(busy.value||!task.value||!['running','queued'].includes(task.value.state))return;
 submitting.value=true;error.value='';
 try{const updated=await invoke<TaskSnapshot>('cancel_task',{taskId:task.value.id});if(!disposed)returnedTask.value=updated;}
 catch(cause){if(!disposed)error.value=t('launcherImport.importError',{cause:String(cause)});}finally{submitting.value=false;}
}
function startAnother(){workspace.gameId='';workspace.sourceName='';workspace.targetName='';initialGame.value=null;preview.value=null;candidateId.value='';contentId.value='';error.value='';step.value='source';copiedRatio.value=null;}
onMounted(()=>{void initialize();});onUnmounted(()=>{disposed=true;emit('busy',false);});
</script>
<template>
  <div class="import-launcher-page page-content">
    <ol
      class="import-steps"
      :aria-label="t('launcherImport.steps')"
    >
      <li
        v-for="(label,index) in steps"
        :key="label"
        :aria-current="index===stepIndex?'step':undefined"
        :class="{'is-done':index<stepIndex}"
      >
        <span>{{ index+1 }}</span>{{ t('launcherImport.'+label) }}
      </li>
    </ol>
    <section
      v-if="step==='source'"
      class="import-source-step"
    >
      <TransferWorldScene
        :progress="null"
        state="waiting"
        :source-label="t('launcherImport.otherLauncher')"
        target-label="Cobble"
      />
      <h2>{{ t('launcherImport.sourceStep') }}</h2>
      <div class="import-source-actions">
        <button
          type="button"
          class="primary-button launcher-import-discover"
          :disabled="loading||busy"
          @click="chooseSource(true)"
        >
          <LoadingIndicator
            v-if="busy"
            compact
            :label="t('launcherImport.reading')"
          /><AppIcon
            v-else
            name="search"
          />{{ t('launcherImport.findInstalled') }}
        </button><button
          type="button"
          class="text-button pack-picker"
          :disabled="loading||busy"
          @click="chooseSource()"
        >
          <AppIcon name="folder" />{{ t('launcherImport.chooseSource') }}
        </button>
      </div>
      <p class="import-source-formats">
        {{ t('launcherImport.sourceHint') }}
      </p>
    </section>
    <section
      v-else-if="step==='select'"
      class="import-selection-step"
    >
      <h2>{{ t('launcherImport.sourceGame') }}</h2><p class="import-path">
        {{ preview?.source_display }}
      </p>
      <p
        v-if="!preview?.candidates.length"
        class="profile-notice"
      >
        {{ t('launcherImport.noCandidates') }}
      </p>
      <RadioGroupRoot
        v-if="preview?.candidates.length"
        v-model="candidateId"
        :disabled="busy"
        :aria-label="t('launcherImport.sourceGame')"
        class="import-candidates"
      >
        <RadioGroupItem
          v-for="item in preview.candidates"
          :key="item.candidate_id"
          :value="item.candidate_id"
          class="import-candidate"
        >
          <AppIcon :name="item.loader_kind?'puzzle':'monitor'" /><span><strong>{{ item.name }}</strong><span class="import-candidate-meta">{{ item.minecraft_version ? 'Minecraft '+item.minecraft_version : t('launcherImport.unsupported') }}<template v-if="item.loader_kind"> · {{ loaderLabel(item.loader_kind) }} {{ item.loader_version }}</template></span></span><AppIcon
            v-if="candidateId===item.candidate_id"
            name="check"
          />
        </RadioGroupItem>
      </RadioGroupRoot>
      <template v-if="candidate">
        <CompatibilityNotice
          :minecraft-version="candidate.minecraft_version??''"
          :loader-kind="candidate.loader_kind"
          :loader-version="candidate.loader_version"
        />
        <p
          v-if="!candidate.supported"
          class="settings-error"
          role="status"
        >
          {{ candidate.unsupported_reason||t('launcherImport.unsupportedImport') }}
        </p>
        <ul
          v-if="candidate.warnings.length"
          class="launcher-import-warnings"
        >
          <li
            v-for="warning in candidate.warnings"
            :key="warning"
          >
            {{ warning }}
          </li>
        </ul>
        <div
          v-if="candidate.supported&&candidate.content_roots.length>1"
          class="import-content-roots"
        >
          <h3>{{ t('launcherImport.gameDataDirectory') }}</h3><RadioGroupRoot
            v-model="contentId"
            :aria-label="t('launcherImport.gameDataDirectory')"
            :disabled="busy"
            class="import-root-options"
          >
            <RadioGroupItem
              v-for="root in candidate.content_roots"
              :key="root.content_id"
              :value="root.content_id"
              class="import-root-option"
            >
              <AppIcon name="folder" />{{ rootLabel(root) }}<AppIcon
                v-if="contentId===root.content_id"
                name="check"
              />
            </RadioGroupItem>
          </RadioGroupRoot>
        </div>
        <p
          v-if="candidate.supported&&!candidate.content_roots.length"
          class="profile-notice"
        >
          {{ t('launcherImport.noDataDirectory') }}
        </p>
        <p
          v-if="contentRoot"
          class="import-path import-selected-root"
        >
          {{ rootLabel(contentRoot) }}<span>{{ sourcePath }}</span>
        </p>
      </template>
      <footer class="import-step-actions">
        <button
          type="button"
          class="secondary-button"
          :disabled="busy"
          @click="step='source'"
        >
          <AppIcon name="arrow-left" />{{ t('navigation.back') }}
        </button><button
          type="button"
          class="primary-button"
          :disabled="busy||!candidate?.supported||!contentRoot"
          @click="step='confirm'"
        >
          {{ t('launcherImport.continue') }}
        </button>
      </footer>
    </section>
    <form
      v-else-if="step==='confirm'"
      class="import-confirm-step"
      @submit.prevent="create"
    >
      <h2>{{ t('launcherImport.confirmStep') }}</h2>
      <TransferWorldScene
        class="import-confirm-scene"
        :progress="null"
        state="waiting"
        :source-label="t('launcherImport.confirmSource')"
        :target-label="t('launcherImport.confirmTarget')"
      />
      <div class="import-confirm-fields">
        <div class="form-field">
          <label for="launcher-import-name">{{ t('launcherImport.gameName') }}</label><input
            id="launcher-import-name"
            v-model="name"
            maxlength="80"
            autocomplete="off"
            :disabled="busy"
          >
        </div>
        <div class="form-field">
          <label>{{ t('launcherImport.installLocation') }}</label><button
            type="button"
            class="folder-button"
            :title="directory"
            :disabled="busy"
            @click="chooseDirectory"
          >
            <AppIcon name="folder" /><span>{{ directory||t('launcherImport.defaultGameDirectory') }}</span><AppIcon name="pencil" />
          </button>
        </div>
        <RadioGroupRoot
          v-if="contentRoot?.content_id==='isolated'"
          :model-value="moveSource?'move':'copy'"
          :aria-label="t('launcherImport.transferMode')"
          :disabled="busy"
          class="import-mode-options"
          @update:model-value="moveSource=$event==='move'"
        >
          <RadioGroupItem value="copy">
            {{ t('launcherImport.copyMode') }}
          </RadioGroupItem><RadioGroupItem value="move">
            {{ t('launcherImport.moveMode') }}
          </RadioGroupItem>
        </RadioGroupRoot>
        <p
          v-if="contentRoot?.content_id!=='isolated'"
          class="import-copy-mode"
        >
          <AppIcon name="copy" />{{ t('launcherImport.copyMode') }}
        </p>
        <p
          class="import-consequence"
          :class="{'is-move':moveSource}"
        >
          {{ t(moveSource?'launcherImport.moveNotice':'launcherImport.copyNotice') }}
        </p>
        <label
          v-if="contentRoot?.has_saves"
          class="import-save-option"
        ><span>{{ t('launcherImport.includeSaves') }}</span><SwitchField
          v-model="includeSaves"
          :label="t('launcherImport.includeSaves')"
          :disabled="busy"
        /></label>
        <SettingsDisclosure
          icon="download"
          :title="t('launcherImport.downloadSource')"
          :value="sourceSummary"
        >
          <RadioGroupRoot
            v-model="source.source"
            class="source-options"
            :disabled="busy"
            :aria-label="t('launcherImport.downloadSource')"
          >
            <RadioGroupItem
              value="automatic"
              class="source-option"
            >
              {{ t('launcherImport.automaticRecommended') }}
            </RadioGroupItem><RadioGroupItem
              value="official"
              class="source-option"
            >
              {{ t('launcherImport.official') }}
            </RadioGroupItem><RadioGroupItem
              value="bmclapi"
              class="source-option"
            >
              BMCLAPI
            </RadioGroupItem><RadioGroupItem
              v-if="source.mirror"
              value="custom"
              class="source-option"
            >
              {{ t('launcherImport.customMirror') }}
            </RadioGroupItem>
          </RadioGroupRoot><input
            v-if="source.source==='custom'"
            id="launcher-import-mirror"
            v-model="source.mirror"
            readonly
            type="url"
            :aria-label="t('launcherImport.mirrorAddress')"
            :disabled="busy"
          >
        </SettingsDisclosure>
      </div>
      <footer class="import-step-actions">
        <button
          type="button"
          class="secondary-button"
          :disabled="busy"
          @click="step='select'"
        >
          <AppIcon name="arrow-left" />{{ t('navigation.back') }}
        </button><button
          class="primary-button done-button"
          type="submit"
          :disabled="loading||busy||!candidate?.supported||!contentRoot"
        >
          <LoadingIndicator
            v-if="submitting"
            compact
            :label="t('launcherImport.creating')"
          />{{ t(moveSource?'launcherImport.moveImportButton':'launcherImport.importButton') }}
        </button>
      </footer>
    </form>
    <section
      v-else
      class="import-progress-step"
      aria-live="polite"
    >
      <h2>{{ phaseLabel }}</h2><TransferWorldScene
        :progress="sceneProgress"
        :state="state"
        :source-label="t('launcherImport.sourceWorld',{name:workspace.sourceName})"
        :target-label="t('launcherImport.targetWorld',{name:workspace.targetName})"
      />
      <div
        v-if="progress&&state!=='complete'"
        class="import-transfer-progress"
      >
        <div class="import-progress-caption">
          <span v-if="ratio!==null">{{ progress.completed }} / {{ progress.total }}</span>
        </div><div
          v-if="ratio!==null"
          class="import-progress-track"
          role="progressbar"
          :aria-label="taskPhaseLabel(progress)"
          :aria-valuenow="progress.completed"
          :aria-valuemin="0"
          :aria-valuemax="progress.total"
        >
          <span :style="{width:ratio*100+'%'}" />
        </div>
      </div>
      <p
        v-if="progressError"
        class="settings-error"
        role="alert"
      >
        {{ progressError }}
      </p>
      <GameImportResult
        v-if="importedGame"
        :game-id="importedGame.id"
        :status="importedGame.status"
      />
      <footer class="import-step-actions">
        <button
          v-if="state==='working'||state==='waiting'"
          type="button"
          class="secondary-button"
          :disabled="busy||!task||task.state==='cancelling'"
          @click="cancelImport"
        >
          <AppIcon name="x" />{{ t(task?.state==='cancelling'?'tasks.cancelling':'tasks.cancel') }}
        </button>
        <button
          v-if="state==='complete'||state==='failed'||state==='cancelled'"
          type="button"
          class="text-button"
          :disabled="busy"
          @click="startAnother"
        >
          {{ t('launcherImport.importAnother') }}
        </button><button
          v-if="importedGame?.status==='installed'"
          type="button"
          class="primary-button"
          @click="emit('open',importedGame)"
        >
          {{ t('launcherImport.openGame') }}
        </button><button
          v-else-if="(state==='failed'||state==='cancelled')&&canRetry"
          type="button"
          class="primary-button"
          :disabled="busy"
          @click="retry"
        >
          <AppIcon name="refresh-cw" />{{ t('tasks.retryTask') }}
        </button>
      </footer>
    </section>
    <p
      v-if="error"
      class="settings-error pack-error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
<style scoped>
.import-launcher-page{max-width:none}.import-selection-step{max-width:960px;margin-inline:auto}.import-steps{display:flex;gap:var(--ui-space-5);list-style:none;margin:0 0 var(--ui-space-5);padding:0;overflow-x:auto}.import-steps li{display:flex;align-items:center;gap:var(--ui-space-2);white-space:nowrap;color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}.import-steps li>span{display:grid;place-items:center;width:24px;height:24px;flex:none;border:1px solid var(--ui-border);border-radius:50%;font-variant-numeric:tabular-nums}.import-steps li[aria-current=step]{color:var(--ui-text);font-weight:600}.import-steps li[aria-current=step]>span{background:var(--tint);border-color:var(--ui-focus)}.import-steps li.is-done>span{color:var(--ui-focus)}.import-launcher-page h2{font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:600;margin:0 0 var(--ui-space-3)}.import-source-step{text-align:center;max-width:680px;margin-inline:auto}.import-source-actions{display:flex;justify-content:center;gap:var(--ui-space-3);flex-wrap:wrap}.import-source-formats{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);margin-top:var(--ui-space-3)}.import-path{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);overflow-wrap:anywhere;margin:var(--ui-space-2) 0 var(--ui-space-4)}.import-candidates{display:grid;border-top:1px solid var(--ui-border)}.import-candidate{display:flex;align-items:center;gap:var(--ui-space-3);width:100%;min-width:0;padding:var(--ui-space-4) var(--ui-space-3);border:0;border-bottom:1px solid var(--ui-border);background:transparent;color:var(--ui-text);font:inherit;text-align:left;cursor:pointer}.import-candidate>span{flex:1;min-width:0}.import-candidate strong{display:block;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);font-weight:600;overflow-wrap:anywhere}.import-candidate>.app-icon:first-child{width:24px;height:24px;color:var(--ui-text-muted)}.import-candidate-meta{display:block;font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);margin-top:var(--ui-space-1)}.import-candidate[data-state=checked],.import-root-option[data-state=checked]{background:var(--tint)}.import-candidate:focus-visible,.import-root-option:focus-visible{outline:var(--ui-focus-ring);outline-offset:-2px}.import-content-roots{margin-top:var(--ui-space-5)}.import-content-roots h3{font-size:var(--ui-text-label);line-height:var(--ui-leading-label);margin-bottom:var(--ui-space-2)}.import-root-options{display:flex;gap:var(--ui-space-2);flex-wrap:wrap}.import-root-option{display:flex;align-items:center;gap:var(--ui-space-2);min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;font-size:var(--ui-text-label)}.import-selected-root{margin-top:var(--ui-space-3)}.import-selected-root>span{display:block}.import-confirm-step{max-width:960px;margin-inline:auto;display:grid;grid-template-columns:minmax(220px,.8fr) minmax(0,1.2fr);gap:var(--ui-space-3) var(--ui-space-5)}.import-confirm-step>h2,.import-confirm-step>.import-step-actions{grid-column:1/-1}.import-confirm-fields{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--ui-space-3);min-width:0}.import-confirm-fields>*:not(.form-field){grid-column:1/-1}.import-confirm-fields>.form-field{min-width:0}.import-confirm-fields>.settings-disclosure{margin-block:0}.import-confirm-step>.import-step-actions{margin-top:var(--ui-space-2);padding-top:var(--ui-space-3);border-top:1px solid var(--ui-border)}.import-confirm-step h2{margin:0}.import-confirm-scene{max-width:360px;justify-self:center;align-self:start;width:100%}.import-confirm-step .form-field>label{font-size:var(--ui-text-label);line-height:var(--ui-leading-label);font-weight:600}.import-confirm-step input{min-height:var(--ui-control-height);font-size:var(--ui-text-body)}.import-confirm-step .folder-button{min-height:var(--ui-control-height);padding-inline:var(--ui-space-3);font-size:var(--ui-text-label)}.import-mode-options{display:flex;gap:var(--ui-space-2)}.import-mode-options>button{flex:1;min-height:var(--ui-control-height);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:transparent;color:var(--ui-text);font:inherit;font-size:var(--ui-text-label)}.import-mode-options>button[data-state=checked]{border-color:var(--ui-focus);background:var(--tint);font-weight:600}.import-consequence.is-move{color:var(--ui-warning)}.import-confirm-step .folder-button>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.import-consequence{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);margin:0}.import-copy-mode{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-label);margin:0}.import-save-option{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);font-size:var(--ui-text-label)}.import-step-actions{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);margin-top:var(--ui-space-5)}.import-step-actions>.primary-button{margin-left:auto}.import-step-actions>.done-button{width:auto;flex:0 1 auto;min-width:180px;margin-top:0}.import-source-actions>.pack-picker{display:inline-flex;align-items:center;justify-content:center;gap:var(--ui-space-2);width:auto;min-height:var(--ui-control-height);height:auto;margin:0;padding:0 var(--ui-space-3);border:0;background:transparent;font-size:var(--ui-text-label)}.import-source-actions>.pack-picker :deep(svg){width:20px;height:20px}.import-progress-step{max-width:680px;margin-inline:auto}.import-progress-step>h2{text-align:center}.import-progress-caption{display:flex;align-items:center;justify-content:flex-end;gap:var(--ui-space-3);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);color:var(--ui-text-muted);font-variant-numeric:tabular-nums}.import-progress-track{height:6px;background:var(--ui-surface-3);border-radius:var(--ui-radius-sm);overflow:hidden;margin-top:var(--ui-space-2)}.import-progress-track>span{display:block;height:100%;background:var(--ui-focus)}.import-progress-step>.settings-error{margin-top:var(--ui-space-4)}.launcher-import-warnings{font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);padding-left:var(--ui-space-4);color:var(--ui-warning)}@media(max-width:600px){.import-steps{gap:var(--ui-space-3)}.import-confirm-step{grid-template-columns:minmax(0,1fr)}.import-confirm-scene{max-width:330px}.import-confirm-scene :deep(.transfer-world-art){min-height:80px;max-height:80px}.import-step-actions{margin-top:var(--ui-space-4)}}
</style>
