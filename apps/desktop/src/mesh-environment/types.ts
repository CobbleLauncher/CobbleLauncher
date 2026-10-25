// SPDX-License-Identifier: MIT
import type { EnvironmentRequirements,EnvironmentInstallationPreview } from '../reverse-config/types';
export type SharedMod={source:'modrinth'|'curseforge';project_id:string;version_id:string;name:string;sha1:string;size:number};
export type SharedConfig={path:string;text:string;sha256:string};
export type SharedDocument={schema_version:1;name:string;game_version:string;loader:string|null;loader_version:string|null;mods:SharedMod[];configs:SharedConfig[];excluded:string[]};
export type SharePreview={preview_id:string;document:SharedDocument;configs:{id:string;name:string;size:number;text:string}[]};
export type ReceivedEnvironment={preview_id:string;host_peer_id:string;sha256:string;document:SharedDocument;comparison:null|{available:boolean;game_id:string;version_matches?:boolean;loader_matches?:boolean;matching_mods?:number;required_mods?:number;additional_mods?:number;matching_configs?:number;required_configs?:number;local_excluded?:string[]}};
export type PreparedEnvironment={plan_id:string;host_peer_id:string;sha256:string;personal?:{sourceName?:string;keys:number;graphics:number;skipped:number;reason:string|null};preflight:Omit<EnvironmentInstallationPreview,'loader_version'>};
export type Requirements=EnvironmentRequirements;
