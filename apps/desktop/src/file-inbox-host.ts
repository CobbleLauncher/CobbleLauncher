// SPDX-License-Identifier: MIT
import type {PackPreview} from './games';
import type {CurseForgePackPreview} from './curseforge';
import type {OpenedBlueprint} from './blueprints';
import type {PluginReport} from './plugin-reports';
export type FileInboxHostRequest={handoff_id:string;plugin_id:string};
export type FileInboxHostPreview={handoffId:string;name:string;bytes:number;handler:{id:string;label:string;kind:'import'|'viewer'|'report';requiresGame:boolean};game:{id:string;name:string;version:string;available:boolean}|null;warning:'executable_mod'|null};
export type FileInboxResult={kind:'mrpack';preview:PackPreview}|{kind:'curseforge-pack';preview:CurseForgePackPreview}|{kind:'viewer';viewer:OpenedBlueprint}|{kind:'game-content';gameId:string;contentKind:'mod'|'resource-pack'|'shader-pack';imported:true}|{kind:'report';report:PluginReport;pluginId:string};
