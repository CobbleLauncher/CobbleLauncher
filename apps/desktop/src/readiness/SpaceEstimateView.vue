<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import type { SpaceEstimate } from '../readiness';
import { locale, t } from '../i18n';
import AppIcon from '../ui/AppIcon.vue';
defineProps<{ space: SpaceEstimate }>();
function bytes(value: number) {
  const unit = value >= 1024 ** 3 ? 3 : value >= 1024 ** 2 ? 2 : value >= 1024 ? 1 : 0;
  return `${new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 }).format(value / 1024 ** unit)} ${['B', 'KiB', 'MiB', 'GiB'][unit]}`;
}
</script>
<template>
  <section
    class="space-estimate"
    :aria-label="t('readiness.space')"
  >
    <h2>
      <AppIcon
        name="hard-drive"
        :size="20"
      />{{ t('readiness.space') }}
    </h2>
    <div
      v-for="entry in space.volumes"
      :key="entry.volume.id"
      class="space-volume"
      :class="{ insufficient: entry.verdict === 'insufficient' }"
    >
      <span :title="entry.volume.label">{{ entry.volume.label }}</span>
      <strong :aria-label="t('readiness.spaceRequired', { size: bytes(entry.additional_peak_bytes) })">{{ bytes(entry.additional_peak_bytes) }}</strong>
      <span>{{ entry.volume.available_bytes === null ? t('readiness.spaceUnknown') : t('readiness.spaceAvailable', { size: bytes(entry.volume.available_bytes) }) }}</span>
    </div>
    <p
      v-if="!space.can_proceed"
      role="alert"
    >
      {{ t('readiness.spaceInsufficient') }}
    </p>
    <p
      v-else-if="!space.exact"
      class="space-note"
    >
      {{ t('readiness.spaceEstimate') }}
    </p>
  </section>
</template>
<style scoped>
.space-estimate h2{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-section);line-height:var(--ui-leading-section);margin:0 0 var(--ui-space-4)}
.space-estimate h2 :deep(svg){color:var(--ui-text-muted)}
.space-volume{display:grid;gap:var(--ui-space-2);padding-block:var(--ui-space-4);border-top:1px solid var(--ui-border);font-variant-numeric:tabular-nums}
.space-volume>span:first-child{overflow-wrap:anywhere;font-size:var(--ui-text-body)}
.space-volume>strong{font-size:var(--ui-text-page);line-height:var(--ui-leading-page);font-weight:var(--ui-weight-semibold)}
.space-volume>span:last-child,.space-note{color:var(--ui-text-muted);font-size:var(--ui-text-caption)}
.space-note{line-height:var(--ui-leading-caption);margin-top:var(--ui-space-3)}
.insufficient,.space-estimate [role=alert]{color:var(--ui-danger)}
</style>
