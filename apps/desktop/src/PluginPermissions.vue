<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed} from 'vue';
import {permissionNodeLabel} from './plugin-permission-label';
import type {CatalogManifest} from './plugin-catalogs';
import {effectivePermissionNodes,type PermissionNode} from './plugin-permissions';
import { Globe, FolderOpen, Gamepad2, MessageSquare, PanelsTopLeft, Database, Clipboard, FileDown, FileInput } from '@lucide/vue';
import {permissionCategory, permissionTone, type PermissionCategory} from './extensions/permission-review';
const props=defineProps<{manifest:CatalogManifest;nodes?:PermissionNode[]}>();
const rows=computed(()=>props.nodes??effectivePermissionNodes(props.manifest));
const icons={network:Globe,files:FolderOpen,game:Gamepad2,messages:MessageSquare,interface:PanelsTopLeft,storage:Database,clipboard:Clipboard,exports:FileDown,handoff:FileInput};
const groups=computed(()=>{
 const result=new Map<PermissionCategory,PermissionNode[]>();
 for(const node of rows.value){const category=permissionCategory(node);result.set(category,[...(result.get(category)??[]),node]);}
 return [...result].map(([category,nodes])=>({category,nodes,tone:nodes.some(node=>permissionTone(node)==='write')?'write':permissionTone(nodes[0]!)}));
});
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
          {{ permissionNodeLabel(node, manifest) }}
        </li>
      </ul>
    </div>
  </div>
</template>
<style scoped>
.plugin-permissions{display:grid;gap:var(--ui-space-3);margin:var(--ui-space-3) 0;font-size:var(--ui-text-body);line-height:var(--ui-leading-body);overflow-wrap:anywhere}.permission-group{--permission-tone:var(--ui-text-muted);display:flex;align-items:flex-start;gap:var(--ui-space-3)}.permission-group[data-tone=network]{--permission-tone:var(--ui-warning)}.permission-group[data-tone=files]{--permission-tone:var(--ui-info)}.permission-group[data-tone=write]{--permission-tone:var(--ui-danger)}.permission-group-icon{display:grid;place-items:center;flex:none;width:32px;height:32px;border-radius:var(--ui-radius-sm);background:color-mix(in srgb,var(--permission-tone) 8%,transparent);color:var(--permission-tone)}.permission-group ul{list-style:none;padding:var(--ui-space-1) 0;margin:0;min-width:0}.permission-group li+li{margin-top:var(--ui-space-2)}
</style>
