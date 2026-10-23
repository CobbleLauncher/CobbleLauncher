<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed,onUnmounted,ref,watch} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {t} from './i18n';
import {pluginLocale} from './extensions/contributions';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import type {GameScreenshot} from './screenshots';
import {MAX_SHEET_IMAGES,type ScreenshotSheetPlugin,type ScreenshotSheetPreview,type ScreenshotSheetOrdering} from './screenshot-sheet';
const props=withDefaults(defineProps<{gameId:string;screenshots:GameScreenshot[];active?:boolean}>(),{active:true});
const emit=defineEmits<{busy:[value:boolean];extensions:[]}>();
const plugins=ref<ScreenshotSheetPlugin[]>([]);const pluginId=ref('');const loading=ref(false);const loadFailed=ref(false);
const columns=ref(0);const ordering=ref<ScreenshotSheetOrdering>('selected');const preview=ref<ScreenshotSheetPreview|null>(null);
const phase=ref<'preview'|'save'|''>('');const pending=computed(()=>phase.value!=='');const error=ref('');const outcome=ref('');const cancelling=ref(false);
let revision=0;let disposed=false;let ownedRequest:string|null=null;let cancelAccepted=false;
watch(pending,value=>emit('busy',value),{flush:'sync'});
function invalidate(){++revision;preview.value=null;error.value='';outcome.value='';const id=ownedRequest;ownedRequest=null;if(id)void invoke('cancel_screenshot_sheet',{requestId:id}).catch(()=>{});}
async function loadPlugins(){
 if(!props.active)return;const current=revision;loading.value=true;loadFailed.value=false;
 try{const result=await invoke<ScreenshotSheetPlugin[]>('list_screenshot_sheet_plugins');if(!disposed&&current===revision){plugins.value=result;if(!result.some(plugin=>plugin.plugin_id===pluginId.value))pluginId.value=result[0]?.plugin_id??'';}}
 catch{if(!disposed&&current===revision)loadFailed.value=true;}
 finally{if(!disposed&&current===revision)loading.value=false;}
}
function message(cause:unknown,save=false){const value=String(cause);if(value.includes('SCREENSHOT_CHANGED')||value.includes('INVALID_SELECTION'))return t('screenshotSheet.selectionChanged');if(value.includes('PLUGIN_CHANGED')||value.includes('PLUGIN_UNAVAILABLE'))return t('screenshotSheet.pluginUnavailable');if(value.includes('SHEET_EXPIRED'))return t('screenshotSheet.expired');if(value.includes('OUTPUT_IS_SOURCE'))return t('screenshotSheet.outputIsSource');return t(save?'screenshotSheet.saveFailed':'screenshotSheet.previewFailed');}
async function generate(){
 if(pending.value||!props.active||!pluginId.value||props.screenshots.length<1||props.screenshots.length>MAX_SHEET_IMAGES)return;
 invalidate();const current=revision;const requestId=crypto.randomUUID();ownedRequest=requestId;cancelAccepted=false;cancelling.value=false;phase.value='preview';
 try{const result=await invoke<ScreenshotSheetPreview|null>('preview_screenshot_sheet',{gameId:props.gameId,screenshotIds:props.screenshots.map(item=>item.id),pluginId:pluginId.value,requestId,columns:columns.value,ordering:ordering.value,locale:pluginLocale()});if(!disposed&&current===revision){if(cancelAccepted||!result){outcome.value=t('screenshotSheet.cancelled');void invoke('cancel_screenshot_sheet',{requestId}).catch(()=>{});}else preview.value=result;}}
 catch(cause){if(!disposed&&current===revision){if(cancelAccepted)outcome.value=t('screenshotSheet.cancelled');else error.value=message(cause);}}
 finally{if(!disposed){phase.value='';cancelling.value=false;}}
}
async function cancel(){if(phase.value!=='preview'||!ownedRequest||cancelling.value)return;const current=revision;const requestId=ownedRequest;cancelling.value=true;try{await invoke('cancel_screenshot_sheet',{requestId});if(!disposed&&current===revision)cancelAccepted=true;}catch{if(!disposed&&current===revision){error.value=t('screenshotSheet.cancelFailed');cancelling.value=false;}}}
async function save(){
 if(!preview.value||pending.value)return;const current=revision;const previewId=preview.value.preview_id;phase.value='save';error.value='';outcome.value='';
 try{const saved=await invoke<boolean>('save_screenshot_sheet',{previewId});if(!disposed&&current===revision)outcome.value=t(saved?'screenshotSheet.saved':'screenshotSheet.saveCancelled');}
 catch(cause){if(!disposed&&current===revision)error.value=message(cause,true);}
 finally{if(!disposed)phase.value='';}
}
watch(()=>[props.gameId,props.screenshots.map(item=>item.id).join('\u0000'),props.active] as const,()=>{invalidate();loading.value=false;if(props.active)void loadPlugins();},{immediate:true});
watch([pluginId,columns,ordering],()=>invalidate());
onUnmounted(()=>{disposed=true;invalidate();if(pending.value)emit('busy',false);});
</script>
<template>
  <section
    class="screenshot-sheet"
    :aria-busy="pending||loading"
  >
    <LoadingIndicator
      v-if="loading"
      :label="t('screenshotSheet.loadingPlugins')"
      compact
    />
    <template v-else-if="loadFailed">
      <p
        class="settings-error"
        role="alert"
      >
        {{ t('screenshotSheet.pluginsFailed') }}
      </p><button
        class="text-button"
        @click="loadPlugins"
      >
        {{ t('screenshots.retry') }}
      </button>
    </template>
    <template v-else-if="!plugins.length">
      <p class="setting-hint">
        {{ t('screenshotSheet.noPlugin') }}
      </p><button
        class="text-button"
        @click="emit('extensions')"
      >
        {{ t('navigation.extensions') }}
      </button>
    </template>
    <template v-else>
      <div class="sheet-controls">
        <label v-if="plugins.length>1">{{ t('screenshotSheet.layout') }}<select
          v-model="pluginId"
          :disabled="pending"
        ><option
          v-for="plugin in plugins"
          :key="plugin.plugin_id"
          :value="plugin.plugin_id"
        >{{ plugin.name }}</option></select></label><label>{{ t('screenshotSheet.columns') }}<select
          v-model="columns"
          :disabled="pending"
        ><option :value="0">{{ t('screenshotSheet.automatic') }}</option><option
          v-for="number in [1,2,3,4]"
          :key="number"
          :value="number"
        >{{ number }}</option></select></label><label>{{ t('screenshotSheet.ordering') }}<select
          v-model="ordering"
          :disabled="pending"
        ><option value="selected">{{ t('screenshotSheet.orderSelected') }}</option><option value="oldest">{{ t('screenshotSheet.orderOldest') }}</option><option value="newest">{{ t('screenshotSheet.orderNewest') }}</option></select></label>
      </div>
      <div class="sheet-actions">
        <button
          class="secondary-button"
          :disabled="pending||!pluginId"
          @click="generate"
        >
          <AppIcon
            v-if="phase==='preview'"
            name="loader-circle"
            class="ui-busy-icon"
          />{{ t('screenshotSheet.preview') }}
        </button><button
          v-if="phase==='preview'"
          class="text-button"
          :disabled="cancelling"
          @click="cancel"
        >
          {{ t('screenshotSheet.cancel') }}
        </button><button
          v-if="preview"
          class="secondary-button"
          :disabled="pending"
          @click="save"
        >
          <AppIcon
            v-if="phase==='save'"
            name="loader-circle"
            class="ui-busy-icon"
          /><AppIcon
            v-else
            name="download"
          />{{ t('screenshotSheet.saveAs') }}
        </button>
      </div>
      <LoadingIndicator
        v-if="phase==='preview'"
        :label="t('screenshotSheet.creating')"
        compact
      />
      <LoadingIndicator
        v-if="phase==='save'"
        :label="t('screenshotSheet.saving')"
        compact
      />
    </template>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p><p
      v-if="outcome"
      class="setting-hint"
      role="status"
    >
      {{ outcome }}
    </p>
    <img
      v-if="preview"
      class="sheet-preview"
      :src="preview.data_url"
      :width="preview.width"
      :height="preview.height"
      :alt="t('screenshotSheet.previewAlt')"
    >
  </section>
