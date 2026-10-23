<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { InspectorSession } from './types';
import { t } from '../i18n';
defineProps<{ summary: InspectorSession['summary'] }>();
function bytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / 1024 / 1024).toFixed(1)} MiB`;
}
</script>
<template>
  <div class="inspection-summary">
    <section
      v-for="(pack, index) in [summary.current, summary.baseline].filter(Boolean)"
      :key="index"
      :aria-label="t(index ? 'inspector.baseline' : 'inspector.current')"
    >
      <p
        v-if="summary.baseline"
        class="inspection-summary-role"
      >
        {{ t(index ? 'inspector.baseline' : 'inspector.current') }}
      </p>
      <h3>{{ pack!.name.text }}<span v-if="!pack!.name.complete">…</span><span class="inspection-summary-version">{{ pack!.version_id.text }}<span v-if="!pack!.version_id.complete">…</span></span></h3>
      <dl>
        <div><dt>{{ t('inspector.declared_file') }}</dt><dd>{{ pack!.declared_files }}</dd></div><div><dt>{{ t('inspector.override_file') }}</dt><dd>{{ pack!.override_files }}</dd></div><div>
          <dt>{{ t('inspector.declaredBytes') }}</dt><dd :title="String(pack!.declared_download_bytes)">
            {{ bytes(pack!.declared_download_bytes) }}
          </dd>
        </div>
      </dl>
    </section>
  </div>
  <dl
    v-if="summary.changes"
    class="inspection-change-counts"
    :aria-label="t('inspector.changes')"
  >
    <div
      v-for="kind in (['added','removed','changed','unchanged'] as const)"
      :key="kind"
    >
      <dt>{{ t(`inspector.${kind}`) }}</dt><dd>{{ summary.changes[kind] }}</dd>
    </div>
  </dl>
</template>
<style scoped>
.inspection-summary{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,280px),1fr));gap:var(--ui-space-4);margin:var(--ui-space-4) 0}.inspection-summary section{min-width:0;padding:var(--ui-space-4);border:1px solid var(--line);border-radius:var(--ui-radius-md);background:var(--panel)}.inspection-summary h3{margin:0;font-size:var(--ui-text-body);overflow-wrap:anywhere}.inspection-summary-version{font-weight:400;color:var(--muted);margin-left:var(--ui-space-2)}.inspection-summary-role{margin:0 0 var(--ui-space-2);color:var(--muted);font-size:var(--ui-text-label)}dl{display:flex;flex-wrap:wrap;gap:var(--ui-space-4);margin:var(--ui-space-3) 0 0}dl div{display:flex;align-items:baseline;gap:var(--ui-space-2)}dt{font-size:var(--ui-text-label);color:var(--muted)}dd{margin:0;font-variant-numeric:tabular-nums;font-size:var(--ui-text-label)}.inspection-change-counts{margin-bottom:var(--ui-space-4)}
</style>
