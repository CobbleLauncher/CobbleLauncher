<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed,watch} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {DialogContent,DialogOverlay,DialogPortal,DialogRoot,DialogTitle} from 'reka-ui';
import {locale} from './i18n';
import AppIcon from './ui/AppIcon.vue';
import LoadingIndicator from './ui/LoadingIndicator.vue';
import ReportResults from './ReportResults.vue';
import {createFileInboxHostReview} from './use-file-inbox-host-review';
import type {FileInboxHostRequest,FileInboxResult} from './file-inbox-host';
const props=defineProps<{request:FileInboxHostRequest|null}>();
const emit=defineEmits<{result:[value:FileInboxResult];finished:[];busy:[value:boolean]}>();
const state=createFileInboxHostReview({invoke,result:value=>emit('result',value),finished:()=>emit('finished')});
const{preview,report,error,phase}=state;
const zh=computed(()=>locale.value==='zh-CN');const tr=(en:string,cn:string)=>zh.value?cn:en;
const applying=computed(()=>phase.value==='apply');
const title=computed(()=>report.value?tr('Report','分析报告'):preview.value?preview.value.handler.kind==='import'?preview.value.handler.label:tr(`Open with ${preview.value.handler.label}`,`使用 ${preview.value.handler.label} 打开`):tr('Review file','查看文件'));
const failure=computed(()=>/EXPIRED/.test(error.value)?tr('This request expired. Review the file again in Inbox.','此请求已过期，请在投递页重新查看文件。'):/GAME_BUSY|GAME_REQUIRED|GAME_SCOPE/.test(error.value)?tr('Close the game, then review this file again.','请先关闭游戏，再重新查看文件。'):/CANCELLED/.test(error.value)?tr('Cancelled.','已取消。'):/SOURCE_CHANGED/.test(error.value)?tr('The selected file changed. Choose it again.','所选文件已更改，请重新选择。'):tr('Could not open this file. Review it again in Inbox.','无法打开此文件，请在投递页重新查看。'));
const canConfirm=computed(()=>phase.value==='ready'&&!!preview.value&&(!preview.value.handler.requiresGame||!!preview.value.game?.available));
watch(()=>props.request,request=>{if(request)void state.open(request);},{immediate:true});
watch(phase,value=>emit('busy',value==='preview'||value==='apply'),{immediate:true});
function close(open:boolean){if(!open)void state.dismiss();}
defineExpose({confirmLeave:async()=>applying.value?false:state.dismiss()});
</script>
<template>
  <DialogRoot
    :open="!!request"
    @update:open="close"
  >
    <DialogPortal>
      <DialogOverlay class="inbox-review-overlay" />
      <DialogContent
        class="inbox-review-dialog"
        :class="{'inbox-report-dialog':!!report}"
        :aria-describedby="undefined"
        @escape-key-down="applying && $event.preventDefault()"
        @interact-outside="applying && $event.preventDefault()"
      >
        <DialogTitle>{{ title }}</DialogTitle>
        <p
          v-if="preview"
          class="inbox-review-name"
        >
          {{ preview.name }}
        </p>
        <LoadingIndicator
          v-if="phase==='preview'"
          :label="tr('Loading…','正在加载…')"
        />
        <template v-if="preview && !report">
          <p
            v-if="preview.game"
            class="inbox-review-game"
          >
            <AppIcon name="folder" />{{ preview.game.name }} <span>{{ preview.game.version }}</span>
          </p>
          <p
            v-if="preview.warning==='executable_mod'"
            class="inbox-review-warning"
          >
            {{ tr('Mods can run code when the game starts.','Mod 可在游戏启动时执行代码。') }}
          </p>
          <p
            v-if="preview.game && !preview.game.available"
            role="alert"
            class="settings-error"
          >
            {{ tr('Close this game before importing.','请先关闭游戏，再导入文件。') }}
          </p>
        </template>
        <p
          v-if="error"
          role="alert"
          class="settings-error"
        >
          {{ failure }}
        </p>
        <p
          v-if="applying"
          role="status"
        >
          {{ tr('Opening…','正在打开…') }}
        </p>
        <div
          v-if="report"
          class="inbox-report-body"
        >
          <ReportResults :report="report.report" />
        </div>
        <div class="inbox-review-actions">
          <button
            class="secondary-button"
            :disabled="applying"
            @click="state.dismiss()"
          >
            {{ report||phase==='error'?tr('Close','关闭'):tr('Cancel','取消') }}
          </button>
          <button
            v-if="phase==='ready'||applying"
            class="primary-button"
            :disabled="!canConfirm"
            @click="state.confirm()"
          >
            {{ applying?tr('Opening…','正在打开…'):preview?.handler.id.startsWith('builtin:')&&preview.handler.requiresGame?tr('Import','导入'):tr('Continue','继续') }}
          </button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
<style scoped>
.inbox-review-overlay{position:fixed;inset:0;background:#0007;z-index:110}.inbox-review-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(480px,calc(100vw - 32px));max-height:calc(100vh - 48px);overflow:auto;box-sizing:border-box;padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--panel);color:var(--ink);z-index:111}.inbox-review-dialog h2{font-size:18px;margin:0 0 14px}.inbox-review-name{font-size:15px;font-weight:600;overflow-wrap:anywhere}.inbox-review-dialog p{line-height:1.6}.inbox-review-game{display:flex;align-items:center;gap:8px;flex-wrap:wrap;font-size:14px}.inbox-review-game svg{width:16px;height:16px}.inbox-review-game span{color:var(--muted)}.inbox-review-warning{font-size:13px;color:var(--muted)}.inbox-review-actions{display:flex;justify-content:flex-end;gap:10px;margin-top:22px;flex-wrap:wrap}.inbox-report-dialog{width:min(900px,calc(100vw - 32px))}.inbox-report-body{min-width:0;max-height:60vh;overflow:auto}
</style>

