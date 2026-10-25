<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed,onUnmounted,ref,watch} from 'vue';
import {DialogRoot,DialogPortal,DialogOverlay,DialogContent,DialogTitle,DialogDescription} from 'reka-ui';
import {TriangleAlert,CircleHelp,LoaderCircle} from '@lucide/vue';
import {locale} from '../i18n';
defineOptions({inheritAttrs:false});
const props=withDefaults(defineProps<{open:boolean;title:string;description?:string;confirmLabel:string;cancelLabel?:string;danger?:boolean;requireDelay?:boolean;busy?:boolean;disabled?:boolean}>(),{description:'',cancelLabel:'',danger:false,requireDelay:false,busy:false,disabled:false});
const emit=defineEmits<{'update:open':[value:boolean];confirm:[];cancel:[]}>();
const remaining=ref(0),submitted=ref(false),cancelButton=ref<HTMLButtonElement|null>(null);
let timer:ReturnType<typeof setInterval>|undefined,opener:HTMLElement|null=null;
const zh=computed(()=>locale.value==='zh-CN');
const cancelText=computed(()=>props.cancelLabel||(zh.value?'取消':'Cancel'));
const confirmText=computed(()=>remaining.value?`${props.confirmLabel} (${remaining.value})`:props.confirmLabel);
const countdownText=computed(()=>zh.value?`${remaining.value} 秒后可确认`:`Confirm in ${remaining.value} seconds`);
function stop(){if(timer!==undefined){clearInterval(timer);timer=undefined;}}
function reset(){stop();if(!props.open)return;submitted.value=false;remaining.value=props.requireDelay?3:0;if(!remaining.value)return;const deadline=Date.now()+3000;timer=setInterval(()=>{remaining.value=Math.max(0,Math.ceil((deadline-Date.now())/1000));if(!remaining.value)stop();},100);}
watch(()=>props.open,open=>{if(open&&typeof document!=='undefined')opener=document.activeElement instanceof HTMLElement?document.activeElement:null;reset();},{immediate:true,flush:'sync'});
watch(()=>[props.requireDelay,props.title,props.description],reset,{flush:'sync'});
function cancel(){if(props.busy||submitted.value)return;submitted.value=true;stop();emit('update:open',false);emit('cancel');}
function confirm(){if(!props.open||remaining.value||props.busy||props.disabled||submitted.value)return;submitted.value=true;stop();emit('confirm');emit('update:open',false);}
function restoreFocus(event:Event){event.preventDefault();if(opener?.isConnected)opener.focus();}
onUnmounted(stop);
</script>
<template>
  <DialogRoot
    :open="open"
    @update:open="value=>{if(!value)cancel();}"
  >
    <DialogPortal>
      <DialogOverlay
        class="confirmation-overlay"
        :class="{'confirmation-overlay-danger':danger}"
      />
      <DialogContent
        v-bind="$attrs"
        class="confirmation-dialog"
        :class="{'confirmation-danger':danger}"
        :role="danger?'alertdialog':'dialog'"
        @open-auto-focus.prevent="cancelButton?.focus()"
        @close-auto-focus="restoreFocus"
        @pointer-down-outside.prevent
        @escape-key-down="event=>{event.preventDefault();cancel();}"
      >
        <div class="confirmation-heading">
          <span class="confirmation-icon"><TriangleAlert
            v-if="danger"
            :size="27"
            aria-hidden="true"
          /><CircleHelp
            v-else
            :size="25"
            aria-hidden="true"
          /></span><div>
            <DialogTitle class="confirmation-title">
              {{ title }}
            </DialogTitle>
          </div>
        </div>
        <DialogDescription
          v-if="description"
          class="confirmation-description"
        >
          {{ description }}
        </DialogDescription>
        <DialogDescription
          v-else
          class="confirmation-sr-only"
        >
          {{ title }}
        </DialogDescription>
        <div
          v-if="$slots.default"
          class="confirmation-context"
        >
          <slot />
        </div>
        <div class="confirmation-actions">
          <button
            ref="cancelButton"
            type="button"
            class="secondary-button confirmation-cancel"
            :disabled="busy||submitted"
            @click="cancel"
          >
            {{ cancelText }}
          </button><button
            type="button"
            class="primary-button confirmation-confirm"
            :class="{'confirmation-destructive':danger}"
            :disabled="remaining>0||busy||disabled||submitted"
            @click="confirm"
          >
            <LoaderCircle
              v-if="busy"
              :size="17"
              class="ui-busy-icon"
              aria-hidden="true"
            />{{ confirmText }}
          </button>
        </div>
        <span
          v-if="remaining"
          class="confirmation-sr-only"
          role="status"
          aria-live="polite"
        >{{ countdownText }}</span>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.confirmation-overlay[data-state=open]{animation:confirmation-overlay-enter var(--ui-duration-normal,160ms) ease-out both}
