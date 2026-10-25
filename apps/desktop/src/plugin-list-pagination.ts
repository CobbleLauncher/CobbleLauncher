// SPDX-License-Identifier: MIT
import { computed, ref, watch, type ComputedRef, type WatchSource } from 'vue';

export const pluginListPageSize = 8;

export function usePluginListPagination<T>(items: ComputedRef<T[]>, resetOn: WatchSource[], pageSize = pluginListPageSize) {
  const page = ref(1);
  const pageCount = computed(() => Math.max(1, Math.ceil(items.value.length / pageSize)));
  const pageItems = computed(() => {
    const start = (page.value - 1) * pageSize;
    return items.value.slice(start, start + pageSize);
  });
  watch(resetOn, () => { page.value = 1; }, { flush: 'sync' });
  watch(pageCount, count => { page.value = Math.min(page.value, count); }, { flush: 'sync' });
  return { page, pageCount, pageItems };
}
