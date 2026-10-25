// SPDX-License-Identifier: MIT
import {ref} from 'vue';
import type {FileInboxHostPreview,FileInboxHostRequest,FileInboxResult} from './file-inbox-host';
type Invoke=<T>(command:string,args:Record<string,unknown>)=>Promise<T>;
export function createFileInboxHostReview(deps:{invoke:Invoke;result:(value:FileInboxResult)=>void;finished:()=>void}){
 const preview=ref<FileInboxHostPreview|null>(null),report=ref<Extract<FileInboxResult,{kind:'report'}>|null>(null),error=ref(''),phase=ref<'idle'|'preview'|'ready'|'apply'|'report'|'error'>('idle');let version=0,active:string|null=null,consumed=false;
 async function open(request:FileInboxHostRequest){if(phase.value==='apply')return;const current=++version;active=request.handoff_id;consumed=false;preview.value=null;report.value=null;error.value='';phase.value='preview';try{const value=await deps.invoke<FileInboxHostPreview>('file_inbox_preview_handoff',{handoffId:request.handoff_id});if(version!==current)return;preview.value=value;phase.value='ready';}catch(cause){if(version!==current)return;error.value=String(cause);phase.value='error';}}
 async function dismiss(){if(phase.value==='apply')return false;const id=active;if(!id)return true;active=null;version++;if(!consumed){try{await deps.invoke('file_inbox_apply_handoff',{handoffId:id,confirmed:false});}catch{/* Cancellation consumes only this proposal. The selected resource stays in Inbox. */}}phase.value='idle';deps.finished();return true;}
 async function confirm(){if(phase.value!=='ready'||!active||!preview.value||preview.value.handler.requiresGame&&(!preview.value.game||!preview.value.game.available))return;const current=version,id=active;phase.value='apply';consumed=true;error.value='';try{const value=await deps.invoke<FileInboxResult>('file_inbox_apply_handoff',{handoffId:id,confirmed:true});if(current!==version)return;if(value.kind==='report'){report.value=value;phase.value='report';deps.result(value);}else{phase.value='idle';active=null;deps.result(value);deps.finished();}}catch(cause){if(current!==version)return;error.value=String(cause);phase.value='error';}}
 return{preview,report,error,phase,open,dismiss,confirm};
}
