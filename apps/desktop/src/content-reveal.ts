// SPDX-License-Identifier: MIT
import { nextTick, onUnmounted, ref, watch, type Ref } from 'vue';
export interface ContentReveal { request: number; files: string[] }
/** Locate exact installed files without selecting them for batch operations. */
export function useContentReveal(target: () => ContentReveal | undefined, rows: Ref<{ file_name: string }[]>, root: Ref<HTMLElement | undefined>, prepare: (file: string) => Promise<void>) {
  const highlighted = ref<string[]>([]);
  let handled: ContentReveal | undefined, revision = 0, timer: ReturnType<typeof setTimeout> | undefined;
  const canonical = (file: string) => file.replace(/\.disabled$/i, '');
  watch(target, () => { ++revision; highlighted.value = []; clearTimeout(timer); }, { flush: 'sync' });
  watch([target, rows], async ([request, items]) => {
    if (!request) { ++revision; handled = undefined; highlighted.value = []; clearTimeout(timer); return; }
    if (request === handled) return;
    const names = new Set(request.files.map(canonical));
    const matched = items.filter(item => names.has(canonical(item.file_name))).map(item => item.file_name);
    if (!matched.length) return;
    handled = request; const current = ++revision; clearTimeout(timer);
    await prepare(matched[0]!);
    if (current !== revision) return;
    highlighted.value = matched; await nextTick();
    if (current !== revision) return;
    const row = [...(root.value?.querySelectorAll<HTMLElement>('[data-content-file]') ?? [])].find(element => element.dataset.contentFile === matched[0]);
    row?.focus({ preventScroll: true }); row?.scrollIntoView({ block: 'nearest' });
    timer = setTimeout(() => { highlighted.value = []; }, 3500);
  }, { immediate: true });
  onUnmounted(() => { ++revision; clearTimeout(timer); });
  return highlighted;
}
