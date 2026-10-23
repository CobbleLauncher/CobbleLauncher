<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import PluginPermissions from './PluginPermissions.vue';
import type { CatalogManifest } from './plugin-catalogs';
import type { PermissionNode } from './plugin-permissions';
import { permissionConsentKey, permissionTone } from './extensions/permission-review';
import PermissionConsentCard from './ui/PermissionConsentCard.vue';

defineProps<{ manifest: CatalogManifest; nodes: PermissionNode[]; disabled?: boolean }>();
const accepted = defineModel<string[]>({ required: true });
function toggle(node: PermissionNode, checked: boolean) {
  const key = permissionConsentKey(node);
  accepted.value = checked ? [...new Set([...accepted.value, key])]
    : accepted.value.filter(value => value !== key);
}
</script>
<template>
  <section
    v-if="nodes.length"
    class="sensitive-permissions"
  >
    <h3>{{ t('pluginPermissions.sensitiveTitle') }}</h3>
    <PermissionConsentCard
      v-for="node in nodes"
      :key="permissionConsentKey(node)"
      class="sensitive-permission"
      :model-value="accepted.includes(permissionConsentKey(node))"
      :disabled="disabled"
      :tone="permissionTone(node)"
      :label="t('pluginPermissions.allowScope')"
      @update:model-value="toggle(node, $event)"
    >
      <PluginPermissions
        :manifest="manifest"
        :nodes="[node]"
      />
    </PermissionConsentCard>
  </section>
</template>
<style scoped>
.sensitive-permissions{display:grid;gap:8px;margin:var(--ui-space-4) 0}
.sensitive-permissions h3{font-size:var(--ui-text-body);margin:0 0 var(--ui-space-3)}
.sensitive-permission :deep(.plugin-permissions){margin:0}
</style>
