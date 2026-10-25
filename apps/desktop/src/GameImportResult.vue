<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { t } from './i18n';
import AppIcon from './ui/AppIcon.vue';

type ImportResult = {
  mode: 'copy' | 'move';
  sourceRetained: boolean;
  recycledRoots: number;
  retainedRoots: number;
};

const props = defineProps<{ gameId: string; status: 'installing' | 'installed' | 'failed' }>();
const result = ref<ImportResult | null>(null);
let disposed = false;
let request = 0;

watch(() => [props.gameId, props.status] as const, async ([gameId, status]) => {
  const currentRequest = ++request;
  result.value = null;
  if (status !== 'installed') return;
  try {
    const value = await invoke<ImportResult | null>('get_launcher_import_result', { id: gameId });
    if (!disposed && currentRequest === request && gameId === props.gameId && status === props.status) result.value = value?.mode === 'move' ? value : null;
  } catch {
    if (currentRequest === request) result.value = null;
  }
}, { immediate: true });

onUnmounted(() => { disposed = true; request++; });
</script>

<template>
  <aside
    v-if="result"
    class="game-import-result"
    role="status"
  >
    <div class="import-result-line">
      <AppIcon
        :name="result.recycledRoots > 0 ? 'check' : 'info'"
        :class="result.recycledRoots > 0 ? 'ui-success' : 'ui-text-muted'"
      />
      <span>{{ result.recycledRoots > 0 ? t('launcherImport.movedRoots', { count: result.recycledRoots }) : t('launcherImport.importedSourceKept') }}</span>
      <span
        class="shared-info"
        role="note"
        :aria-label="t('launcherImport.sharedRuntimeRetained')"
        :title="t('launcherImport.sharedRuntimeRetained')"
      ><AppIcon name="info" /></span>
    </div>
    <div
      v-if="result.sourceRetained && result.retainedRoots > 0"
      class="import-result-warning ui-warning"
    >
      <AppIcon name="alert-circle" />
      <span>{{ t('launcherImport.retainedRoots', { count: result.retainedRoots }) }}</span>
    </div>
  </aside>
</template>

<style scoped>
.game-import-result{display:grid;gap:var(--ui-space-2);margin:0 0 var(--ui-space-4);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption)}
.import-result-line,.import-result-warning{display:flex;align-items:center;gap:var(--ui-space-2);min-width:0}
.import-result-line>span:nth-child(2),.import-result-warning>span{overflow-wrap:anywhere}
.shared-info{display:inline-flex;align-items:center;flex:none;color:var(--ui-text-muted);cursor:help}
.shared-info :deep(.app-icon){width:14px;height:14px}
</style>
