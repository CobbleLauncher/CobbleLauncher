// SPDX-License-Identifier: MIT
export type PlayConfig = {id:string;name:string;size:number;text:string};
export type PlayModReference = {name:string;source:string;projectId:string;versionId:string;sha1:string;url:string;size:number};
export type PlayManifestPreview = {revision:string;name:string;version:string;loader:string|null;loaderVersion:string|null;mods:PlayModReference[];configs:PlayConfig[];excludedMods:string[]};
type Transport=(request:{method:'playManifest.preview'|'playManifest.export';params:Record<string,unknown>})=>Promise<unknown>;
function game(id:string){if(typeof id!=='string'||!id||id.length>128||[...id].some(char=>char.charCodeAt(0)<32))throw Error('PLAY_SHARE_SCOPE');return id;}
export function createPlayManifest(transport:Transport){return Object.freeze({preview:(gameId:string)=>transport({method:'playManifest.preview',params:{gameId:game(gameId)}}) as Promise<PlayManifestPreview>,export:(input:{gameId:string;revision:string;configIds:string[]})=>{if(!/^[a-f0-9]{64}$/.test(input.revision)||input.configIds.length>64||new Set(input.configIds).size!==input.configIds.length||input.configIds.some(id=>!/^[a-f0-9]{64}$/.test(id)))throw Error('PLAY_SHARE_REQUEST_INVALID');return transport({method:'playManifest.export',params:{gameId:game(input.gameId),revision:input.revision,configIds:[...input.configIds]}}) as Promise<{saved:boolean;mods?:number;configs?:number}>;}});}


