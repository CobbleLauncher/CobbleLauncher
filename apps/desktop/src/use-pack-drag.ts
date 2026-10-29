// SPDX-License-Identifier: MIT
import { onMounted, onUnmounted, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
export type PackDrag = { active: boolean; kind: 'mrpack' | 'archive' | 'mixed' | 'none'; count: number };
export function usePackDrag() {
  const drag = ref<PackDrag>({ active: false, kind: 'none', count: 0 });
  let disposed = false;
  let stop: UnlistenFn | undefined;
  const clear = () => { drag.value = { active: false, kind: 'none', count: 0 }; };
  onMounted(() => {
    window.addEventListener('blur', clear);
    if (isTauri()) void listen<PackDrag>('cobble-pack-drag', ({ payload }) => {
      if (!disposed && ['mrpack', 'archive', 'mixed', 'none'].includes(payload.kind)) drag.value = payload;
    }).then((unlisten) => { if (disposed) unlisten(); else stop = unlisten; }).catch(clear);
  });
  onUnmounted(() => { disposed = true; stop?.(); window.removeEventListener('blur', clear); clear(); });
  return drag;
}
