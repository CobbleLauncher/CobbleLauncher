// SPDX-License-Identifier: MIT
export type ChatMessage = {role:'user'|'assistant';content:string};
const bytes=(value:string)=>new TextEncoder().encode(value).length;
export function prepareMessages(history:ChatMessage[], prompt:string):{messages:ChatMessage[];omitted:number}{
  const content=prompt.trim();
  if(!content||bytes(content)>16384)throw new Error('AI_MESSAGE_INVALID');
  const messages:ChatMessage[]=[{role:'user',content}];
  let total=bytes(content),omitted=0;
  for(let index=history.length-1;index>=0;index--){
    const item=history[index]!;
    const size=bytes(item.content);
    if(size>16384||messages.length>=20||total+size>49152){omitted=index+1;break;}
    messages.unshift(item);total+=size;
  }
  return {messages,omitted};
}
export function validEndpoint(value:string):boolean{
  try{const url=new URL(value);return !url.username&&!url.password&&!url.hash&&!url.search&&url.pathname!=='/'&&
    (url.protocol==='https:'||(url.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(url.hostname)));}
  catch{return false;}
}
export function localEndpoint(value:string):boolean{
  try{const url=new URL(value);return url.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(url.hostname);}
  catch{return false;}
}
export function requestId():string{return crypto.randomUUID().replaceAll('-','');}
