<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed} from 'vue';
import {locale,t} from './i18n';
import type {CatalogManifest} from './plugin-catalogs';
import {effectivePermissionNodes,type PermissionNode} from './plugin-permissions';
import {manifestSettingFields} from './extensions/manifest-settings';
import { Globe, FolderOpen, Gamepad2, MessageSquare, PanelsTopLeft } from '@lucide/vue';
import {permissionCategory, permissionTone, type PermissionCategory} from './extensions/permission-review';
const props=defineProps<{manifest:CatalogManifest;nodes?:PermissionNode[]}>();
const rows=computed(()=>props.nodes??effectivePermissionNodes(props.manifest));
const icons={network:Globe,files:FolderOpen,game:Gamepad2,messages:MessageSquare,interface:PanelsTopLeft};
const groups=computed(()=>{
 const result=new Map<PermissionCategory,PermissionNode[]>();
 for(const node of rows.value){const category=permissionCategory(node);result.set(category,[...(result.get(category)??[]),node]);}
 return [...result].map(([category,nodes])=>({category,nodes,tone:nodes.some(node=>permissionTone(node)==='write')?'write':permissionTone(nodes[0]!)}));
});
function settingsKey(key:string){const [scope,...rest]=key.split(':');const id=rest.join(':');const field=manifestSettingFields(props.manifest.contributions?.settings).find(value=>value.id===id&&value.scope===scope);const label=field?.label?(locale.value==='zh-CN'?field.label['zh-CN']??field.label.en:field.label.en):t('navigation.settings');return t(scope==='instance'?'pluginPermissions.instanceSetting':'pluginPermissions.globalSetting',{name:label});}
function label(node:PermissionNode){
 if('scopes'in node)return (locale.value==='zh-CN'?'读取游戏检查数据：':'Read game inspection data: ')+node.scopes.map(scope=>({mods:'Mods',packs:locale.value==='zh-CN'?'资源包':'Packs',worlds:locale.value==='zh-CN'?'世界':'Worlds',backups:locale.value==='zh-CN'?'备份':'Backups',screenshots:locale.value==='zh-CN'?'截图':'Screenshots',storage:locale.value==='zh-CN'?'存储':'Storage',runtime:locale.value==='zh-CN'?'运行环境':'Runtime',options:locale.value==='zh-CN'?'游戏设置':'Options',datapacks:locale.value==='zh-CN'?'数据包':'Data packs',servers:locale.value==='zh-CN'?'服务器名称':'Server names'})[scope]).join(', ');
 if(node.permission==='launcher:game-diagnostics/read')return locale.value==='zh-CN'?'读取近期游戏及其日志和崩溃报告':'Read recent games, logs and crash reports';
 if(node.permission==='launcher:game-keybindings/read')return locale.value==='zh-CN'?'读取游戏按键以进行对比':'Read game keybindings for comparison';
 if(node.permission==='launcher:game-keybindings/write')return locale.value==='zh-CN'?'复制游戏按键并恢复备份':'Copy game keybindings and restore backups';
 if('extensions'in node)return t(node.permission==='launcher:resource/selected-folder/read'?'pluginPermissions.selectedFolderFiles':'pluginPermissions.selectedFiles',{extensions:node.extensions.map(value=>`.${value}`).join(', ')});
 if('origins'in node)return t('pluginPermissions.network',{origins:node.origins.join(', ')});
 if('keys'in node)return node.keys.map(settingsKey).join(' · ');
 const labels={'launcher:ui/webview':'webview','launcher:game-config/read':'gameConfigRead','launcher:images/selected/read':'selectedImages','launcher:images/output/propose':'proposeImage','launcher:composer/draft':'draft','launcher:composer/context/public-identity':'publicIdentity','launcher:composer/context/selected-peer':'selectedPeer','launcher:composer/context/selected-session':'selectedSession'} as const;
 return t(`pluginPermissions.${labels[node.permission]}`);
}
</script>
<template>
  <div
    v-if="rows.length"
    class="plugin-permissions"
  >
    <div
      v-for="group in groups"
      :key="group.category"
      class="permission-group"
      :data-tone="group.tone"
    >
      <span
        class="permission-group-icon"
        aria-hidden="true"
      ><component
        :is="icons[group.category]"
        :size="18"
        :stroke-width="1.7"
      /></span>
      <ul>
        <li
          v-for="node in group.nodes"
          :key="node.permission"
        >
          {{ label(node) }}
        </li>
      </ul>
    </div>
  </div>
</template>
<style scoped>
.plugin-permissions{display:grid;gap:12px;margin:12px 0;font-size:14px;line-height:1.5;overflow-wrap:anywhere}
.permission-group{--permission-tone:var(--muted);display:flex;align-items:flex-start;gap:12px}.permission-group[data-tone=network]{--permission-tone:var(--ui-warning)}.permission-group[data-tone=files]{--permission-tone:var(--ui-info)}.permission-group[data-tone=write]{--permission-tone:var(--ui-danger)}.permission-group-icon{display:grid;place-items:center;flex:none;width:32px;height:32px;border-radius:6px;background:color-mix(in srgb,var(--permission-tone) 10%,transparent);color:var(--permission-tone)}
.permission-group ul{list-style:none;padding:4px 0;margin:0;min-width:0}.permission-group li+li{margin-top:8px}
</style>
