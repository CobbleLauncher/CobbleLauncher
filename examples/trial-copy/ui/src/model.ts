// SPDX-License-Identifier: MIT
import type {TrialCopyPreview,TrialCopyTask} from '../../../../packages/plugin-web-sdk/src/trial-copy';
export function validName(value:string){return !!value.trim()&&[...value].length<=80&&![...value].some(c=>c.charCodeAt(0)<32||c.charCodeAt(0)===127);}
export function canCreate(preview:TrialCopyPreview|null,busy:boolean){return !!preview&&!busy&&preview.enoughSpace!==false;}
export function progress(task:TrialCopyTask){return task.totalBytes>0?Math.max(0,Math.min(1,task.completedBytes/task.totalBytes)):null;}
export function bytes(value:number){if(!Number.isFinite(value)||value<0)return '—';const units=['B','KiB','MiB','GiB','TiB'];let unit=0;while(value>=1024&&unit<units.length-1){value/=1024;unit++;}return `${value.toLocaleString('en',{maximumFractionDigits:unit?1:0})} ${units[unit]}`;}
