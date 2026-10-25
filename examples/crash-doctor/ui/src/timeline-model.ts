// SPDX-License-Identifier: MIT
import type {Report} from '../../../../packages/plugin-web-sdk/src/index';
export type StageEvent={stage:string;clock:string;elapsed:number|null;line:number;excerpt:string;segment:number|null};
export type LogGap={ms:number;first:number;last:number;from:string;to:string;segment:number};
export type WarningCluster={from:string;to:string;count:number;first:number;last:number;segment:number};
export type Timeline={metrics:Map<string,number|null>;events:StageEvent[];gaps:LogGap[];clusters:WarningCluster[]};
const number=(value:string|undefined):number|null=>{if(!value||!/^\d+$/.test(value))return null;const result=Number(value);return Number.isSafeInteger(result)?result:null;};
export function timelineFromReport(report:Report|null):Timeline|null {
  if(!report)return null;
  const table=(columns:string[])=>report.tables.find(table=>table.columns.join('\0')===columns.join('\0'))?.rows;
  const summary=table(['Timeline field','Value']);if(!summary)return null;
  const metrics=new Map(summary.map(row=>[row[0]!,number(row[1])]));
  const events=(table(['Stage','Clock','Elapsed ms','Line','Excerpt','Segment'])??[]).slice(0,96).filter(row=>number(row[3])!==null).map(row=>({stage:row[0]!,clock:row[1]??'',elapsed:number(row[2]),line:number(row[3])!,excerpt:row[4]??'',segment:number(row[5])}));
  const gaps=(table(['Gap ms','From line','To line','From clock','To clock','Segment'])??[]).slice(0,32).filter(row=>[0,1,2,5].every(index=>number(row[index])!==null)).map(row=>({ms:number(row[0])!,first:number(row[1])!,last:number(row[2])!,from:row[3]!,to:row[4]!,segment:number(row[5])!}));
  const clusters=(table(['First clock','Last clock','Warnings','First line','Last line','Segment'])??[]).slice(0,32).filter(row=>[2,3,4,5].every(index=>number(row[index])!==null)).map(row=>({from:row[0]!,to:row[1]!,count:number(row[2])!,first:number(row[3])!,last:number(row[4])!,segment:number(row[5])!}));
  return {metrics,events,gaps,clusters};
}
export const seconds=(ms:number|null)=>ms===null?'—':`${(ms/1000).toFixed(ms%1000?1:0)} s`;
/** Scan only the granted text. Return a bounded neighborhood rather than a giant split array. */
export function lineExcerpt(text:string,first:number,last=first):string {
  if(!Number.isSafeInteger(first)||first<1)return '';
  const startLine=Math.max(1,first-2),endLine=Math.min(last+2,first+6);let line=1,start=startLine===1?0:-1,end=text.length;
  for(let index=0;index<text.length;index++)if(text[index]==='\n'){line++;if(line===startLine)start=index+1;if(line>endLine){end=index;break;}}
  if(start<0||line<first)return '';
  return new TextDecoder().decode(new TextEncoder().encode(text.slice(start,end)).subarray(0,65536));
}
