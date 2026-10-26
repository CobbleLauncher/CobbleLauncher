// SPDX-License-Identifier: MIT
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { meshProfileLabel, type MeshAppSnapshot } from '../mesh-social';
import { isActiveTask, type TaskSnapshot } from '../tasks';
import { transferText as text } from './labels';
import { emptyTransferSelection, type OutgoingTransfer, type TransferContext, type TransferOffer, type TransferPeer, type TransferPreview, type TransferRecord } from './types';

export type GameTransferEntry = {gameId?:string;initialTaskId?:string;initialOfferId?:string;incomingManifestId?:string};
export function useGameTransfer(props:GameTransferEntry,onTask:(id:string)=>void,onDiscard:(id:string)=>void){
  const context=ref<TransferContext|null>(null),peers=ref<TransferPeer[]>([]),offers=ref<TransferOffer[]>([]),preview=ref<TransferPreview|null>(null),offer=ref<TransferOffer|null>(null),outgoing=ref<OutgoingTransfer|null>(null),task=ref<TaskSnapshot|null>(null);
  const stage=ref<'target'|'content'|'review'|'inbox'|'receive'|'transfer'>(props.initialTaskId||props.initialOfferId?'transfer':props.gameId?'target':'inbox'),selection=ref(emptyTransferSelection()),peerId=ref(''),name=ref(''),busy=ref(false),error=ref(''),meshAvailable=ref(false),loaded=ref(false);
  let mounted=true,poll:ReturnType<typeof setTimeout>|undefined,unlisten:(()=>void)|undefined,polling=false,taskId=props.initialTaskId,draftOfferId:string|undefined;
  const sendMode=computed(()=>!!props.gameId||!!props.initialOfferId||!!outgoing.value);
  const selectedPeer=computed(()=>peers.value.find(peer=>peer.peerId===peerId.value));
  const active=computed(()=>outgoing.value?['waiting','transferring','prepared'].includes(outgoing.value.state):!!task.value&&isActiveTask(task.value));
  const record=computed<TransferRecord|null>(()=>{
    if(outgoing.value){const item=outgoing.value;return {taskId:item.offerId,direction:'send',state:({prepared:'waiting',waiting:'waiting',transferring:'running',completed:'succeeded',expired:'interrupted',cancelled:'cancelled',rejected:'failed'} as const)[item.state],bytesDone:item.bytesDone,bytesTotal:item.bytesTotal,error:item.error,gameName:item.gameName};}
    if(!task.value)return null;const item=task.value,bytes=item.progress?.phase==='receiving';return {taskId:item.id,direction:'receive',state:item.state,bytesDone:bytes?item.progress!.completed:0,bytesTotal:bytes?item.progress!.total:null,phase:item.progress?.phase,error:item.error,gameId:item.game_id,gameName:item.game_name};
  });
  function failure(cause:unknown){
    const code=String(cause);
    return text(/BUSY|RUNNING|SOURCE_IN_USE/u.test(code)?'busyGame':/IDENTITY|CONTACT|PEER_UNKNOWN/u.test(code)?'identity':/OFFLINE|NOT_CONNECTED/u.test(code)?'offline':/SPACE|NO_SPACE/u.test(code)?'space':/CHANGED|MISMATCH/u.test(code)?'changed':/EXPIRED|UNKNOWN|NOT_FOUND/u.test(code)?'expired':/DISABLED|UNAVAILABLE|NOT_READY/u.test(code)?'unavailable':'failedGeneric');
  }
  async function run(action:()=>Promise<void>){if(busy.value)return;busy.value=true;error.value='';try{await action();}catch(cause){if(mounted)error.value=failure(cause);}finally{if(mounted)busy.value=false;}}
  function updateTask(item:TaskSnapshot){if(!mounted||item.id!==taskId||task.value&&item.updated_at<task.value.updated_at)return;task.value=item;stage.value='transfer';}
  function schedule(){clearTimeout(poll);if(mounted&&active.value)poll=setTimeout(()=>void refreshProgress(),1000);}
  async function refreshProgress(){
    if(polling||!mounted)return;polling=true;
    try{
      if(outgoing.value){const id=outgoing.value.offerId,value=await invoke<OutgoingTransfer>('game_transfer_status',{offerId:id});if(mounted&&outgoing.value?.offerId===id){outgoing.value={...outgoing.value,...value};error.value='';}}
      else if(taskId){const rows=await invoke<TaskSnapshot[]>('list_tasks');const item=rows.find(row=>row.id===taskId);if(item)updateTask(item);}
    }catch(cause){if(mounted){error.value=failure(cause);if(outgoing.value&&/EXPIRED|NOT_FOUND/u.test(String(cause)))outgoing.value={...outgoing.value,state:'expired'};}}finally{polling=false;schedule();}
  }
  async function refresh(){await run(async()=>{
    if(!isTauri()){loaded.value=true;error.value=text('unavailable');return;}
    if(taskId){await refreshProgress();if(!task.value)error.value=text('noTask');return;}
    const snapshot=await invoke<MeshAppSnapshot|null>('mesh_app_snapshot');if(!mounted)return;
    meshAvailable.value=!!snapshot;
    peers.value=(snapshot?.contacts??[]).map(item=>({peerId:item.profile.peer_id,displayName:meshProfileLabel(item.profile),connected:item.connected}));
    if(props.gameId){const value=await invoke<TransferContext>('game_transfer_context',{gameId:props.gameId});if(!mounted)return;context.value=value;meshAvailable.value=meshAvailable.value&&value.meshAvailable;for(const scope of value.availableScopes)selection.value[scope]=true;}
    if(props.gameId||props.initialOfferId){const rows=await invoke<OutgoingTransfer[]>('game_transfer_list_outgoing');if(!mounted)return;const found=rows.find(item=>props.initialOfferId?item.offerId===props.initialOfferId:(item.gameId===props.gameId&&['waiting','transferring'].includes(item.state)));if(found){outgoing.value=found;peerId.value=found.peerId??'';stage.value='transfer';schedule();}else if(props.initialOfferId)error.value=text('expired');}
    else{offers.value=await invoke<TransferOffer[]>('game_transfer_offers');if(!mounted)return;if(props.incomingManifestId){const found=offers.value.find(item=>item.manifestId===props.incomingManifestId);if(found)selectOffer(found);}}
    loaded.value=true;
  });if(mounted)loaded.value=true;}
  function selectOffer(value:TransferOffer){offer.value=value;name.value=value.gameName??`Minecraft ${value.version}`;stage.value='receive';error.value='';}
  async function prepare(){await run(async()=>{
    if(!props.gameId||!Object.values(selection.value).some(Boolean))return;
    if(draftOfferId){await invoke('game_transfer_cancel',{offerId:draftOfferId});draftOfferId=undefined;preview.value=null;}
    const value=await invoke<TransferPreview>('game_transfer_prepare',{gameId:props.gameId,selection:{...selection.value}});
    if(!mounted){await invoke('game_transfer_cancel',{offerId:value.offerId});return;}draftOfferId=value.offerId;preview.value=value;stage.value='review';
  });}
  async function send(){await run(async()=>{
    if(!preview.value||!peerId.value)return;
    const prepared=preview.value;const value=await invoke<{manifestId:string;totalBytes:number;totalFiles:number}>('game_transfer_send',{offerId:prepared.offerId,peerId:peerId.value});
    draftOfferId=undefined;if(!mounted)return;
    outgoing.value={offerId:prepared.offerId,state:'waiting',bytesDone:0,bytesTotal:value.totalBytes,gameName:prepared.gameName,peerId:peerId.value};stage.value='transfer';schedule();
  });}
  async function accept(){await run(async()=>{
    if(!offer.value||!name.value.trim()){error.value=text('invalidName');return;}
    const value=await invoke<{taskId:string;gameId:string}>('game_transfer_accept',{peerId:offer.value.peerId,manifestId:offer.value.manifestId,name:name.value.trim(),useDefaultDirectory:true});
    if(!mounted)return;taskId=value.taskId;onTask(taskId);stage.value='transfer';await refreshProgress();
  });}
  async function reject(){await run(async()=>{if(!offer.value)return;await invoke('game_transfer_reject',{peerId:offer.value.peerId,manifestId:offer.value.manifestId});if(mounted){offers.value=offers.value.filter(item=>item!==offer.value);offer.value=null;stage.value='inbox';}});}
  async function cancel(){await run(async()=>{
    if(outgoing.value){await invoke('game_transfer_cancel',{offerId:outgoing.value.offerId});if(mounted)outgoing.value={...outgoing.value,state:'cancelled'};}
    else if(taskId)updateTask(await invoke<TaskSnapshot>('cancel_task',{taskId}));
  });}
  async function discard(){
    let discarded=false;
    await run(async()=>{
      const item=task.value;
      if(!item?.game_id||!['failed','cancelled','interrupted'].includes(item.state))return;
      await invoke('game_transfer_discard',{gameId:item.game_id});
      if(!mounted)return;
      onDiscard(item.game_id);onTask(item.id);taskId=undefined;task.value=null;offer.value=null;stage.value='inbox';discarded=true;
    });
    if(discarded&&mounted)await refresh();
  }
  onMounted(async()=>{
    if(isTauri()){try{const stop=await listen<TaskSnapshot>('task-updated',event=>updateTask(event.payload));if(!mounted){stop();return;}unlisten=stop;}catch{/* Polling still reads the actual task history. */}}
    if(mounted)await refresh();
  });
  onUnmounted(()=>{mounted=false;clearTimeout(poll);unlisten?.();if(draftOfferId)void invoke('game_transfer_cancel',{offerId:draftOfferId}).catch(()=>{});});
  return {context,peers,offers,preview,offer,outgoing,task,stage,selection,peerId,name,busy,error,meshAvailable,loaded,sendMode,selectedPeer,active,record,refresh,selectOffer,prepare,send,accept,reject,cancel,discard,failure};
}
