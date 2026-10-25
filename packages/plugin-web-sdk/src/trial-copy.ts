// SPDX-License-Identifier: MIT
import type {Transport} from './index';
export type TrialCopyInput={gameId:string;name:string;includeWorlds?:boolean};
export type TrialCopyPreview={planId:string;game:{id:string;name:string;version:string};name:string;includeWorlds:boolean;fileCount:number;totalBytes:number;requiredBytes:number;availableBytes:number|null;enoughSpace:boolean|null;destinationLabel:string};
export type TrialCopyTask={taskId:string;status:'running'|'cancelling'|'succeeded'|'failed'|'cancelled';completedBytes:number;totalBytes:number;error:string|null;game:{id:string;name:string;version:string}|null};
function id(value:string){if(typeof value!=='string'||!/^[a-zA-Z0-9_-]{1,128}$/.test(value))throw Error('TRIAL_COPY_REQUEST_INVALID');return value}
function input(value:TrialCopyInput){if(typeof value.name!=='string'||!value.name.trim()||[...value.name].length>80||/[\u0000-\u001f\u007f]/u.test(value.name)||value.includeWorlds!==undefined&&typeof value.includeWorlds!=='boolean')throw Error('TRIAL_COPY_REQUEST_INVALID');return {gameId:id(value.gameId),name:value.name.trim(),includeWorlds:value.includeWorlds??false}}
export function createTrialCopy(transport:Transport){return Object.freeze({
 preview:(args:TrialCopyInput)=>transport({method:'trialCopy.preview',params:input(args)}) as Promise<TrialCopyPreview>,
 pickDestination:(args:TrialCopyInput)=>transport({method:'trialCopy.pickDestination',params:input(args)}) as Promise<TrialCopyPreview|null>,
 start:(args:{planId:string})=>transport({method:'trialCopy.start',params:{planId:id(args.planId)}}) as Promise<{taskId:string}>,
 task:(args:{taskId:string})=>transport({method:'trialCopy.task',params:{taskId:id(args.taskId)}}) as Promise<TrialCopyTask>,
 cancel:(args:{taskId?:string}={})=>transport({method:'trialCopy.cancel',params:args.taskId===undefined?{}:{taskId:id(args.taskId)}}) as Promise<null>,
 openGame:(args:{taskId:string})=>transport({method:'trialCopy.openGame',params:{taskId:id(args.taskId)}}) as Promise<{requested:true}>,
})}
