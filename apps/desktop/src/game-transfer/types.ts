// SPDX-License-Identifier: MIT
export const transferScopes = ['saves','mods','resourcepacks','shaderpacks','config','options'] as const;
export type TransferScope = typeof transferScopes[number];
export type TransferSelection = Record<TransferScope,boolean>;
export type TransferPeer = { peerId:string; displayName:string; connected:boolean };
export type TransferContext = { gameId:string; name:string; version:string; loaderKind:string|null; loaderVersion:string|null; availableScopes:TransferScope[]; meshAvailable:boolean;stopped:boolean };
export type TransferPreview = { offerId:string; gameName:string; fileCount:number; totalBytes:number; version:string; loaderKind:string|null; loaderVersion:string|null; sharedFileNames:true };
export type TransferOffer = { peerId:string; manifestId:string; gameName?:string; version:string; loaderKind:string|null; loaderVersion:string|null; fileCount:number; totalBytes:number; scopes:TransferScope[] };
export type OutgoingTransfer = { offerId:string;gameId?:string; state:'prepared'|'waiting'|'transferring'|'completed'|'expired'|'cancelled'|'rejected'; bytesDone:number; bytesTotal:number; error?:string|null; gameName?:string; peerId?:string|null };
export type TransferRecord = { taskId:string; state:'queued'|'waiting'|'running'|'succeeded'|'failed'|'cancelled'|'interrupted'|'cancelling'; bytesDone:number; bytesTotal:number|null;phase?:string; error?:string|null; gameId?:string|null; direction:'send'|'receive'; gameName?:string; peerName?:string };
export function emptyTransferSelection(): TransferSelection { return { saves:false,mods:false,resourcepacks:false,shaderpacks:false,config:false,options:false }; }
