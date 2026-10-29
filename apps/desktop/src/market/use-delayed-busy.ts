// SPDX-License-Identifier: MIT
import { onUnmounted, ref, watch, type Ref } from 'vue';

/** Keep local busy/disabled state immediate; defer only the visual loading mark. */
export function useDelayedBusy(busy: Ref<boolean>, delay = 140) {
  const visible = ref(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  watch(busy, value => {
    clearTimeout(timer);
    if (!value) visible.value = false;
    else timer = setTimeout(() => { visible.value = true; }, delay);
  }, { immediate: true, flush: 'sync' });
  onUnmounted(() => clearTimeout(timer));
  return visible;
}
