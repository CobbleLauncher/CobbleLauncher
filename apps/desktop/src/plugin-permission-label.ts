// SPDX-License-Identifier: MIT
import {locale,t} from './i18n';
import type {CatalogManifest} from './plugin-catalogs';
import type {PermissionNode} from './plugin-permissions';
import {manifestSettingFields} from './extensions/manifest-settings';
function settingsKey(manifest:CatalogManifest,key:string){const [scope,...rest]=key.split(':');const id=rest.join(':');const field=manifestSettingFields(manifest.contributions?.settings).find(value=>value.id===id&&value.scope===scope);const label=field?.label?(locale.value==='zh-CN'?field.label['zh-CN']??field.label.en:field.label.en):t('navigation.settings');return t(scope==='instance'?'pluginPermissions.instanceSetting':'pluginPermissions.globalSetting',{name:label});}
export function permissionNodeLabel(node:PermissionNode, manifest:CatalogManifest){
 if('namespaces'in node)return t(node.permission==='launcher:mesh/events/send'?'pluginPermissions.meshEventsSend':'pluginPermissions.meshEventsReceive',{namespaces:node.namespaces.join(', ')});
 if(node.permission==='launcher:ai/chat')return t('pluginPermissions.aiChat');
 if(node.permission==='launcher:ai/configure')return t('pluginPermissions.aiConfigure');
 const workflowLabels={'launcher:game-config/write':'gameConfigWrite','launcher:storage/cache/clear':'storageCacheClear','launcher:instances/move-request':'instanceMoveRequest','launcher:instances/copy-request':'instanceCopyRequest','launcher:game-recovery/read':'gameRecoveryRead','launcher:game-recovery/open-request':'gameRecoveryOpenRequest','launcher:play-manifest/export':'playManifestExport','launcher:files/import-request':'fileInboxImportRequest'} as const;
 if(node.permission==='launcher:blueprints/generated-save')return t('pluginPermissions.generatedBlueprintSave');
 if(node.permission==='launcher:exports/text')return t('pluginPermissions.textExport');
 if(node.permission==='launcher:clipboard/write')return t('pluginPermissions.clipboardWrite');
 if(node.permission==='launcher:storage/private')return t('pluginPermissions.privateData');
 if('scopes'in node)return (locale.value==='zh-CN'?'读取游戏检查数据：':'Read game inspection data: ')+node.scopes.map(scope=>({mods:'Mods',packs:locale.value==='zh-CN'?'资源包':'Packs',worlds:locale.value==='zh-CN'?'世界':'Worlds',backups:locale.value==='zh-CN'?'备份':'Backups',screenshots:locale.value==='zh-CN'?'截图':'Screenshots',storage:locale.value==='zh-CN'?'存储':'Storage',runtime:locale.value==='zh-CN'?'运行环境':'Runtime',options:locale.value==='zh-CN'?'游戏设置':'Options',datapacks:locale.value==='zh-CN'?'数据包':'Data packs',servers:locale.value==='zh-CN'?'服务器名称':'Server names'})[scope]).join(', ');
 if(node.permission==='launcher:game-diagnostics/read')return locale.value==='zh-CN'?'读取近期游戏及其日志和崩溃报告':'Read recent games, logs and crash reports';
 if(node.permission==='launcher:game-keybindings/read')return locale.value==='zh-CN'?'读取游戏按键以进行对比':'Read game keybindings for comparison';
 if(node.permission==='launcher:game-keybindings/write')return locale.value==='zh-CN'?'复制游戏按键并恢复备份':'Copy game keybindings and restore backups';
 if(node.permission==='launcher:world-snapshots/read')return t('pluginPermissions.worldSnapshotsRead');
 if(node.permission==='launcher:world-snapshots/restore-request')return t('pluginPermissions.worldSnapshotsRestoreRequest');
 if(node.permission==='launcher:worlds/seed/read')return t('pluginPermissions.worldSeedRead');
 if('extensions'in node)return t(node.permission==='launcher:resource/selected-folder/read'?'pluginPermissions.selectedFolderFiles':'pluginPermissions.selectedFiles',{extensions:node.extensions.map(value=>`.${value}`).join(', ')});
 if('origins'in node)return t('pluginPermissions.network',{origins:node.origins.join(', ')});
 if('keys'in node)return node.keys.map(key=>settingsKey(manifest,key)).join(' · ');
 const labels={...workflowLabels,'launcher:ui/webview':'webview','launcher:game-config/read':'gameConfigRead','launcher:images/selected/read':'selectedImages','launcher:images/output/propose':'proposeImage','launcher:composer/draft':'draft','launcher:composer/context/public-identity':'publicIdentity','launcher:composer/context/selected-peer':'selectedPeer','launcher:composer/context/selected-session':'selectedSession'} as const;
 return t(`pluginPermissions.${labels[node.permission]}`);
}
