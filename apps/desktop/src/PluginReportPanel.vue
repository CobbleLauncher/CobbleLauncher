<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { pluginLocale } from './extensions/contributions';
import { computed, onUnmounted, ref, watch } from 'vue';
import type {Game} from './games';
import ManagedConfigReportPanel from './ManagedConfigReportPanel.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import ExtensionSettingsPanel from './ExtensionSettingsPanel.vue';
import AppIcon from './ui/AppIcon.vue';
import ReportResults from './ReportResults.vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import type { PluginReport, PluginFolderReport } from './plugin-reports';

const props = withDefaults(defineProps<{ pluginId: string; instanceId?: string; disabled?: boolean; games?:Game[]; manifest?: {api_version?:string;file_extensions?:string[];report_protocols?:string[];permissions?:{permission:string}[];contributions?:{settings?:unknown}} }>(),{games:()=>[],instanceId:undefined,manifest:undefined});
const optionsMounted = ref(false);
const optionsBusy = ref(false);
const optionsConfirming = ref(false);
const optionsPanel = ref<InstanceType<typeof ExtensionSettingsPanel> | null>(null);
const hasOptions = computed(() => !!props.manifest?.contributions?.settings);
const unsavedOptions = computed(() => !optionsBusy.value && !(optionsPanel.value?.canLeave() ?? true));
async function confirmLeave() {
  if (pending.value || managedBusy.value || optionsBusy.value) return false;
  return await optionsPanel.value?.confirmLeave() ?? true;
}
defineExpose({ confirmLeave, canLeave: () => !pending.value && !managedBusy.value && !optionsBusy.value && (optionsPanel.value?.canLeave() ?? true) });
const managedBusy=ref(false);const managedTarget=ref<string|undefined>(props.instanceId);
const canCheckFolder=computed(()=>props.manifest?.api_version==='0.2.0' && props.manifest.report_protocols?.includes('cobble.config-folder@1') && props.manifest.permissions?.some(node=>node.permission==='launcher:resource/selected-folder/read'));
const action=ref<'file'|'compare'|'folder'>('file');
let disposed=false;let revision=0;
const pending = ref(false);
const cancelling = ref(false);
const report = ref<PluginReport | null>(null);
const error = ref('');
const folderScope=ref<Pick<PluginFolderReport,'files'|'skipped'|'truncated'>|null>(null);

async function analyze(mode:'file'|'compare'|'folder'='file') {
  if(props.disabled||pending.value||managedBusy.value||optionsBusy.value||optionsConfirming.value||(mode==='folder'&&!canCheckFolder.value))return;
  const context = revision;
  optionsConfirming.value = true;
  let proceed: boolean;
  try { proceed = await confirmLeave(); } finally { optionsConfirming.value = false; }
  if(!proceed || context!==revision || props.disabled || optionsBusy.value || pending.value || managedBusy.value)return;
  const current=revision;action.value=mode;pending.value=true;error.value='';report.value=null;folderScope.value=null;
  try {
    const instanceId=canCheckFolder.value?managedTarget.value:props.instanceId;
    const args={pluginId:props.pluginId,locale:pluginLocale(),...(instanceId?{instanceId}:{}),...(mode==='compare'?{compare:true}:{})};
    if(mode==='folder'){
      const result=await invoke<PluginFolderReport|null>('analyze_plugin_folder_report',args);
      if(!disposed&&current===revision){report.value=result?.report??null;folderScope.value=result?{files:result.files,skipped:result.skipped,truncated:result.truncated}:null;}
    }else{
      const result=await invoke<PluginReport|null>('analyze_plugin_report',args);if(!disposed&&current===revision)report.value=result;
    }
  }
  catch(cause){if(!disposed&&current===revision)error.value=mode==='folder'?t('reports.folderFailed'):String(cause);}
  finally{if(!disposed){pending.value=false;cancelling.value=false;}}
}
async function cancel() {
  cancelling.value = true;
  try { await invoke('cancel_plugin_report'); }
  catch (cause) { if(!disposed){error.value = String(cause); cancelling.value = false;} }
}
watch(()=>[props.pluginId,props.instanceId,managedTarget.value],()=>{++revision;report.value=null;folderScope.value=null;error.value='';});
watch(()=>props.pluginId,()=>{optionsMounted.value=false;optionsBusy.value=false;});
onUnmounted(()=>{disposed=true;++revision;});
</script>

