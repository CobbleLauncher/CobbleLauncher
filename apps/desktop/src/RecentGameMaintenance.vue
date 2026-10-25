<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { locale } from './i18n';
import { maintenanceCopy, recentMaintenance, type MaintenanceOverview, type MaintenanceItem } from './recent-maintenance';
const props = defineProps<{ gameId: string; revision?: number }>();
const copy = computed(() => maintenanceCopy[locale.value]);
const records = ref<ReturnType<typeof recentMaintenance>>([]), loading = ref(false), failed = ref(false), partial = ref(false), limited = ref(false);
let generation = 0;
async function load() {
  const current = ++generation; loading.value = true; failed.value = false;
  try {
    const value = await invoke<MaintenanceOverview>('list_game_maintenance', { id: props.gameId });
    if (current === generation) { records.value = recentMaintenance(value.items ?? []); partial.value = Boolean(value.unavailable?.length); limited.value = value.limited || (value.items?.length ?? 0) > 40; }
  } catch { if (current === generation) failed.value = true; }
  finally { if (current === generation) loading.value = false; }
}
function status(item: MaintenanceItem): string {
  if (item.status !== 'applied' && item.status !== 'undone' && item.status !== 'undoing' && item.status !== 'installing' && item.status !== 'automatic') return '';
  return copy.value[item.status];
}
watch(() => [props.gameId, props.revision], () => { records.value = []; void load(); }, { immediate: true });
</script>
<template>
  <section
    class="recent-maintenance"
    :aria-label="copy.title"
    :aria-busy="loading"
  >
    <h3>{{ copy.title }}</h3>
    <p
      v-if="failed || partial"
      :role="failed ? 'alert' : 'status'"
      class="ui-warning"
    >
      {{ failed ? copy.failed : copy.partial }} <button
        type="button"
        class="text-button"
        :disabled="loading"
        @click="load"
      >
        {{ copy.retry }}
      </button>
    </p>
    <p
      v-if="!records.length && !loading && !failed"
      class="ui-caption"
    >
      {{ copy.empty }}
    </p>
    <ol v-if="records.length">
      <li
        v-for="item in records"
        :key="`${item.kind}:${item.id}`"
      >
        <div>
          <span class="maintenance-kind">{{ copy[item.kind] }}</span><span v-if="item.title">{{ item.title }}</span><span
            v-if="status(item)"
            class="ui-caption"
          >{{ status(item) }}</span>
        </div>
        <time :datetime="new Date(item.at).toISOString()">{{ new Date(item.at).toLocaleString(locale) }}</time>
      </li>
    </ol>
    <p
      v-if="limited && records.length"
      class="ui-caption"
    >
      {{ copy.limited }}
    </p>
  </section>
</template>
<style scoped>
.recent-maintenance{padding-block:var(--ui-space-3);min-width:0}
h3{margin:0 0 var(--ui-space-3);font-size:var(--ui-text-section);line-height:var(--ui-leading-section);font-weight:var(--ui-weight-semibold)}
ol{list-style:none;margin:0;padding:0;max-height:360px;overflow:auto}
li{display:flex;justify-content:space-between;align-items:start;gap:var(--ui-space-4);padding:var(--ui-space-3) 0;border-top:1px solid var(--ui-border)}
li>div{display:flex;gap:var(--ui-space-2);flex-wrap:wrap;min-width:0;overflow-wrap:anywhere;font-size:var(--ui-text-body)}
.maintenance-kind{font-weight:var(--ui-weight-medium)}
time{color:var(--ui-text-muted);font-size:var(--ui-text-caption);line-height:var(--ui-leading-caption);white-space:nowrap;font-variant-numeric:tabular-nums}
@media(max-width:600px){li{flex-direction:column;gap:var(--ui-space-1)}}
</style>
