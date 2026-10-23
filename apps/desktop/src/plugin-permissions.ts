// SPDX-License-Identifier: MIT
import type { CatalogManifest } from './plugin-catalogs';
import { manifestSettingFields } from './extensions/manifest-settings';
export type InspectionScope = 'mods'|'packs'|'worlds'|'backups'|'screenshots'|'storage'|'runtime'|'options'|'datapacks'|'servers';
export type PermissionNode =
 | {permission:'launcher:game-inspection/read';scopes:InspectionScope[]}
 | {permission:'launcher:ui/webview'|'launcher:game-config/read'|'launcher:game-diagnostics/read'|'launcher:game-keybindings/read'|'launcher:game-keybindings/write'}
 | {permission:'launcher:resource/selected/read';extensions:string[]}
 | {permission:'launcher:resource/selected-folder/read';extensions:string[]}
 | {permission:'launcher:provider/https/get';origins:string[]}
 | {permission:'launcher:settings/read';keys:string[]}
 | {permission:'launcher:images/selected/read'|'launcher:images/output/propose'|'launcher:composer/draft'|'launcher:composer/context/public-identity'|'launcher:composer/context/selected-peer'|'launcher:composer/context/selected-session'};
const composerNodes = {draft:'launcher:composer/draft',public_identity:'launcher:composer/context/public-identity',selected_peer:'launcher:composer/context/selected-peer',selected_session:'launcher:composer/context/selected-session'} as const;
export function effectivePermissionNodes(manifest:CatalogManifest):PermissionNode[] {
 if(manifest.permissions)return manifest.permissions;
 const nodes:PermissionNode[]=[];
 if(manifest.frontend)nodes.push({permission:'launcher:ui/webview'});
 if(manifest.capabilities.some(value=>value==='report'||value==='viewer'))nodes.push({permission:'launcher:resource/selected/read',extensions:[...manifest.file_extensions]});
 if(manifest.capabilities.includes('blueprint-provider'))nodes.push({permission:'launcher:provider/https/get',origins:[...(manifest.provider_origins??[])]});
 for(const scope of manifest.composer_permissions??[])nodes.push({permission:composerNodes[scope]});
 if(manifest.capabilities.includes('report')&&manifest.api_version==='0.2.0'){
  const keys=manifestSettingFields(manifest.contributions?.settings).filter(field=>field.id).map(field=>`${field.scope}:${field.id}`);
  if(keys.length)nodes.push({permission:'launcher:settings/read',keys:[...new Set(keys)].sort()});
 }
 return nodes;
}
export function addedPermissionNodes(next:CatalogManifest,previous:CatalogManifest):PermissionNode[]{
 const old=effectivePermissionNodes(previous);
 return effectivePermissionNodes(next).flatMap((node):PermissionNode[]=>{
  const prior=old.find(value=>value.permission===node.permission);
  if(!prior)return [node];
  if('extensions'in node&&'extensions'in prior){const extensions=node.extensions.filter(value=>!prior.extensions.includes(value));return extensions.length?[{...node,extensions}]:[];}
  if('scopes'in node&&'scopes'in prior){const scopes=node.scopes.filter(value=>!prior.scopes.includes(value));return scopes.length?[{...node,scopes}]:[];}
  if('origins'in node&&'origins'in prior){const origins=node.origins.filter(value=>!prior.origins.includes(value));return origins.length?[{...node,origins}]:[];}
  if('keys'in node&&'keys'in prior){const keys=node.keys.filter(value=>!prior.keys.includes(value));return keys.length?[{...node,keys}]:[];}
  return [];
 });
}