<template>
  <div
    class="plugin-report-panel"
    :aria-busy="pending || managedBusy || optionsBusy"
  >
    <SettingsDisclosure
      v-if="hasOptions"
      :key="pluginId"
      :title="t('inspector.options')"
      @update:open="value => { if(value) optionsMounted = true; }"
    >
      <fieldset
        v-if="optionsMounted"
        class="report-options"
        :disabled="disabled || pending || managedBusy || optionsConfirming"
      >
        <ExtensionSettingsPanel
          ref="optionsPanel"
          :key="pluginId"
          :plugin-id="pluginId"
          scope="global"
          @busy="optionsBusy = $event"
        />
      </fieldset>
    </SettingsDisclosure>
    <p
      v-if="canCheckFolder && unsavedOptions"
      class="setting-hint"
    >
      {{ t('inspector.resolveOptions') }}
    </p>
    <ManagedConfigReportPanel
      v-if="canCheckFolder"
      :plugin-id="pluginId"
      :games="games"
      :instance-id="instanceId"
      :extensions="manifest?.file_extensions??[]"
      :disabled="disabled||pending||optionsBusy||optionsConfirming||unsavedOptions"
      @busy="managedBusy=$event"
      @target="managedTarget=$event"
    />
    <component
      :is="canCheckFolder?SettingsDisclosure:'div'"
      :title="canCheckFolder?t('managedConfigReports.otherFiles'):undefined"
    >
      <div class="report-actions">
        <button
          class="secondary-button"
          :disabled="disabled || pending || managedBusy || optionsBusy || optionsConfirming"
          @click="analyze()"
        >
          <AppIcon
            v-if="pending && action==='file'"
            name="loader-circle"
            class="ui-busy-icon"
          />{{ pending && action==='file' ? t('reports.running') : t('reports.analyze') }}
        </button>
        <button
          v-if="canCheckFolder"
          class="text-button"
          :disabled="disabled || pending || managedBusy || optionsBusy || optionsConfirming"
          @click="analyze('folder')"
        >
          <AppIcon
            v-if="pending && action==='folder'"
            name="loader-circle"
            class="ui-busy-icon"
          />{{ t(pending && action==='folder'?'reports.checkingFolder':'reports.checkFolder') }}
        </button>
        <button
          v-if="pluginId === 'modpack-inspector'"
          class="text-button"
          :disabled="disabled || pending || managedBusy || optionsBusy || optionsConfirming"
          @click="analyze('compare')"
        >
          {{ t('reports.comparePacks') }}
        </button>
        <button
          v-if="pending"
          class="text-button"
          :disabled="cancelling"
          @click="cancel"
        >
          {{ t('reports.cancel') }}
        </button>
      </div>
      <p
        v-if="error"
        class="settings-error"
        role="alert"
      >
        {{ error }}
      </p>
      <p
        v-if="folderScope"
        class="folder-scope setting-hint"
        role="status"
      >
        {{ t('reports.folderSummary',{files:folderScope.files,skipped:folderScope.skipped}) }}<span v-if="folderScope.truncated"> · {{ t('reports.folderLimited') }}</span>
      </p>
      <ReportResults
        v-if="report"
        :report="report"
      />
    </component>
  </div>
</template>

<style scoped>
.plugin-report-panel{flex-basis:100%;min-width:0}.report-actions{display:flex;gap:var(--ui-space-3);align-items:center;flex-wrap:wrap}.report-actions button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.report-actions svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm)}
.folder-scope{font-size:var(--ui-text-caption);margin:var(--ui-space-3) 0}
.report-options{border:0;padding:0;margin:0;min-width:0}
</style>
