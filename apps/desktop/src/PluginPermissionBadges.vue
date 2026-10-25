<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import { t } from './i18n';
import { effectivePermissionNodes, type PermissionNode } from './plugin-permissions';
import type { CatalogManifest } from './plugin-catalogs';
import AppIcon from './ui/AppIcon.vue';
import { permissionNodeLabel } from './plugin-permission-label';

const props = defineProps<{ manifest: CatalogManifest }>();
type Badge = { kind: 'network' | 'files' | 'world' | 'writes'; nodes: PermissionNode[] };
const badges = computed<Badge[]>(() => {
  const grouped = new Map<Badge['kind'], PermissionNode[]>([['network', []], ['files', []], ['world', []], ['writes', []]]);
  for (const node of effectivePermissionNodes(props.manifest)) {
    const permission = node.permission as string;
    const kind: Badge['kind'] | undefined = permission === 'launcher:worlds/seed/read'
      ? 'world'
      : permission === 'launcher:provider/https/get' || permission.startsWith('launcher:ai/') || permission.startsWith('launcher:mesh/events/')
      ? 'network'
      : ['launcher:resource/selected/read', 'launcher:resource/selected-folder/read', 'launcher:game-config/read', 'launcher:game-keybindings/read', 'launcher:game-diagnostics/read', 'launcher:game-inspection/read', 'launcher:world-snapshots/read', 'launcher:game-recovery/read', 'launcher:play-manifest/export'].includes(permission)
        ? 'files'
        : ['launcher:game-keybindings/write', 'launcher:images/output/propose', 'launcher:world-snapshots/restore-request', 'launcher:game-config/write', 'launcher:storage/cache/clear', 'launcher:instances/move-request', 'launcher:instances/copy-request'].includes(permission)
          ? 'writes'
          : undefined;
    if (kind) grouped.get(kind)!.push(node);
  }
  return [...grouped].filter(([, nodes]) => nodes.length).map(([kind, nodes]) => ({ kind, nodes }));
});
const labels = { network: 'networkBadge', files: 'filesBadge', world: 'worldSeedBadge', writes: 'writesBadge' } as const;
const icons = { network: 'network', files: 'folder', world: 'folder', writes: 'pencil' } as const;
function detail(node: PermissionNode): string {
  if ('namespaces' in node) return permissionNodeLabel(node, props.manifest);
  if (node.permission === 'launcher:ai/chat') return t('pluginPermissions.aiChat');
  if (node.permission === 'launcher:ai/configure') return t('pluginPermissions.aiConfigure');
  if (node.permission === 'launcher:worlds/seed/read') return t('pluginPermissions.worldSeedRead');
  if ('scopes' in node) return node.scopes.join(', ');
  if ('origins' in node) return node.origins.join(', ');
  if ('extensions' in node) return node.extensions.map(value => `.${value}`).join(', ');
  return node.permission;
}
function title(badge: Badge): string {
  return `${t(`pluginPermissions.${labels[badge.kind]}`)}: ${badge.nodes.map(detail).filter(Boolean).join(' · ')}`;
}
</script>
<template>
  <span
    v-if="badges.length"
    class="plugin-permission-badges"
    :aria-label="t('pluginPermissions.badgesLabel')"
  >
    <span
      v-for="badge in badges"
      :key="badge.kind"
      class="plugin-permission-badge"
      :title="title(badge)"
    >
      <AppIcon :name="icons[badge.kind]" />{{ t(`pluginPermissions.${labels[badge.kind]}`) }}
    </span>
  </span>
</template>
<style scoped>
.plugin-permission-badges{display:flex;flex-wrap:wrap;gap:5px;margin-top:7px}
.plugin-permission-badge{display:inline-flex;align-items:center;gap:4px;border:1px solid color-mix(in srgb,var(--ui-warning) 35%,var(--line));border-radius:999px;padding:2px 7px;background:color-mix(in srgb,var(--ui-warning) 10%,transparent);color:var(--ui-warning);font-size:var(--ui-text-caption,12px);line-height:1.5}
.plugin-permission-badge :deep(svg){width:12px;height:12px}
</style>
