// SPDX-License-Identifier: MIT
import {expect,it} from 'vitest';
import {isSupportedCatalogPlugin,type CatalogManifest} from './plugin-catalogs';
import {addedPermissionNodes} from './plugin-permissions';
import {permissionConsentKey,requiresSensitiveConsent} from './extensions/permission-review';
function fixture():CatalogManifest{return{id:'inspector',name:'Inspector',publisher:'Fixture',version:'0.1.0',api_version:'0.2.0',capabilities:['report'],file_extensions:['json'],component_sha256:'a'.repeat(64),frontend:{api_version:'0.1.0',entry:'ui/index.html',assets_sha256:'b'.repeat(64)},permissions:[{permission:'launcher:ui/webview'},{permission:'launcher:game-inspection/read',scopes:['mods']}]};}
it('accepts scoped inspection without selected-file permission and rejects incompatible manifests',()=>{
 const manifest=fixture();expect(isSupportedCatalogPlugin({manifest,description:''})).toBe(true);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,file_extensions:[]},description:''})).toBe(true);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,file_extensions:[],frontend:undefined,permissions:undefined},description:''})).toBe(false);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,api_version:'0.1.0'},description:''})).toBe(false);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,frontend:undefined},description:''})).toBe(false);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,file_extensions:['txt']},description:''})).toBe(false);
 expect(isSupportedCatalogPlugin({manifest:{...manifest,permissions:[{permission:'launcher:ui/webview'},{permission:'launcher:game-inspection/read',scopes:[]}]},description:''})).toBe(false);
});
it('requires a new acknowledgement only for the added inspection scopes',()=>{
 const previous=fixture(),next={...previous,permissions:[{permission:'launcher:ui/webview' as const},{permission:'launcher:game-inspection/read' as const,scopes:['mods','packs'] as const}]};
 const reviewed:CatalogManifest={...next,permissions:[{permission:'launcher:ui/webview'},{permission:'launcher:game-inspection/read',scopes:['mods','packs']}]};
 expect(addedPermissionNodes(reviewed,previous)).toEqual([{permission:'launcher:game-inspection/read',scopes:['packs']}]);
 expect(requiresSensitiveConsent({permission:'launcher:game-inspection/read',scopes:['packs']})).toBe(true);
 expect(permissionConsentKey({permission:'launcher:game-inspection/read',scopes:['mods','packs']})).not.toBe(permissionConsentKey({permission:'launcher:game-inspection/read',scopes:['mods']}));
 expect(addedPermissionNodes(previous,reviewed)).toEqual([]);
});