.confirmation-overlay[data-state=closed]{animation:confirmation-overlay-exit var(--ui-duration-fast,120ms) ease-out both}
.confirmation-dialog[data-state=open]{animation:confirmation-dialog-enter var(--ui-duration-normal,160ms) ease-out both}
.confirmation-dialog[data-state=closed]{animation:confirmation-dialog-exit var(--ui-duration-fast,120ms) ease-out both}
@keyframes confirmation-overlay-enter{from{opacity:0}to{opacity:1}}
@keyframes confirmation-overlay-exit{from{opacity:1}to{opacity:0}}
@keyframes confirmation-dialog-enter{from{opacity:0;transform:translate(-50%,calc(-50% + 6px))}to{opacity:1;transform:translate(-50%,-50%)}}
@keyframes confirmation-dialog-exit{from{opacity:1;transform:translate(-50%,-50%)}to{opacity:0;transform:translate(-50%,calc(-50% + 4px))}}
@media(prefers-reduced-motion:reduce){.confirmation-dialog[data-state],.confirmation-overlay[data-state]{animation:none}}
.confirmation-sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}.confirmation-overlay{position:fixed;inset:0;z-index:1200;background:#0008}.confirmation-overlay-danger{background:#09090bc9;backdrop-filter:blur(5px)}.confirmation-dialog{position:fixed;z-index:1201;top:50%;left:50%;transform:translate(-50%,-50%);box-sizing:border-box;width:min(480px,calc(100vw - 32px));max-height:calc(100dvh - 40px);overflow:auto;padding:28px;background:var(--ui-surface-1);color:var(--ui-text);border:1px solid var(--ui-border);border-radius:var(--ui-radius-lg,12px);box-shadow:0 24px 90px #0005;font-size:var(--ui-text-body,14px);line-height:var(--ui-leading-body,1.6)}.confirmation-danger{width:min(560px,calc(100vw - 32px));border-top:4px solid var(--ui-danger)}.confirmation-heading{display:flex;gap:16px;align-items:center}.confirmation-icon{flex:none;display:grid;place-items:center;width:52px;height:52px;border-radius:50%;background:var(--ui-surface-2);color:var(--ui-text-muted)}.confirmation-danger .confirmation-icon{color:var(--ui-danger);background:color-mix(in srgb,var(--ui-danger) 12%,var(--ui-surface-1))}.confirmation-title{font-size:20px;line-height:1.35;font-weight:650;margin:0;overflow-wrap:anywhere}.confirmation-warning{color:var(--ui-danger);font-size:12px;font-weight:600;margin:0 0 4px}.confirmation-description{margin:20px 0 0;color:var(--ui-text-muted);font-size:14px;line-height:1.65;white-space:pre-line;overflow-wrap:anywhere}.confirmation-context{margin-top:20px;padding:14px 16px;background:var(--ui-surface-2);border-left:2px solid var(--ui-border);font-size:13px;max-height:35dvh;overflow:auto;overflow-wrap:anywhere}.confirmation-context :deep(p){margin:4px 0}.confirmation-context :deep(ul){margin:0;padding-left:18px}.confirmation-actions{display:flex;justify-content:flex-end;align-items:center;gap:12px;margin-top:28px}.confirmation-actions button{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:42px;font-size:14px}.confirmation-actions .confirmation-destructive{background:var(--ui-danger);border:1px solid var(--ui-danger);color:var(--ui-surface-1)}.confirmation-actions .confirmation-destructive:disabled{opacity:.5}.confirmation-actions .confirmation-destructive:not(:disabled):hover{background:color-mix(in srgb,var(--ui-danger) 85%,#000);border-color:color-mix(in srgb,var(--ui-danger) 85%,#000)}@media(max-width:560px){.confirmation-dialog{padding:22px}.confirmation-heading{gap:12px}.confirmation-icon{width:44px;height:44px}.confirmation-title{font-size:18px}.confirmation-actions{display:grid;grid-template-columns:1fr 1fr}.confirmation-actions button{white-space:normal}.confirmation-description{font-size:14px}}@media(prefers-reduced-motion:reduce){.confirmation-dialog,.confirmation-overlay,.confirmation-actions button{animation:none;transition:none}.confirmation-overlay-danger{backdrop-filter:none}}
</style>
