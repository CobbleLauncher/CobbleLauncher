// SPDX-License-Identifier: MIT
import type {Report} from '../../../../packages/plugin-web-sdk/src/index';
export type Counter={id:string;value:bigint};
export function counters(report:Report,category:string):Counter[]{const table=report.tables.find(t=>t.columns.join(',')==='category,id,value');return(table?.rows??[]).flatMap(r=>{if(r.length!==3||r[0]!==category||!/^[0-9]{1,20}$/.test(r[2]!))return[];const value=BigInt(r[2]!);return value<=18446744073709551615n?[{id:r[1]!,value}]:[];});}
export function time(ticks:bigint,zh:boolean){const minutes=ticks/1200n;return zh?`${minutes/60n} 小时 ${minutes%60n} 分钟`:`${minutes/60n}h ${minutes%60n}m`;}
export function distance(cm:bigint,zh:boolean){const km=cm/100000n,tenths=(cm%100000n)/10000n;return `${km}.${tenths} ${zh?'公里':'km'}`;}
export function percent(value:bigint,max:bigint){return max===0n?0:Number(value*1000n/max)/10;}
export function label(id:string){return id.startsWith('minecraft:')?id.slice(10).replaceAll('_',' '):id;}