</template>
<style scoped>
.screenshot-sheet{min-width:0;margin-top:var(--ui-space-4)}.sheet-controls{display:flex;align-items:end;gap:var(--ui-space-3);flex-wrap:wrap}.sheet-controls label{display:grid;gap:var(--ui-space-2);min-width:110px;font-size:var(--ui-text-label);flex:1}.sheet-controls select{width:100%;min-width:0;min-height:var(--ui-control-height);padding:8px 10px;border:1px solid var(--line);border-radius:var(--ui-radius-md);background:var(--bg);color:var(--ink);font:inherit;font-size:var(--ui-text-label)}.sheet-actions{display:flex;align-items:center;gap:var(--ui-space-2);flex-wrap:wrap;margin:var(--ui-space-4) 0}.sheet-actions button{display:inline-flex;align-items:center;gap:var(--ui-space-2)}.sheet-actions svg{width:var(--ui-icon-sm);height:var(--ui-icon-sm)}.sheet-preview{display:block;max-width:100%;width:auto;height:auto;max-height:560px;margin-top:var(--ui-space-4);border:1px solid var(--line);border-radius:var(--ui-radius-md);object-fit:contain}.screenshot-sheet .setting-hint{font-size:var(--ui-text-caption)}.screenshot-sheet .settings-error{font-size:var(--ui-text-caption)}.sheet-preview{animation:sheet-enter .14s ease-out}@keyframes sheet-enter{from{opacity:0}to{opacity:1}}@media(prefers-reduced-motion:reduce){.sheet-preview{animation:none}}@media(max-width:600px){.sheet-controls{gap:9px}.sheet-controls label{min-width:100px}.sheet-preview{max-height:400px}}
</style>
