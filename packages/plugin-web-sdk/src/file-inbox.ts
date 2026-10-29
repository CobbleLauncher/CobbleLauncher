// SPDX-License-Identifier: MIT
import type {Transport} from './index';
export type FileInboxKind='mrpack'|'curseforge-pack'|'mod'|'resource-pack'|'shader-pack'|'blueprint'|'log'|'ambiguous'|'unknown';
export type FileInboxHandler={id:string;label:string;kind:'import'|'viewer'|'report';requiresGame:boolean};
export type FileInboxGame={id:string;name:string;version:string;compatibility:'declared-match'|'unknown'|'mismatch';reason:string|null;available:boolean};
export type FileInboxItem={resourceId:string;name:string;bytes:number;kind:FileInboxKind;evidence:string[];handlers:FileInboxHandler[];games:FileInboxGame[];expiresAt:number;declaredMinecraft:string|null;declaredLoader:string|null;error:string|null};
export type FileInboxOpen={resourceId:string;handlerId:string;gameId?:string};
export type FileInboxLink={provider:'modrinth'|'curseforge';kind:'mod'|'modpack'|'resource_pack'|'shader_pack';project:string;version?:string};
export type FileInboxOpenLink={url:string;gameId?:string};
/** Recognizes project pages only. A recognized link is never an artifact URL. */
export function parseFileInboxLink(value:string):FileInboxLink{
 if(value.length>2048||value.trim()!==value)throw Error('FILE_INBOX_LINK_UNSUPPORTED');
 let url:URL;try{url=new URL(value);}catch{throw Error('FILE_INBOX_LINK_UNSUPPORTED');}
 if(url.protocol!=='https:'||url.username||url.password||url.port||url.search||url.hash)throw Error('FILE_INBOX_LINK_UNSUPPORTED');
 const parts=url.pathname.replace(/^\/+|\/+$/g,'').split('/');let provider:FileInboxLink['provider'],kind:FileInboxLink['kind']|undefined,project:string,version:string|undefined;
 if(['modrinth.com','www.modrinth.com'].includes(url.hostname)&&(parts.length===2||parts.length===4&&parts[2]==='version')){
  const kinds:Record<string,FileInboxLink['kind']>={mod:'mod',modpack:'modpack',resourcepack:'resource_pack',shader:'shader_pack'};
  provider='modrinth';kind=kinds[parts[0]!];project=parts[1]!;version=parts.length===4?parts[3]:undefined;
 }else if(['curseforge.com','www.curseforge.com'].includes(url.hostname)&&parts[0]==='minecraft'&&(parts.length===3||parts.length===5&&parts[3]==='files')){
  const kinds:Record<string,FileInboxLink['kind']>={'mc-mods':'mod',modpacks:'modpack'};
  provider='curseforge';kind=kinds[parts[1]!];project=parts[2]!;version=parts.length===5?parts[4]:undefined;
 }else throw Error('FILE_INBOX_LINK_UNSUPPORTED');
 const valid=(s:string)=>!!s&&s.length<=128&&/^[a-zA-Z0-9._-]+$/.test(s);
 if(!kind||!valid(project)||version!==undefined&&!valid(version)||provider==='curseforge'&&version!==undefined&&!/^\d+$/.test(version))throw Error('FILE_INBOX_LINK_UNSUPPORTED');
 return{provider,kind,project,...(version===undefined?{}:{version})};
}
function token(value:string){if(!/^[a-f0-9]{32}$/.test(value))throw Error('FILE_INBOX_RESOURCE_INVALID');return value;}
function open(args:FileInboxOpen){token(args.resourceId);if(!/^[a-z0-9][a-z0-9:._-]{0,159}$/.test(args.handlerId))throw Error('FILE_INBOX_HANDLER_INVALID');if(args.gameId!==undefined&&(!args.gameId||args.gameId.length>128||/[\u0000-\u001f\u007f]/u.test(args.gameId)))throw Error('PLUGIN_GAME_SCOPE');return{resourceId:args.resourceId,handlerId:args.handlerId,...(args.gameId===undefined?{}:{gameId:args.gameId})};}
/** User-selected host resources. No path or file bytes cross this interface. */
export function createFileInbox(transport:Transport){return Object.freeze({
 pick:()=>transport({method:'fileInbox.pick',params:{}})as Promise<FileInboxItem[]>,
 pending:()=>transport({method:'fileInbox.pending',params:{}})as Promise<FileInboxItem[]>,
 open:(args:FileInboxOpen)=>transport({method:'fileInbox.open',params:open(args)})as Promise<{requested:true}>,
 openLink:(args:FileInboxOpenLink)=>{parseFileInboxLink(args.url);if(args.gameId!==undefined&&(!args.gameId||args.gameId.length>128||/[\u0000-\u001f\u007f]/u.test(args.gameId)))throw Error('PLUGIN_GAME_SCOPE');return transport({method:'fileInbox.openLink',params:{url:args.url,...(args.gameId===undefined?{}:{gameId:args.gameId})}})as Promise<{requested:true}>;},
 release:(args:{resourceIds:string[]})=>{if(args.resourceIds.length>32||new Set(args.resourceIds).size!==args.resourceIds.length)throw Error('FILE_INBOX_RESOURCE_INVALID');return transport({method:'fileInbox.release',params:{resourceIds:args.resourceIds.map(token)}})as Promise<null>;},
 });}
