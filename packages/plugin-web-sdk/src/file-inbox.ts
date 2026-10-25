// SPDX-License-Identifier: MIT
import type {Transport} from './index';
export type FileInboxKind='mrpack'|'curseforge-pack'|'mod'|'resource-pack'|'shader-pack'|'blueprint'|'log'|'ambiguous'|'unknown';
export type FileInboxHandler={id:string;label:string;kind:'import'|'viewer'|'report';requiresGame:boolean};
export type FileInboxGame={id:string;name:string;version:string;compatibility:'declared-match'|'unknown'|'mismatch';reason:string|null;available:boolean};
export type FileInboxItem={resourceId:string;name:string;bytes:number;kind:FileInboxKind;evidence:string[];handlers:FileInboxHandler[];games:FileInboxGame[];expiresAt:number;declaredMinecraft:string|null;declaredLoader:string|null;error:string|null};
export type FileInboxOpen={resourceId:string;handlerId:string;gameId?:string};
function token(value:string){if(!/^[a-f0-9]{32}$/.test(value))throw Error('FILE_INBOX_RESOURCE_INVALID');return value;}
function open(args:FileInboxOpen){token(args.resourceId);if(!/^[a-z0-9][a-z0-9:._-]{0,159}$/.test(args.handlerId))throw Error('FILE_INBOX_HANDLER_INVALID');if(args.gameId!==undefined&&(!args.gameId||args.gameId.length>128||/[\u0000-\u001f\u007f]/u.test(args.gameId)))throw Error('PLUGIN_GAME_SCOPE');return{resourceId:args.resourceId,handlerId:args.handlerId,...(args.gameId===undefined?{}:{gameId:args.gameId})};}
/** User-selected host resources. No path or file bytes cross this interface. */
export function createFileInbox(transport:Transport){return Object.freeze({
 pick:()=>transport({method:'fileInbox.pick',params:{}})as Promise<FileInboxItem[]>,
 pending:()=>transport({method:'fileInbox.pending',params:{}})as Promise<FileInboxItem[]>,
 open:(args:FileInboxOpen)=>transport({method:'fileInbox.open',params:open(args)})as Promise<{requested:true}>,
 release:(args:{resourceIds:string[]})=>{if(args.resourceIds.length>32||new Set(args.resourceIds).size!==args.resourceIds.length)throw Error('FILE_INBOX_RESOURCE_INVALID');return transport({method:'fileInbox.release',params:{resourceIds:args.resourceIds.map(token)}})as Promise<null>;},
 });}
